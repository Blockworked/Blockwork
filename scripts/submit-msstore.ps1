# Submits MSIX packages (one per architecture) to the Microsoft Store through the Partner Center submission API.
# Copies the last published submission, swaps in the new packages and commits it for certification.
#
# With -Flight it goes to that package flight (its name or ID) instead of the public listing.
# Missing flight names are created through the API; -FlightGroupIds selects their tester groups.
#
# Usage: scripts/submit-msstore.ps1 -Msix dist/blockwork-windows-x86_64.msix, dist/blockwork-windows-arm64.msix [-Flight Beta]
# Needs MSSTORE_TENANT_ID, MSSTORE_CLIENT_ID, MSSTORE_CLIENT_SECRET and MSSTORE_APP_ID in the environment.
param(
  [Parameter(Mandatory)][string[]]$Msix,
  [string]$Flight,
  [string[]]$FlightGroupIds = @(),
  [switch]$NoCommit
)

$ErrorActionPreference = "Stop"
if ($Flight -and -not $Flight.Trim()) { throw "Flight must contain a name or ID, or be empty for the public listing." }
foreach ($name in "MSSTORE_TENANT_ID", "MSSTORE_CLIENT_ID", "MSSTORE_CLIENT_SECRET", "MSSTORE_APP_ID") {
  if (-not [Environment]::GetEnvironmentVariable($name)) { throw "$name is not set" }
}
$Msix = @($Msix | ForEach-Object { (Resolve-Path $_).Path })
$api = "https://manage.devcenter.microsoft.com/v1.0/my/applications/$env:MSSTORE_APP_ID"

$token = (Invoke-RestMethod -Method Post -Uri "https://login.microsoftonline.com/$env:MSSTORE_TENANT_ID/oauth2/token" -Body @{
  grant_type    = "client_credentials"
  client_id     = $env:MSSTORE_CLIENT_ID
  client_secret = $env:MSSTORE_CLIENT_SECRET
  resource      = "https://manage.devcenter.microsoft.com"
}).access_token
$headers = @{ Authorization = "Bearer $token" }

function Call($Method, $Uri, $Body) {
  $params = @{ Method = $Method; Uri = $Uri; Headers = $headers; ContentType = "application/json; charset=utf-8" }
  if ($Body) { $params.Body = ($Body | ConvertTo-Json -Depth 32) }
  Invoke-RestMethod @params
}

# A flight's submissions live under the flight and list flightPackages instead
$base = $api
$packagesField = "applicationPackages"
$pendingField = "pendingApplicationSubmission"
if ($Flight) {
  $Flight = $Flight.Trim()
  $flights = @()
  $next = "$api/listflights?top=100"
  do {
    try {
      $page = Call Get $next
    } catch {
      # The API returns 404 when the app has no flights.
      if ($next -eq "$api/listflights?top=100" -and [int]$_.Exception.Response.StatusCode -eq 404) { break }
      throw
    }
    $flights += @($page.value | Where-Object { $_ })
    $next = if ($page.'@nextLink') { "https://manage.devcenter.microsoft.com/v1.0/my/$($page.'@nextLink')" } else { $null }
  } while ($next)
  $matches = @($flights | Where-Object { $_.flightId -eq $Flight })
  if (-not $matches) { $matches = @($flights | Where-Object { $_.friendlyName -eq $Flight }) }
  if ($matches.Count -gt 1) { throw "Multiple package flights named '$Flight'. Use a flight ID instead." }
  $match = $matches | Select-Object -First 1
  if (-not $match) {
    $flightGuid = [guid]::Empty
    if ([guid]::TryParse($Flight, [ref]$flightGuid)) {
      throw "Package flight ID '$Flight' no longer exists. Enter a flight name to create a new flight through the API."
    }
    $groups = @($FlightGroupIds | ForEach-Object { $_.Split(',') } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    if (-not $groups) {
      throw "Creating package flight '$Flight' requires at least one tester group ID. Set the workflow's flight_group_ids input or pass -FlightGroupIds. Existing flights keep their groups."
    }
    Write-Host "Creating package flight '$Flight' through the API"
    $match = Call Post "$api/flights" @{ friendlyName = $Flight; groupIds = $groups }
    if (-not $match.flightId) { throw "The Store did not return an ID for the new package flight." }
  }
  Write-Host "Submitting to package flight '$($match.friendlyName)' ($($match.flightId))"
  $base = "$api/flights/$($match.flightId)"
  $packagesField = "flightPackages"
  $pendingField = "pendingFlightSubmission"
}

# Only one pending submission may exist, so drop a leftover draft first
$current = Call Get $base
if ($current.$pendingField) {
  Write-Host "Deleting pending submission $($current.$pendingField.id)"
  try {
    Call Delete "$base/submissions/$($current.$pendingField.id)" | Out-Null
  } catch {
    if ($_.ErrorDetails.Message -match 'submissions that are created through the API') {
      $target = if ($Flight) { "package flight '$($match.friendlyName)'" } else { "app" }
      $recovery = if ($Flight) {
        "Delete only the in-progress submission in Partner Center, keeping the flight and its tester groups, then rerun this workflow. Alternatively, use a new flight name so the workflow creates it through the API."
      } else {
        "Delete the in-progress app submission in Partner Center, then rerun this workflow."
      }
      throw "Partner Center owns pending submission $($current.$pendingField.id) for this $target, so the API cannot delete or update it. $recovery Do not edit API-created submissions in Partner Center."
    }
    throw
  }
}

$submission = Call Post "$base/submissions"
Write-Host "Created submission $($submission.id)"

# Replace the previous packages with these
$packages = @($submission.$packagesField | Where-Object { $_ })
foreach ($package in $packages) { $package.fileStatus = "PendingDelete" }
foreach ($file in $Msix) {
  $packages += [pscustomobject]@{
    fileName              = Split-Path $file -Leaf
    fileStatus            = "PendingUpload"
    minimumDirectXVersion = "None"
    minimumSystemRam      = "None"
  }
}
$submission.$packagesField = $packages
$uploadUrl = $submission.fileUploadUrl
Call Put "$base/submissions/$($submission.id)" $submission | Out-Null

# The packages go up as one zip with the files at its root
$zip = Join-Path ([IO.Path]::GetTempPath()) "msstore-upload.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Msix -DestinationPath $zip
Write-Host "Uploading $(($Msix | ForEach-Object { Split-Path $_ -Leaf }) -join ', ')"
Invoke-RestMethod -Method Put -Uri $uploadUrl -InFile $zip -Headers @{ "x-ms-blob-type" = "BlockBlob" } -ContentType "application/octet-stream" | Out-Null

if ($NoCommit) {
  Write-Host "Left submission $($submission.id) as a draft"
  exit 0
}

Call Post "$base/submissions/$($submission.id)/commit" | Out-Null
do {
  Start-Sleep -Seconds 15
  $status = Call Get "$base/submissions/$($submission.id)/status"
  Write-Host "Status: $($status.status)"
} while ($status.status -eq "CommitStarted")

if ($status.status -eq "CommitFailed") {
  $status.statusDetails | ConvertTo-Json -Depth 8 | Write-Host
  throw "Store submission $($submission.id) failed to commit"
}
Write-Host "Submission $($submission.id) is in certification"
