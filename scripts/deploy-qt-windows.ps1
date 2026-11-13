# Creates a self-contained Windows directory containing Blockwork, its daemon,
# and the Qt runtime selected by CXX-Qt.
param(
  [string]$Target = "x86_64-pc-windows-msvc",
  [string]$Destination = "",
  [string]$ReleaseDirectory = ""
)

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$release = if ($ReleaseDirectory) { Resolve-Path $ReleaseDirectory } else { Join-Path $root "target/$Target/release" }
if (-not $Destination) { $Destination = Join-Path $root "dist/windows-$Target" }
$Destination = [IO.Path]::GetFullPath($Destination)
$distRoot = [IO.Path]::GetFullPath((Join-Path $root "dist")) + [IO.Path]::DirectorySeparatorChar
if (-not ($Destination + [IO.Path]::DirectorySeparatorChar).StartsWith($distRoot, [StringComparison]::OrdinalIgnoreCase)) {
  throw "Destination must be inside $distRoot"
}

$qmakePath = if ($env:QMAKE) { $env:QMAKE } else { (Get-Command qmake6, qmake -ErrorAction SilentlyContinue | Select-Object -First 1).Source }
if (-not $qmakePath) {
  $minimalRoot = Join-Path $env:LOCALAPPDATA "qt_minimal_download"
  if (Test-Path $minimalRoot) {
    $qmakePath = (Get-ChildItem -LiteralPath $minimalRoot -Recurse -File -Filter qmake.exe |
      Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
  }
}
if (-not $qmakePath) { throw "qmake was not found; set QMAKE to the Qt 6 qmake executable" }
$deploy = Join-Path (Split-Path $qmakePath) "windeployqt.exe"
if (-not (Test-Path $deploy)) { throw "windeployqt.exe was not found next to $qmakePath" }
$qtRoot = Split-Path (Split-Path $qmakePath)

Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $Destination | Out-Null
Copy-Item (Join-Path $release "blockwork.exe") $Destination
Copy-Item (Join-Path $release "blockwork-daemon.exe") $Destination
New-Item -ItemType Directory -Force -Path (Join-Path $Destination "licenses") | Out-Null
Copy-Item (Join-Path $root "res/licenses/QWindowKit-LICENSE") (Join-Path $Destination "licenses/QWindowKit-LICENSE")

& $deploy --release --no-translations --qmldir (Join-Path $root "blockwork-qt/qml") (Join-Path $Destination "blockwork.exe")
if ($LASTEXITCODE -ne 0) { throw "windeployqt failed" }

# Qt Minimal intentionally omits some qmake metadata used by windeployqt to
# discover runtime-loaded plugins. Copy the precise QML and platform trees the
# embedded frontend imports, excluding debug binaries and development files.
function Copy-QtRuntimeTree([string]$Source, [string]$Target) {
  Get-ChildItem -LiteralPath $Source -Recurse -File |
    Where-Object { $_.Extension -notin @('.pdb', '.lib', '.exp') -and $_.Name -notmatch 'd\.dll$' } |
    ForEach-Object {
      $relative = [IO.Path]::GetRelativePath($Source, $_.FullName)
      $targetFile = Join-Path $Target $relative
      New-Item -ItemType Directory -Force -Path (Split-Path $targetFile) | Out-Null
      Copy-Item -LiteralPath $_.FullName -Destination $targetFile -Force
    }
}

Copy-QtRuntimeTree (Join-Path $qtRoot "plugins/platforms") (Join-Path $Destination "platforms")
Copy-QtRuntimeTree (Join-Path $qtRoot "plugins/imageformats") (Join-Path $Destination "imageformats")
foreach ($module in @('QtCore', 'QtQml', 'QtQuick')) {
  Copy-QtRuntimeTree (Join-Path $qtRoot "qml/$module") (Join-Path $Destination "qml/$module")
}
Get-ChildItem -LiteralPath (Join-Path $qtRoot "bin") -Filter '*.dll' -File |
  Where-Object { $_.Name -notmatch 'd\.dll$' } |
  Copy-Item -Destination $Destination -Force

# The MSVC runtime (vcruntime140, msvcp140, ...) isn't on every Windows
# install, so ship it app-local. Visual Studio's redist folder has the DLLs.
function Find-CompilerRuntime([string]$Arch) {
  $roots = @()
  if ($env:VCToolsRedistDir) { $roots += $env:VCToolsRedistDir }
  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
  if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vs) {
      Get-ChildItem -LiteralPath (Join-Path $vs "VC/Redist/MSVC") -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+(\.\d+)+$' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { $roots += $_.FullName }
    }
  }
  foreach ($candidate in $roots) {
    $crt = Get-ChildItem -LiteralPath (Join-Path $candidate $Arch) -Directory -Filter 'Microsoft.VC*.CRT' -ErrorAction SilentlyContinue |
      Select-Object -First 1
    if ($crt) { return $crt.FullName }
  }
  return $null
}

$crtArch = if ($Target -like "aarch64-*") { "arm64" } else { "x64" }
$crtDir = Find-CompilerRuntime $crtArch
if ($crtDir) {
  Get-ChildItem -LiteralPath $crtDir -Filter '*.dll' -File | Copy-Item -Destination $Destination -Force
  Write-Host "Bundled MSVC runtime from $crtDir"
} elseif ($env:GITHUB_ACTIONS) {
  throw "MSVC runtime DLLs for $crtArch were not found; the build would not start on a clean Windows install"
} else {
  Write-Warning "MSVC runtime DLLs for $crtArch were not found; the staged app needs the Visual C++ Redistributable installed"
}

Write-Host "Staged Qt application at $Destination"
