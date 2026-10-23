$ErrorActionPreference = 'Stop'

# The daemon keeps files locked, so stop it first
Get-Process -Name 'blockwork', 'blockwork-daemon' -ErrorAction SilentlyContinue | Stop-Process -Force

[array]$keys = Get-UninstallRegistryKey -SoftwareName 'Blockwork*'
if ($keys.Count -eq 1) {
  $packageArgs = @{
    packageName    = 'blockwork'
    fileType       = 'exe'
    silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART'
    file           = $keys[0].UninstallString -replace '^"?([^"]+)"?.*$', '$1'
    validExitCodes = @(0)
  }
  Uninstall-ChocolateyPackage @packageArgs
} elseif ($keys.Count -eq 0) {
  Write-Warning 'Blockwork is not installed, nothing to uninstall.'
} else {
  throw "Found $($keys.Count) matching uninstall entries, refusing to guess."
}
