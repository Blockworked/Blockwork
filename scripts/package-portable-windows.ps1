# Zips a staged Windows app (scripts/deploy-qt-windows.ps1) as the portable
# download: one top-level Blockwork folder holding the exe, the Qt runtime and
# the MSVC runtime, so it runs straight after unzipping.
# Usage: scripts/package-portable-windows.ps1 -Target x86_64-pc-windows-msvc -Output blockwork-windows-x86_64.zip
param(
  [string]$Target = "x86_64-pc-windows-msvc",
  [Parameter(Mandatory = $true)][string]$Output
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$staged = Join-Path $root "dist/windows-$Target"
foreach ($required in @("blockwork.exe", "blockwork-daemon.exe", "Qt6Core.dll", "Qt6Quick.dll", "platforms/qwindows.dll", "vcruntime140.dll")) {
  if (-not (Test-Path (Join-Path $staged $required))) {
    throw "$required is missing from $staged; run scripts/deploy-qt-windows.ps1 first"
  }
}

$work = Join-Path $root "dist/portable-$Target"
$folder = Join-Path $work "Blockwork"
Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $work | Out-Null
Copy-Item -LiteralPath $staged -Destination $folder -Recurse
Set-Content -LiteralPath (Join-Path $folder "README.txt") -Encoding utf8 -Value @(
  "Blockwork (portable)",
  "",
  "Run blockwork.exe. Keep every file in this folder together:",
  "the DLLs and subfolders are the Qt runtime Blockwork needs.",
  "",
  "For Start menu shortcuts and automatic updates, use the installer",
  "(blockwork-windows-<arch>-setup.exe) from https://blockworked.com instead."
)

$Output = [IO.Path]::GetFullPath($Output, (Get-Location).Path)
Remove-Item -LiteralPath $Output -Force -ErrorAction SilentlyContinue
Compress-Archive -Path $folder -DestinationPath $Output
# Unzip it somewhere fresh and start it with only Windows itself on PATH, so a
# DLL missing from the zip can't be found in the Qt or Rust install instead.
$check = Join-Path $work "check"
Expand-Archive -LiteralPath $Output -DestinationPath $check
$exe = Join-Path $check "Blockwork/blockwork.exe"
$savedPath = $env:PATH
try {
  $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
  $process = Start-Process -FilePath $exe -ArgumentList "--help" -NoNewWindow -PassThru
  $null = $process.Handle  # keeps ExitCode readable after the process exits
  if (-not $process.WaitForExit(60000)) {
    $process.Kill()
    throw "The unzipped blockwork.exe --help did not exit within 60 s"
  }
  if ($process.ExitCode -ne 0) {
    throw ("The unzipped blockwork.exe --help exited with 0x{0:X8} (0xC0000135 means a DLL is missing)" -f $process.ExitCode)
  }
} finally {
  $env:PATH = $savedPath
}

Write-Host "Portable zip written to $Output"
