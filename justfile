name := "blockwork"
appid := "com.blockworked.Blockwork"

set shell := ["sh", "-cu"]
set windows-shell := ["cmd.exe", "/Q", "/C"]

# Qt 6.10 needs the ARM intrinsic declaration with Apple Clang 21.
# cc-rs applies this only to C++ targeting Apple Silicon macOS.
export CXXFLAGS_aarch64_apple_darwin := env_var_or_default("CXXFLAGS_aarch64_apple_darwin", "") + " -include arm_acle.h"
export MACOSX_DEPLOYMENT_TARGET := env_var_or_default("MACOSX_DEPLOYMENT_TARGET", "13.0")

TARGET := if os() == "windows" { 'target\release\blockwork.exe' } else { "target/release/blockwork" }
DAEMON := if os() == "windows" { 'target\release\blockwork-daemon.exe' } else { "target/release/blockwork-daemon" }
AGENT := if os() == "windows" { 'target\release\blockwork-agent.exe' } else { "target/release/blockwork-agent" }

LIBDIR := "/usr/lib/blockwork"

mkdir-cargo := if os() == "windows" { 'if not exist .cargo mkdir .cargo' } else { 'mkdir -p .cargo' }
rm-cargo-cfg := if os() == "windows" { 'if exist .cargo\config.toml (del /F /Q .cargo\config.toml)' } else { 'rm -f .cargo/config.toml' }

default: build

# Build everything. The UI starts `blockwork-daemon` from beside itself,
# so the whole workspace has to be built, not just the default members.
# blockwork-linux-bridge is Linux-only and ships with the Geometry Dash mod
# (blockwork-ffi/build-windows.sh builds it).
# qt-version only accepts x.y.0 releases, so a distro Qt like 6.12.1 is skipped
# for a Qt minimal download that QWindowKit can't build against. Prefer qmake6.
[unix]
build *args:
    #!/bin/sh
    set -eu
    q="${QMAKE:-$(command -v qmake6 || true)}"
    if [ -n "$q" ]; then export QMAKE="$q"; fi
    cargo build --release --workspace --exclude blockwork-linux-bridge {{args}}

[windows]
build *args:
    cargo build --release --workspace --exclude blockwork-linux-bridge {{args}}

[linux]
run: build kwin-plugin
    "{{TARGET}}"

[macos]
run: build
    "{{TARGET}}"

# The KWin plugin behind the In window block's background input. Skips with a
# note when KWin's dev files are missing; installing it asks for sudo.
[linux]
kwin-plugin:
    #!/bin/sh
    set -u
    if ! command -v cmake >/dev/null || [ ! -d /usr/share/ECM ] || ! ls /usr/lib*/cmake/KWin/KWinConfig.cmake >/dev/null 2>&1; then
        echo "Skipping the KWin plugin: needs cmake, extra-cmake-modules and KWin's dev files"
        exit 0
    fi
    if ! { cmake -S blockwork-kwin -B target/kwin-plugin -DCMAKE_BUILD_TYPE=Release >/dev/null && cmake --build target/kwin-plugin; }; then
        echo "KWin plugin build failed: background input stays off, see the output above" >&2
        exit 0
    fi
    built=target/kwin-plugin/blockworkseathost.so
    plugins="$(qtpaths6 --query QT_INSTALL_PLUGINS 2>/dev/null || echo /usr/lib/qt6/plugins)/kwin/plugins"
    changed=0
    if ! cmp -s "$built" "$plugins/blockworkseathost.so"; then
        echo "Installing the KWin plugin to $plugins"
        sudo install -Dm755 "$built" "$plugins/blockworkseathost.so" || { echo "KWin plugin not installed: rerun and allow sudo" >&2; exit 0; }
        changed=1
    fi
    # Only a running KWin can load it; it can't unload an old copy.
    loaded="$(qdbus6 org.kde.KWin /Plugins org.kde.KWin.Plugins.LoadedPlugins 2>/dev/null || true)"
    if [ -z "$loaded" ]; then
        exit 0
    elif echo "$loaded" | grep -qx blockworkseathost; then
        if [ "$changed" = 1 ]; then echo "KWin still runs the old plugin: log out and back in to load the new one"; else echo "KWin plugin up to date and loaded"; fi
    else
        qdbus6 org.kde.KWin /Plugins org.kde.KWin.Plugins.LoadPlugin blockworkseathost >/dev/null
        if qdbus6 org.kde.KWin /Plugins org.kde.KWin.Plugins.LoadedPlugins | grep -qx blockworkseathost; then
            echo "KWin plugin loaded"
        else
            echo "KWin didn't load the plugin: log out and back in, then check journalctl --user -b | grep -i kwin" >&2
        fi
    fi

# Stage the Qt DLLs and QML plugins beside both executables before launching.
[windows]
run: build
    pwsh -NoProfile -File scripts/deploy-qt-windows.ps1 -ReleaseDirectory target/release -Destination dist/windows-local
    dist\windows-local\blockwork.exe

test:
    cargo test --workspace --exclude blockwork-linux-bridge

# QML tests against the pinned blockstitch (fetched into ../blockstitch if missing).
test-qml blockstitch="../blockstitch" *filters="":
    [ -d "{{blockstitch}}/.git" ] || scripts/fetch-blockstitch.sh "{{blockstitch}}"
    python3 blockwork-qt/tests/run.py "{{blockstitch}}" {{filters}}

# Window behavior through QWindowKit's real event filters.
test-window *filters="":
    python3 blockwork-qt/window-tests/run.py {{filters}}

# Editor timings with a big macro (page, load, edit, drag, zoom), offscreen.
# Prints PERF lines; pass a blockstitch checkout and optional test filters.
bench-qml blockstitch="../blockstitch" *filters="":
    blockwork-qt/tests/perf/run.sh "{{blockstitch}}" {{filters}}

# Run a command (default: the live screen tests) on a headless desktop with
# a stand-in game window. See docs/mock-desktop.md.
[linux]
mock-desktop desktop="x11" *cmd="":
    scripts/mock-desktop.sh run {{desktop}} {{cmd}}

clean:
    cargo clean

# Refresh blockwork-qt/i18n/*.ts from the QML (this repo's and blockstitch's)
# and compile them to the .qm files the build embeds. Needs Qt's linguist tools.
translations blockstitch="../blockstitch":
    #!/bin/sh
    set -eu
    # The catalogs hold blockstitch's strings too, so read the pinned commit.
    pin=$(sed -n 's/^blockstitch-core *=.*rev *= *"\([0-9a-f]*\)".*/\1/p' Cargo.toml)
    head=$(git -C "{{blockstitch}}" rev-parse HEAD 2>/dev/null || echo none)
    if [ ! -f .cargo/config.toml ] && [ "${BLOCKWORK_ANY_BLOCKSTITCH:-}" != 1 ] && [ "$head" != "$pin" ]; then
        echo "{{blockstitch}} is at $head, Cargo.toml pins $pin (scripts/fetch-blockstitch.sh, or BLOCKWORK_ANY_BLOCKSTITCH=1)" >&2
        exit 1
    fi
    bin=$(qtpaths6 --query QT_HOST_BINS 2>/dev/null || qtpaths --query QT_HOST_BINS 2>/dev/null || echo /usr/lib/qt6/bin)
    "$bin/lupdate" -silent blockwork-qt/qml "{{blockstitch}}/qml/Blockstitch" -extensions qml -no-obsolete -ts $(ls blockwork-qt/i18n/qml_*.ts | grep -v qml_en.ts)
    "$bin/lupdate" -silent -pluralonly blockwork-qt/qml "{{blockstitch}}/qml/Blockstitch" -extensions qml -no-obsolete -ts blockwork-qt/i18n/qml_en.ts
    "$bin/lrelease" -silent blockwork-qt/i18n/qml_*.ts

# Work on blockstitch locally: point cargo's QML, core and FFI crates at the
# same checkout, without touching the committed lockfile.
blockstitch-local path="../blockstitch":
    {{mkdir-cargo}}
    {{ if os() == "windows" { 'echo paths = ["' + replace(clean(justfile_directory() / path), "\\", "/") + '", "' + replace(clean(justfile_directory() / path), "\\", "/") + '/crates/blockstitch-core", "' + replace(clean(justfile_directory() / path), "\\", "/") + '/crates/blockstitch-ffi"] > .cargo\config.toml' } else { "printf 'paths = [\"%s\", \"%s/crates/blockstitch-core\", \"%s/crates/blockstitch-ffi\"]\\n' \"$(realpath " + path + ")\" \"$(realpath " + path + ")\" \"$(realpath " + path + ")\" > .cargo/config.toml" } }}

blockstitch-published commit="":
    {{rm-cargo-cfg}}
    {{ if os() == "windows" { 'if not "' + commit + '"=="" (node scripts/set-blockstitch-rev.js ' + commit + ')' } else { 'if [ -n "' + commit + '" ]; then node scripts/set-blockstitch-rev.js ' + commit + '; fi' } }}
    cargo fetch

[linux]
install: build
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/blockwork
    sudo install -Dm0755 {{DAEMON}} {{LIBDIR}}/blockwork-daemon
    sudo install -Dm0755 {{AGENT}} {{LIBDIR}}/blockwork-agent
    sudo install -Dm0644 res/licenses/QWindowKit-LICENSE {{LIBDIR}}/licenses/QWindowKit-LICENSE
    sudo ln -sf {{LIBDIR}}/blockwork /usr/bin/blockwork
    sudo ln -sf {{LIBDIR}}/blockwork-agent /usr/bin/blockwork-agent
    sudo install -Dm0644 res/blockwork.desktop /usr/share/applications/blockwork.desktop
    sudo install -Dm0644 res/icons/blockwork.png /usr/share/icons/hicolor/256x256/apps/blockwork.png

[linux]
uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/blockwork /usr/bin/blockwork-agent /usr/share/applications/blockwork.desktop /usr/share/icons/hicolor/256x256/apps/blockwork.png

[windows]
install: msix-install

[windows]
uninstall:
    @echo "Windows uninstalls remove the MSIX package under Settings - Apps."

[macos]
install: macos-install

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

# Set the version, date the changelog, update the metainfo and Flatpak files,
# then commit everything and tag it: `just release 0.6.4`. Pushes nothing.
release version:
    python3 scripts/release.py {{version}}

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
