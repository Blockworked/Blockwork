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
if (-not $qmakePath) { throw "qmake was not found; set QMAKE to the Qt 6 qmake executable" }
$deploy = Join-Path (Split-Path $qmakePath) "windeployqt.exe"
if (-not (Test-Path $deploy)) { throw "windeployqt.exe was not found next to $qmakePath" }
$qtRoot = Split-Path (Split-Path $qmakePath)

Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $Destination | Out-Null
Copy-Item (Join-Path $release "blockwork.exe") $Destination
Copy-Item (Join-Path $release "blockwork-daemon.exe") $Destination

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

Write-Host "Staged Qt application at $Destination"
