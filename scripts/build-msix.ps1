# Builds dist/blockwork-windows-<arch>.msix from an existing release build.
# The Store signs the package on publish, so it is left unsigned here.
#
# Usage: scripts/build-msix.ps1 -Version 0.5.4 [-Target x86_64-pc-windows-msvc] [-Arch x64]
param(
  [Parameter(Mandatory)][string]$Version,
  [string]$Target = "x86_64-pc-windows-msvc",
  [string]$Arch = "x64"
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$release = Join-Path $root "target/$Target/release"
$stage = Join-Path $root "dist/msix-stage"
$out = Join-Path $root "dist/blockwork-windows-$(if ($Arch -eq 'x64') { 'x86_64' } else { $Arch }).msix"

# MSIX versions are a.b.c.d and the Store reserves the last part
if ($Version -notmatch '^v?(\d+)\.(\d+)\.(\d+)') { throw "Bad version '$Version'" }
$msixVersion = "$($Matches[1]).$($Matches[2]).$($Matches[3]).0"

Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
& (Join-Path $PSScriptRoot "deploy-qt-windows.ps1") -Target $Target -Destination $stage
New-Item -ItemType Directory -Path "$stage/Assets" -Force | Out-Null

# Store logos, resized from the app icon
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Image]::FromFile((Join-Path $root "res/icons/blockwork.png"))
foreach ($logo in @{ "StoreLogo" = 50; "Square44x44Logo" = 44; "Square150x150Logo" = 150 }.GetEnumerator()) {
  $size = $logo.Value
  $bmp = New-Object System.Drawing.Bitmap $size, $size
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $g.DrawImage($icon, 0, 0, $size, $size)
  $g.Dispose()
  $bmp.Save((Join-Path $stage "Assets/$($logo.Key).png"), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}
$icon.Dispose()

(Get-Content (Join-Path $root "packaging/msix/AppxManifest.xml") -Raw).
  Replace("__VERSION__", $msixVersion).
  Replace("__ARCH__", $Arch) | Set-Content (Join-Path $stage "AppxManifest.xml") -NoNewline

$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
  Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found (install the Windows SDK)" }

& $makeappx.FullName pack /d $stage /p $out /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "Built $out"
