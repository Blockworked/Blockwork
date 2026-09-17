name := "blockwork"
appid := "com.blockworked.Blockwork"

TARGET := "target/release/blockwork"
CEF_DIR := "target/release"
LIBDIR := "/usr/lib/blockwork"

default: build

build *args:
    cargo build --release {{args}}

run: build
    {{TARGET}}

clean:
    cargo clean

blockstitch-local path="../../blockstitch":
    cd ui && npm pkg set dependencies.blockstitch="link:{{path}}" && pnpm install
    git update-index --skip-worktree ui/package.json ui/pnpm-lock.yaml

blockstitch-published commit="":
    git update-index --no-skip-worktree ui/package.json ui/pnpm-lock.yaml
    git checkout -- ui/package.json ui/pnpm-lock.yaml
    if [ -n "{{commit}}" ]; then cd ui && npm pkg set dependencies.blockstitch="github:Blockworked/blockstitch#{{commit}}"; fi
    cd ui && pnpm install

install:
    # Binary's RUNPATH is `$ORIGIN`, so the CEF runtime payload (libcef.so,
    # GL/Vulkan shims, *.pak, icudtl.dat, locales/, ...) has to live alongside
    # it in a private libdir, not /usr/bin.
    sudo install -Dm0755 {{TARGET}} {{LIBDIR}}/blockwork
    sudo install -Dm0755 {{CEF_DIR}}/blockwork-daemon {{LIBDIR}}/blockwork-daemon
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

uninstall:
    sudo rm -rf {{LIBDIR}}
    sudo rm -f /usr/bin/blockwork /usr/share/applications/blockwork.desktop /usr/share/icons/hicolor/256x256/apps/blockwork.png

replace: build uninstall install

flatpak-sources:
    flatpak run --command=flatpak-cargo-generator org.flatpak.Builder -o packaging/flatpak/cargo-sources.json Cargo.lock
    flatpak run --command=flatpak-node-generator org.flatpak.Builder pnpm ui/pnpm-lock.yaml --pnpm-store-version v11 -o packaging/flatpak/node-sources.json

flatpak-build *args:
    awk '/# BEGIN app-source/ { print; print "      - type: dir"; print "        path: ../.."; print "        skip: [target, .git, .flatpak-builder, flatpak-build, flatpak-repo, node_modules, ui/node_modules, ui/dist, packaging/flatpak]"; skip = 1; next } /# END app-source/ { skip = 0 } !skip' packaging/flatpak/{{appid}}.yml > packaging/flatpak/{{appid}}.local.yml
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

flatpak-uninstall:
    flatpak uninstall --user -y {{appid}}
