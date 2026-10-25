# Submits an MSIX to the Microsoft Store through the Partner Center submission API.
# Copies the last published submission, swaps in the new package and commits it for certification.
#
# Usage: scripts/submit-msstore.ps1 -Msix dist/blockwork-windows-x86_64.msix
# Needs MSSTORE_TENANT_ID, MSSTORE_CLIENT_ID, MSSTORE_CLIENT_SECRET and MSSTORE_APP_ID in the environment.
param(
  [Parameter(Mandatory)][string]$Msix,
  [switch]$NoCommit
)

$ErrorActionPreference = "Stop"
foreach ($name in "MSSTORE_TENANT_ID", "MSSTORE_CLIENT_ID", "MSSTORE_CLIENT_SECRET", "MSSTORE_APP_ID") {
  if (-not [Environment]::GetEnvironmentVariable($name)) { throw "$name is not set" }
}
$Msix = (Resolve-Path $Msix).Path
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

# Only one pending submission may exist, so drop a leftover draft first
$app = Call Get $api
if ($app.pendingApplicationSubmission) {
  Write-Host "Deleting pending submission $($app.pendingApplicationSubmission.id)"
  Call Delete "$api/submissions/$($app.pendingApplicationSubmission.id)" | Out-Null
}

$submission = Call Post "$api/submissions"
Write-Host "Created submission $($submission.id)"

# Replace the previous packages with this one
$packages = @($submission.applicationPackages | Where-Object { $_ })
foreach ($package in $packages) { $package.fileStatus = "PendingDelete" }
$fileName = Split-Path $Msix -Leaf
$submission.applicationPackages = $packages + [pscustomobject]@{
  fileName              = $fileName
  fileStatus            = "PendingUpload"
  minimumDirectXVersion = "None"
  minimumSystemRam      = "None"
}
$uploadUrl = $submission.fileUploadUrl
Call Put "$api/submissions/$($submission.id)" $submission | Out-Null

# The package goes up as a zip with the file at its root
$zip = Join-Path ([IO.Path]::GetTempPath()) "msstore-upload.zip"
Remove-Item $zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Msix -DestinationPath $zip
Write-Host "Uploading $fileName"
Invoke-RestMethod -Method Put -Uri $uploadUrl -InFile $zip -Headers @{ "x-ms-blob-type" = "BlockBlob" } -ContentType "application/octet-stream" | Out-Null

if ($NoCommit) {
  Write-Host "Left submission $($submission.id) as a draft"
  exit 0
}

Call Post "$api/submissions/$($submission.id)/commit" | Out-Null
do {
  Start-Sleep -Seconds 15
  $status = Call Get "$api/submissions/$($submission.id)/status"
  Write-Host "Status: $($status.status)"
} while ($status.status -eq "CommitStarted")

if ($status.status -eq "CommitFailed") {
  $status.statusDetails | ConvertTo-Json -Depth 8 | Write-Host
  throw "Store submission $($submission.id) failed to commit"
}
Write-Host "Submission $($submission.id) is in certification"
