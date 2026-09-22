<#
.SYNOPSIS
    Builds the Blockwork release binary and packages it into a Windows installer.

.DESCRIPTION
    Prerequisite (one-time, local machine only): install Inno Setup 6
        https://jrsoftware.org/isdl.php
    (default install path assumed below: "C:\Program Files (x86)\Inno Setup 6\ISCC.exe")

.PARAMETER Version
    Version string to stamp on the installer. Defaults to the version in Cargo.toml.

.EXAMPLE
    pwsh -File scripts/build-installer.ps1
    pwsh -File scripts/build-installer.ps1 -Version 0.4.0
#>
param([string]$Version)

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

if (-not $Version) {
    $cargoToml = Get-Content (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($cargoToml -match 'version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        $Version = "0.0.0-dev"
    }
}

Push-Location $RepoRoot
try {
    Write-Host "Building blockwork.exe (release, version $Version)..."
    cargo build --release --target x86_64-pc-windows-msvc --workspace --exclude blockwork-linux-bridge
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    & (Join-Path $PSScriptRoot "deploy-qt-windows.ps1") -Target "x86_64-pc-windows-msvc"

    $iscc = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    if (-not (Test-Path $iscc)) {
        throw "ISCC.exe not found at '$iscc'. Install Inno Setup 6: https://jrsoftware.org/isdl.php"
    }

    New-Item -ItemType Directory -Force -Path (Join-Path $RepoRoot "dist") | Out-Null

    Write-Host "Compiling installer (version $Version)..."
    & $iscc "/DMyAppVersion=$Version" (Join-Path $RepoRoot "installer\blockwork.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC.exe failed" }

    Write-Host "Installer written to dist\blockwork-windows-x86_64-setup.exe"
} finally {
    Pop-Location
}
