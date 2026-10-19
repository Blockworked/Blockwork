name := "blockwork"
appid := "com.blockworked.Blockwork"

set allow-duplicate-variables

[unix]
set shell := ["sh", "-cu"]

[windows]
set shell := ["cmd.exe", "/Q", "/C"]

[unix]
TARGET := "target/release/blockwork"

[windows]
TARGET := "target/release/blockwork.exe"

[unix]
DAEMON := "target/release/blockwork-daemon"

[windows]
DAEMON := "target/release/blockwork-daemon.exe"

LIBDIR := "/usr/lib/blockwork"

mkdir-cargo := if os() == "windows" { 'if not exist .cargo mkdir .cargo' } else { 'mkdir -p .cargo' }
rm-cargo-cfg := if os() == "windows" { 'if exist .cargo\config.toml (del /F /Q .cargo\config.toml)' } else { 'rm -f .cargo/config.toml' }

default: build

# Build everything. The UI starts `blockwork-daemon` from beside itself,
# so the whole workspace has to be built, not just the default members.
# blockwork-linux-bridge is Linux-only and ships separately (see packaging).
build *args:
    cargo build --release --workspace --exclude blockwork-linux-bridge {{args}}

run: build
    {{TARGET}}

test:
    cargo test --workspace --exclude blockwork-linux-bridge

clean:
    cargo clean

# Refresh blockwork-qt/i18n/*.ts from the QML (this repo's and blockstitch's)
# and compile them to the .qm files the build embeds. Needs Qt's linguist tools.
translations blockstitch="../blockstitch":
    #!/bin/sh
    set -eu
    bin=$(qtpaths6 --query QT_HOST_BINS 2>/dev/null || qtpaths --query QT_HOST_BINS 2>/dev/null || echo /usr/lib/qt6/bin)
    "$bin/lupdate" -silent blockwork-qt/qml "{{blockstitch}}/qml/Blockstitch" -extensions qml -no-obsolete -ts $(ls blockwork-qt/i18n/qml_*.ts | grep -v qml_en.ts)
    "$bin/lupdate" -silent -pluralonly blockwork-qt/qml "{{blockstitch}}/qml/Blockstitch" -extensions qml -no-obsolete -ts blockwork-qt/i18n/qml_en.ts
    "$bin/lrelease" -silent blockwork-qt/i18n/qml_*.ts

# Work on blockstitch locally: point cargo's blockstitch-core at the same
# checkout, without touching the committed lockfile.
blockstitch-local path="../blockstitch":
    {{mkdir-cargo}}
    {{ if os() == "windows" { 'echo paths = ["' + replace(clean(justfile_directory() / path), "\\", "/") + '/crates/blockstitch-core"] > .cargo\config.toml' } else { "printf 'paths = [\"%s/crates/blockstitch-core\"]\\n' \"$(realpath " + path + ")\" > .cargo/config.toml" } }}

blockstitch-published commit="":
    {{rm-cargo-cfg}}
    {{ if os() == "windows" { 'if not "' + commit + '"=="" (node scripts/set-blockstitch-rev.js ' + commit + ')' } else { 'if [ -n "' + commit + '" ]; then node scripts/set-blockstitch-rev.js ' + commit + '; fi' } }}
    cargo fetch

[linux]
install:
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/blockwork
    sudo install -Dm0755 {{DAEMON}} {{LIBDIR}}/blockwork-daemon
    sudo ln -sf {{LIBDIR}}/blockwork /usr/bin/blockwork
    sudo install -Dm0644 res/blockwork.desktop /usr/share/applications/blockwork.desktop
    sudo install -Dm0644 res/icons/blockwork.png /usr/share/icons/hicolor/256x256/apps/blockwork.png

[linux]
uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/blockwork /usr/share/applications/blockwork.desktop /usr/share/icons/hicolor/256x256/apps/blockwork.png

[windows]
install:
    @echo "Windows installs go through the MSIX - see just msix-install."

[windows]
uninstall:
    @echo "Windows uninstalls remove the MSIX package under Settings - Apps."

[macos]
install:
    @echo "macOS installs go through the app bundle - see just macos-install."

[macos]
uninstall:
    sudo rm -rf "/Applications/Blockwork.app"

# Rebuild and reinstall the editor and daemon.
replace: build uninstall install

# Reinstall over the current install without removing it first.
[linux]
replace-fast: build install

flatpak-sources:
    flatpak run --command=flatpak-cargo-generator org.flatpak.Builder -o packaging/flatpak/cargo-sources.json Cargo.lock

flatpak-build *args:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -f ../blockstitch/Cargo.toml ]]; then
        echo "error: the sibling ../blockstitch QML module is required" >&2
        exit 1
    fi
    flatpak run --command=flatpak-cargo-generator org.flatpak.Builder \
        -o packaging/flatpak/cargo-sources.json Cargo.lock
    awk '
      /# BEGIN app-source/ { print; print "      - type: dir"; print "        path: ../.."; print "        skip: [target, .git, .flatpak-builder, flatpak-build, flatpak-repo, packaging/flatpak]"; skip = 1; next }
      /# END app-source/ { skip = 0; print; next }
      /# BEGIN blockstitch-source/ { print; print "      - type: dir"; print "        path: ../../../blockstitch"; print "        dest: .flatpak-blockstitch"; print "        skip: [target, .git, node_modules, dist]"; skip = 1; next }
      /# END blockstitch-source/ { skip = 0; print; next }
      !skip
    ' packaging/flatpak/{{appid}}.yml > packaging/flatpak/{{appid}}.local.yml
    flatpak-builder --force-clean --user --disable-rofiles-fuse --repo=flatpak-repo flatpak-build packaging/flatpak/{{appid}}.local.yml {{args}}

flatpak-lint:
    #!/usr/bin/env bash
    status=0
    for check in "appstream res/{{appid}}.metainfo.xml" "manifest packaging/flatpak/{{appid}}.yml" "repo flatpak-repo"; do
        echo "==> flatpak-builder-lint $check"
        flatpak run --command=flatpak-builder-lint org.flatpak.Builder $check || status=1
    done
    exit $status

flatpak-install:
    flatpak remote-add --user --if-not-exists --no-gpg-verify blockwork-local flatpak-repo
    flatpak install --user -y --reinstall blockwork-local {{appid}}

flatpak-test *args: (flatpak-build args) flatpak-install
    flatpak run {{appid}}

macos-install *args:
    @if [ "$(uname)" != "Darwin" ]; then echo "error: macos-install only works on macOS" >&2; exit 1; fi
    ./scripts/build-macos-bundle.sh {{args}}
    sudo rm -rf "/Applications/Blockwork.app"
    sudo ditto "dist/Blockwork.app" "/Applications/Blockwork.app"
    sudo xattr -dr com.apple.quarantine "/Applications/Blockwork.app" || true
    @echo "Installed to /Applications/Blockwork.app"

# Windows only: build the Qt UI, deploy its runtime, and pack an unsigned MSIX.
msix target="x86_64-pc-windows-msvc" arch="x64":
    #!pwsh
    $ErrorActionPreference = "Stop"
    cargo build --release --target {{target}} --workspace --exclude blockwork-linux-bridge
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    $version = (Select-String -Path Cargo.toml -Pattern '^version = "(.*)"').Matches[0].Groups[1].Value
    ./scripts/build-msix.ps1 -Version $version -Target {{target}} -Arch {{arch}}

# Windows only: build the MSIX, sign it with a test cert and install it. The
# cert import targets the LocalMachine store, so it needs elevation; this is
# handled automatically via Windows sudo (11 24H2+) when run un-elevated.
msix-install: msix
    #!pwsh
    $ErrorActionPreference = "Stop"
    $isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $isAdmin) {
        if (-not (Get-Command sudo.exe -ErrorAction SilentlyContinue)) {
            throw "sudo.exe not found, so elevation is unavailable; run `just msix-install` from an elevated shell instead"
        }
        Write-Host "Elevating via sudo to install the MSIX..."
        sudo.exe pwsh -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath
        exit $LASTEXITCODE
    }
    ./scripts/sign-msix-test.ps1 -Msix dist/blockwork-windows-x86_64.msix
    Import-Certificate -FilePath dist/blockwork-msix-test.cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople | Out-Null
    Get-AppxPackage -Name "Blockworked.Blockwork" | Remove-AppxPackage
    Add-AppxPackage -Path dist/blockwork-windows-x86_64.msix -ForceUpdateFromAnyVersion

flatpak-uninstall:
    flatpak uninstall --user -y {{appid}}
