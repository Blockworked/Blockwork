# Runs the submission flow against mocked Store responses without credentials or uploads.
$ErrorActionPreference = 'Stop'
$submit = Join-Path $PSScriptRoot '../../scripts/submit-msstore.ps1'
$temp = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory $temp | Out-Null
$file = Join-Path $temp 'test.msix'
Set-Content $file 'test package'
$names = @('MSSTORE_TENANT_ID', 'MSSTORE_CLIENT_ID', 'MSSTORE_CLIENT_SECRET', 'MSSTORE_APP_ID')
$saved = @{}
foreach ($name in $names) {
  $saved[$name] = [Environment]::GetEnvironmentVariable($name)
  [Environment]::SetEnvironmentVariable($name, 'mock')
}
function Start-Sleep { param($Seconds) }
function Invoke-RestMethod {
  param($Method, $Uri, $Body, $Headers, $ContentType, $InFile)
  $global:msstoreTestCalls.Add("$Method $Uri")
  if ($Uri -like 'https://login.microsoftonline.com/*') { return @{ access_token = 'mock' } }
  if ($Uri -match '/listflights') {
    if ($global:msstoreTestScenario -eq 'empty') {
      $exception = [Exception]::new('No flights')
      $exception | Add-Member Response ([pscustomobject]@{ StatusCode = 404 })
      throw $exception
    }
    if ($global:msstoreTestScenario -eq 'missing-id') { return @{ value = @() } }
    if ($global:msstoreTestScenario -eq 'paged' -and $Uri -notmatch 'skip=') {
      return @{ value = @(); '@nextLink' = 'applications/mock/listflights?skip=100&top=100' }
    }
    return @{ value = @(@{ flightId = 'flight'; friendlyName = '0.6 Test' }) }
  }
  if ($Method -eq 'Post' -and $Uri -match '/flights$') {
    $created = $Body | ConvertFrom-Json
    if ($created.friendlyName -ne '0.6 Test' -or ($created.groupIds -join ',') -ne 'one,two') { throw 'Wrong flight creation body' }
    return @{ flightId = 'flight'; friendlyName = $created.friendlyName }
  }
  if ($Method -eq 'Delete') {
    if ($global:msstoreTestScenario -in @('portal', 'public-portal')) {
      $record = [Management.Automation.ErrorRecord]::new([Exception]::new('InvalidOperation'), 'InvalidOperation', 'InvalidOperation', $null)
      $record.ErrorDetails = [Management.Automation.ErrorDetails]::new('{"message":"Ingestion API can only update, delete, and commit submissions that are created through the API"}')
      throw $record
    }
    if ($global:msstoreTestScenario -eq 'denied') { throw 'Access denied' }
    return
  }
  if ($Method -eq 'Get' -and $Uri -match '/status$') { return @{ status = 'PreProcessing' } }
  if ($Method -eq 'Get') {
    return @{ pendingFlightSubmission = @{ id = 'draft' }; pendingApplicationSubmission = @{ id = 'draft' } }
  }
  if ($Method -eq 'Post' -and $Uri -match '/submissions$') {
    return [pscustomobject]@{ id = 'new'; flightPackages = @(); applicationPackages = @(); fileUploadUrl = 'https://mock-upload.invalid' }
  }
  if ($Method -eq 'Put' -or $Uri -match '/commit$') { return }
  throw "Unexpected request: $Method $Uri"
}
try {
  foreach ($global:msstoreTestScenario in @('existing', 'empty', 'paged', 'portal', 'public-portal', 'missing-id', 'denied')) {
    $global:msstoreTestCalls = [Collections.Generic.List[string]]::new()
    $flight = if ($global:msstoreTestScenario -eq 'public-portal') { '' } elseif ($global:msstoreTestScenario -eq 'missing-id') { 'cbe98074-b874-41d4-8807-6440e1bcde2e' } else { '0.6 Test' }
    $expected = switch ($global:msstoreTestScenario) {
      'portal' { 'keeping the flight and its tester groups' }
      'public-portal' { 'Delete the in-progress app submission' }
      'missing-id' { 'no longer exists' }
      'denied' { 'Access denied' }
      default { $null }
    }
    $failure = $null
    try { & $submit -Msix $file -Flight $flight -FlightGroupIds 'one, two' }
    catch { $failure = $_.Exception.Message }
    if ($expected) {
      if (-not $failure -or -not $failure.Contains($expected)) { throw "${global:msstoreTestScenario}: expected '$expected', got '$failure'" }
      if ($global:msstoreTestCalls | Where-Object { $_ -match '^Post .*/submissions' -or $_ -like '*mock-upload*' }) { throw 'Submission continued after failure' }
    } elseif ($failure) { throw "${global:msstoreTestScenario}: $failure" }
    else {
      if (-not ($global:msstoreTestCalls | Where-Object { $_ -match '^Post .*/submissions/new/commit$' })) { throw 'Missing commit' }
      if ($global:msstoreTestScenario -eq 'empty' -and -not ($global:msstoreTestCalls | Where-Object { $_ -match '^Post .*/flights$' })) { throw 'Missing flight creation' }
      if ($global:msstoreTestScenario -ne 'empty' -and ($global:msstoreTestCalls | Where-Object { $_ -match '^Post .*/flights$' })) { throw 'Existing flight was recreated' }
    }
    Write-Host "PASS: $global:msstoreTestScenario"
  }
} finally {
  foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
  Remove-Item -Recurse -Force $temp
}
