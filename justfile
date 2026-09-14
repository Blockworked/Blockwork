name := "blockwork"
appid := "dev.ethanstokes.Blockwork"

# Variables
TARGET := "target/release/blockwork"
CEF_DIR := "target/release"
LIBDIR := "/usr/lib/blockwork"

# Default target
default: build

# Build the project
build *args:
    cargo build --release {{args}}

# Run the project
run: build
    {{TARGET}}

# Clean the project
clean:
    cargo clean

# Point ui/'s blockstitch dependency at a local checkout via a pnpm `link:`
# dependency, and hide the resulting package.json/pnpm-lock.yaml changes from
# git (skip-worktree) so this machine-local switch never shows up as a diff or
# gets committed. `cargo build`/`cargo run` always run `pnpm install` (see
# src-tauri/build.rs) before compiling, in debug *and* release, so this takes
# effect immediately with no separate release-mode step.
#
# Note: `just flatpak-sources`/`flatpak-build` consume the *committed*
# ui/pnpm-lock.yaml to build fully offline -- run `just blockstitch-published`
# first if you're packaging a release while local-link mode is active.
blockstitch-local path="../../blockstitch":
    cd ui && npm pkg set dependencies.blockstitch="link:{{path}}" && pnpm install
    git update-index --skip-worktree ui/package.json ui/pnpm-lock.yaml

# Switch back to the published, pinned blockstitch dependency: un-hide
# package.json/pnpm-lock.yaml from git, restore their committed content, and
# reinstall from the pinned GitHub commit. Optionally pass a new commit hash
# (`just blockstitch-published <sha>`) to pin blockstitch to that commit
# instead of the currently committed one.
blockstitch-published commit="":
    git update-index --no-skip-worktree ui/package.json ui/pnpm-lock.yaml
    git checkout -- ui/package.json ui/pnpm-lock.yaml
    if [ -n "{{commit}}" ]; then cd ui && npm pkg set dependencies.blockstitch="github:EthanRStokes/blockstitch#{{commit}}"; fi
    cd ui && pnpm install

# Install the project
install:
    # Binary's RUNPATH is `$ORIGIN`, so the CEF runtime payload (libcef.so,
    # GL/Vulkan shims, *.pak, icudtl.dat, locales/, ...) has to live alongside
    # it in a private libdir, not /usr/bin.
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/blockwork
    sudo install -Dm0755 {{CEF_DIR}}/libcef.so {{LIBDIR}}/libcef.so
    sudo install -Dm0755 {{CEF_DIR}}/libEGL.so {{LIBDIR}}/libEGL.so
    sudo install -Dm0755 {{CEF_DIR}}/libGLESv2.so {{LIBDIR}}/libGLESv2.so
    sudo install -Dm0755 {{CEF_DIR}}/libvk_swiftshader.so {{LIBDIR}}/libvk_swiftshader.so
    sudo install -Dm0755 {{CEF_DIR}}/libvulkan.so.1 {{LIBDIR}}/libvulkan.so.1
    sudo install -Dm0755 {{CEF_DIR}}/chrome-sandbox {{LIBDIR}}/chrome-sandbox
    sudo install -Dm0644 {{CEF_DIR}}/vk_swiftshader_icd.json {{LIBDIR}}/vk_swiftshader_icd.json
    sudo install -Dm0644 {{CEF_DIR}}/icudtl.dat {{LIBDIR}}/icudtl.dat
    sudo install -Dm0644 {{CEF_DIR}}/v8_context_snapshot.bin {{LIBDIR}}/v8_context_snapshot.bin
    sudo install -Dm0644 {{CEF_DIR}}/chrome_100_percent.pak {{LIBDIR}}/chrome_100_percent.pak
    sudo install -Dm0644 {{CEF_DIR}}/chrome_200_percent.pak {{LIBDIR}}/chrome_200_percent.pak
    sudo install -Dm0644 {{CEF_DIR}}/resources.pak {{LIBDIR}}/resources.pak
    sudo rm -rf {{LIBDIR}}/locales
    sudo cp -r {{CEF_DIR}}/locales {{LIBDIR}}/locales
    sudo ln -sf {{LIBDIR}}/blockwork /usr/bin/blockwork
    sudo install -Dm0644 res/blockwork.desktop /usr/share/applications/blockwork.desktop
    sudo install -Dm0644 res/icons/blockwork.png /usr/share/icons/hicolor/256x256/apps/blockwork.png

# Uninstall the project
uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/blockwork /usr/share/applications/blockwork.desktop /usr/share/icons/hicolor/256x256/apps/blockwork.png

replace: build uninstall install

# Regenerate packaging/flatpak/{cargo,node}-sources.json from the current
# lockfiles (needs org.flatpak.Builder installed: flatpak install flathub
# org.flatpak.Builder). Re-run whenever Cargo.lock or ui/pnpm-lock.yaml changes.
flatpak-sources:
    flatpak run --command=flatpak-cargo-generator org.flatpak.Builder -o packaging/flatpak/cargo-sources.json Cargo.lock
    flatpak run --command=flatpak-node-generator org.flatpak.Builder pnpm ui/pnpm-lock.yaml --pnpm-store-version v11 -o packaging/flatpak/node-sources.json

# Build the Flatpak entirely from source inside the sandbox (Rust + frontend),
# into ./flatpak-build, exporting to a local ./flatpak-repo (both gitignored).
# Building in-sandbox -- rather than reusing a host `cargo build` -- keeps the
# binary linked against the runtime's own glibc instead of the host's, which
# matters on rolling-release distros with a newer glibc than the runtime ships.
# If flatpak-builder errors with "Failed to spawn rofiles-fuse", add
# --disable-rofiles-fuse (needed in some sandboxed/containerized dev environments
# where FUSE isn't available).
flatpak-build *args:
    flatpak-builder --force-clean --user --repo=flatpak-repo flatpak-build packaging/flatpak/{{appid}}.yml {{args}}

# Install the just-built Flatpak from the local repo, adding it as a remote
# first if needed. Re-run after every flatpak-build to pick up changes.
flatpak-install:
    flatpak remote-add --user --if-not-exists --no-gpg-verify blockwork-local flatpak-repo
    flatpak install --user -y --reinstall blockwork-local {{appid}}

# Build, install, and launch in one go -- the normal "does it still work" loop.
flatpak-test *args: (flatpak-build args) flatpak-install
    flatpak run {{appid}}

# Build dist/Blockwork.app and install it to /Applications (macOS only).
macos-install *args:
    @if [ "$(uname)" != "Darwin" ]; then echo "error: macos-install only works on macOS" >&2; exit 1; fi
    ./scripts/build-macos-bundle.sh {{args}}
    sudo rm -rf "/Applications/Blockwork.app"
    sudo ditto "dist/Blockwork.app" "/Applications/Blockwork.app"
    sudo xattr -dr com.apple.quarantine "/Applications/Blockwork.app" || true
    @echo "Installed to /Applications/Blockwork.app"

# Remove the local test install (leaves flatpak-repo/flatpak-build in place).
flatpak-uninstall:
    flatpak uninstall --user -y {{appid}}
