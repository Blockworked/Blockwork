$ErrorActionPreference = 'Stop'

$packageArgs = @{
  packageName    = 'blockwork'
  fileType       = 'exe'
  url64bit       = '__URL__'
  checksum64     = '__CHECKSUM__'
  checksumType64 = 'sha256'
  silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP-'
  validExitCodes = @(0)
}

Install-ChocolateyPackage @packageArgs
