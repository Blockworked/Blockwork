#!/usr/bin/env bash
# Installs a release build into a package root, the same way for every distro.
# usage: packaging/linux/install.sh DESTDIR [PREFIX=/usr, may be ""] [TARGET_DIR=target/release]
set -euo pipefail
cd "$(dirname "$0")/../.."
dest="${1:?DESTDIR}"
prefix="${2-/usr}"
bin="${3:-target/release}"
root="$dest$prefix"

# The UI starts the daemon from beside its own (resolved) path.
install -Dm755 "$bin/blockwork" "$root/lib/blockwork/blockwork"
install -Dm755 "$bin/blockwork-daemon" "$root/lib/blockwork/blockwork-daemon"
install -d "$root/bin"
ln -sf "$prefix/lib/blockwork/blockwork" "$root/bin/blockwork"

install -Dm644 res/blockwork.desktop "$root/share/applications/com.blockworked.Blockwork.desktop"
install -Dm644 res/com.blockworked.Blockwork.metainfo.xml "$root/share/metainfo/com.blockworked.Blockwork.metainfo.xml"
install -Dm644 res/icons/blockwork.png "$root/share/icons/hicolor/512x512/apps/blockwork.png"
install -Dm644 res/licenses/QWindowKit-LICENSE "$root/share/licenses/blockwork/QWindowKit-LICENSE"

# Playback writes to /dev/uinput; let the logged-in user do that, as steam-devices does.
install -Dm644 packaging/linux/60-blockwork-uinput.rules "$root/lib/udev/rules.d/60-blockwork-uinput.rules"
install -Dm644 packaging/linux/blockwork-uinput.conf "$root/lib/modules-load.d/blockwork-uinput.conf"
