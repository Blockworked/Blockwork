#!/usr/bin/env bash
# Builds dist/Blockwork.app and deploys the selected Qt runtime into it.
#
# Usage: scripts/build-macos-bundle.sh [target-triple]
# Defaults to the host's native target triple.

set -euo pipefail

TARGET="${1:-$(rustc -vV | sed -n 's/host: //p')}"
case "$TARGET" in
  aarch64-apple-darwin) ;;
  x86_64-apple-darwin) ;;
  *)
    echo "error: unsupported target '$TARGET' (expected aarch64-apple-darwin or x86_64-apple-darwin)" >&2
    exit 1
    ;;
esac

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
APP_NAME="Blockwork"

echo "Building $APP_NAME (release, $TARGET)..."
cargo build --release --target "$TARGET" --workspace --exclude blockwork-linux-bridge

RELEASE_DIR="target/$TARGET/release"

DIST="dist/$APP_NAME.app"
rm -rf "$DIST"
CONTENTS="$DIST/Contents"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources" "$CONTENTS/Frameworks"

echo "Assembling bundle at $DIST..."
cp "$RELEASE_DIR/blockwork" "$CONTENTS/MacOS/$APP_NAME"
# The UI starts this sibling as its long-lived background process (hotkeys,
# playback, tray icon); it has to sit next to the UI binary.
cp "$RELEASE_DIR/blockwork-daemon" "$CONTENTS/MacOS/blockwork-daemon"
sed -e "s/__VERSION__/$VERSION/g" installer/macos/Info.plist.in > "$CONTENTS/Info.plist"

# Rebuild icon.icns from res/icons/blockwork.png -- the committed
# src-tauri/icons/icon.icns is an empty stub (nothing in this repo invokes
# the Tauri CLI's own bundler, which is what normally generates it).
ICONSET_PARENT="$(mktemp -d)"
ICONSET="$ICONSET_PARENT/icon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 64 128 256 512; do
  sips -z "$size" "$size" res/icons/blockwork.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" res/icons/blockwork.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$CONTENTS/Resources/icon.icns"
rm -rf "$ICONSET_PARENT"

QMAKE_BIN="${QMAKE:-$(command -v qmake6 || command -v qmake || true)}"
if [[ -z "$QMAKE_BIN" ]]; then
  echo "error: qmake not found; set QMAKE to the Qt 6 qmake executable" >&2
  exit 1
fi
MACDEPLOYQT="$(dirname "$QMAKE_BIN")/macdeployqt"
if [[ ! -x "$MACDEPLOYQT" ]]; then
  echo "error: macdeployqt not found next to $QMAKE_BIN" >&2
  exit 1
fi
"$MACDEPLOYQT" "$DIST" -qmldir="$REPO_ROOT/blockwork-qt/qml" -always-overwrite

# Ad-hoc signing isn't optional on Apple Silicon. It has nothing to do with
# Gatekeeper or notarization:
# on Apple Silicon the kernel refuses to exec *any* unsigned binary, even
# ad-hoc-signed ones are enough, but completely unsigned isn't. The "-"
# identity below is the free, local, ad-hoc signature -- no Apple Developer
# Program membership needed. Gatekeeper will still show an "unidentified
# developer" prompt on first launch (right-click > Open, or
# `xattr -dr com.apple.quarantine dist/Blockwork.app` after downloading);
# that's expected without a paid Developer ID for notarization.
codesign --force --sign - "$DIST"

echo "Built $DIST"
