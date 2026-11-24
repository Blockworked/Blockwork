#!/usr/bin/env bash
# Builds a .deb on Debian or Ubuntu (needs dpkg-dev).
#   build-deb.sh native  OUT_DIR [TARGET_DIR=target/release]
#       a release build linked against this distro's Qt (6.10 or newer)
#   build-deb.sh bundled OUT_DIR APPDIR
#       the AppImage's AppDir, Qt included, in /opt/blockwork (for older Qt)
# Prints the .deb path.
set -euo pipefail
cd "$(dirname "$0")/../.."
mode="${1:?native|bundled}"
out="$(mkdir -p "${2:?OUT_DIR}" && cd "$2" && pwd)"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
arch=$(dpkg --print-architecture)
. /etc/os-release
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
root="$work/root"

case "$mode" in
native)
    packaging/linux/install.sh "$root" /usr "${3:-target/release}"
    elves=("$root/usr/lib/blockwork/blockwork" "$root/usr/lib/blockwork/blockwork-daemon")
    private=()
    # Loaded at run time, so dpkg-shlibdeps can't see them.
    qml="qml6-module-qtcore, qml6-module-qtqml, qml6-module-qtqml-workerscript, qml6-module-qtquick,
 qml6-module-qtquick-controls, qml6-module-qtquick-dialogs, qml6-module-qtquick-effects,
 qml6-module-qtquick-layouts, qml6-module-qtquick-shapes, qml6-module-qtquick-templates,
 qml6-module-qtquick-window, qt6-svg-plugins"
    recommends="qt6-wayland"
    debversion="$version-1~$ID${VERSION_ID:-$VERSION_CODENAME}"
    ;;
bundled)
    appdir="$(cd "${3:?APPDIR}" && pwd)"
    install -d "$root/opt/blockwork" "$root/usr/bin"
    cp -a "$appdir/usr/." "$root/opt/blockwork/"
    ln -s /opt/blockwork/bin/blockwork "$root/usr/bin/blockwork"
    install -Dm644 res/blockwork.desktop "$root/usr/share/applications/com.blockworked.Blockwork.desktop"
    install -Dm644 res/com.blockworked.Blockwork.metainfo.xml "$root/usr/share/metainfo/com.blockworked.Blockwork.metainfo.xml"
    install -Dm644 res/icons/blockwork.png "$root/usr/share/icons/hicolor/512x512/apps/blockwork.png"
    install -Dm644 packaging/linux/60-blockwork-uinput.rules "$root/usr/lib/udev/rules.d/60-blockwork-uinput.rules"
    install -Dm644 packaging/linux/blockwork-uinput.conf "$root/usr/lib/modules-load.d/blockwork-uinput.conf"
    install -Dm644 res/licenses/QWindowKit-LICENSE "$root/usr/share/licenses/blockwork/QWindowKit-LICENSE"
    mapfile -t elves < <(find "$root/opt/blockwork" -type f \( -perm -u+x -o -name '*.so*' \) -exec sh -c 'head -c4 "$1" | grep -q ELF' _ {} \; -print)
    mapfile -t private < <(find "$root/opt/blockwork" -type f -name '*.so*' -printf '%h\n' | sort -u | sed 's/^/-l/')
    qml=""
    recommends=""
    debversion="$version-1"
    ;;
*) echo "unknown mode $mode" >&2; exit 1 ;;
esac
install -Dm644 LICENSE "$root/usr/share/doc/blockwork/copyright"

# dpkg-shlibdeps wants a debian/control beside it.
mkdir -p "$work/debian"
printf 'Source: blockwork\n\nPackage: blockwork\nArchitecture: any\n' >"$work/debian/control"
# Warnings are mostly the bundle's $ORIGIN RPATHs, which -l covers. Some
# symbols files ask for ${binary:Version}, which only means "this build".
shlibs=$(cd "$work" && dpkg-shlibdeps -O --ignore-missing-info "${private[@]}" -e"${elves[@]}" \
    2> >(grep -v 'shlibdeps: warning' >&2) \
    | sed -n 's/^shlibs:Depends=//p' | sed 's/ ([<>=]* *\${[^}]*})//g')
depends="$shlibs${qml:+, $qml}"

install -d "$root/DEBIAN"
cat >"$root/DEBIAN/control" <<CONTROL
Package: blockwork
Version: $debversion
Architecture: $arch
Maintainer: Ethan Stokes <erstokes10@gmail.com>
Installed-Size: $(du -sk "$root" --exclude=DEBIAN | cut -f1)
Depends: $depends
Recommends: ${recommends:+$recommends, }hicolor-icon-theme
Suggests: tesseract-ocr
Section: utils
Priority: optional
Homepage: https://blockworked.com
Description: Build keyboard and mouse macros by snapping blocks together
 Blockwork lets you build keyboard and mouse macros by snapping blocks
 together, without writing any code. Record your input and turn it into
 editable blocks, or assemble a sequence by hand, then play it back with
 a global hotkey.
 .
 Recording input and hotkeys need your user in the input group:
 sudo usermod -aG input \$USER
CONTROL
cat >"$root/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
if [ "$1" = configure ]; then
    # Pick up the uinput rule now instead of after a reboot.
    modprobe uinput 2>/dev/null || true
    if command -v udevadm >/dev/null 2>&1; then
        udevadm control --reload-rules 2>/dev/null || true
        udevadm trigger --subsystem-match=misc --sysname-match=uinput 2>/dev/null || true
    fi
fi
POSTINST
chmod 755 "$root/DEBIAN/postinst"

deb="$out/blockwork_${debversion}_$arch.deb"
dpkg-deb --root-owner-group -Zxz --build "$root" "$deb" >&2
echo "$deb"
