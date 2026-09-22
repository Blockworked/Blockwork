name := "blockwork"
appid := "com.blockworked.Blockwork"

TARGET := "target/release/blockwork"
LIBDIR := "/usr/lib/blockwork"

default: build

build *args:
    cargo build --release {{args}}

run: build
    cargo build -p blockwork-daemon
    {{TARGET}}

clean:
    cargo clean

blockstitch-local path="../../blockstitch":
    cd ui && npm pkg set dependencies.blockstitch="link:{{path}}" && pnpm install
    git update-index --skip-worktree ui/package.json ui/pnpm-lock.yaml
    mkdir -p .cargo
    printf 'paths = ["%s/crates/blockstitch-core"]\n' "$(realpath ui/{{path}})" > .cargo/config.toml

blockstitch-published commit="":
    rm -f .cargo/config.toml
    git update-index --no-skip-worktree ui/package.json ui/pnpm-lock.yaml
    git checkout -- ui/package.json ui/pnpm-lock.yaml
    if [ -n "{{commit}}" ]; then cd ui && npm pkg set dependencies.blockstitch="github:Blockworked/blockstitch#{{commit}}"; fi
    cd ui && pnpm install
    if [ -n "{{commit}}" ]; then sed -i 's|\(blockstitch-core = { git = "https://github.com/Blockworked/blockstitch", rev = "\)[^"]*\(" }\)|\1{{commit}}\2|' Cargo.toml; fi
    cargo fetch

install:
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/blockwork
    sudo install -Dm0755 target/release/blockwork-daemon {{LIBDIR}}/blockwork-daemon
    sudo ln -sf {{LIBDIR}}/blockwork /usr/bin/blockwork
    sudo install -Dm0644 res/blockwork.desktop /usr/share/applications/blockwork.desktop
    sudo install -Dm0644 res/icons/blockwork.png /usr/share/icons/hicolor/256x256/apps/blockwork.png

uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/blockwork /usr/share/applications/blockwork.desktop /usr/share/icons/hicolor/256x256/apps/blockwork.png

replace: build uninstall install

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
      /# BEGIN app-source/ { print; print "      - type: dir"; print "        path: ../.."; print "        skip: [target, .git, .flatpak-builder, flatpak-build, flatpak-repo, node_modules, ui/node_modules, ui/dist, packaging/flatpak]"; skip = 1; next }
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
