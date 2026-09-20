# Signs an MSIX with a throwaway self-signed cert (subject = manifest publisher) so it can be
# installed locally, and writes the matching .cer next to it. The Store re-signs release builds.
#
# Usage: scripts/sign-msix-test.ps1 -Msix dist/blockwork-windows-x86_64.msix
param([Parameter(Mandatory)][string]$Msix)

$ErrorActionPreference = "Stop"
$subject = "CN=C52AE2E4-FA2D-4A12-9608-85DF74763E33"
$cerPath = Join-Path (Split-Path $Msix) "blockwork-msix-test.cer"

$cert = New-SelfSignedCertificate -Type Custom -Subject $subject `
  -KeyUsage DigitalSignature -FriendlyName "Blockwork MSIX test" -CertStoreLocation Cert:\CurrentUser\My `
  -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
Export-Certificate -Cert $cert -FilePath $cerPath | Out-Null

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found (install the Windows SDK)" }

& $signtool.FullName sign /fd SHA256 /sha1 $cert.Thumbprint $Msix
if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
Write-Host "Signed $Msix, certificate at $cerPath"
