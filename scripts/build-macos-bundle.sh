#!/usr/bin/env bash
# Builds dist/Blockwork.app: a proper macOS app bundle with the CEF framework
# and helper processes alongside the main binary.
#
# cef-dll-sys deliberately doesn't place CEF's runtime files for you on macOS
# (unlike Linux/Windows, where it copies them next to target/<triple>/release/) --
# see the "leave it to tools like tauri-cli for now" comment in its build.rs.
# This script is that missing packaging step: it assembles the standard
# CEF/Chromium bundle layout (Contents/Frameworks/Chromium Embedded
# Framework.framework + a Helper.app per CEF subprocess type) and ad-hoc
# code-signs the result, since Apple Silicon refuses to run unsigned
# executables at all (not just a Gatekeeper warning -- see the note below).
#
# Usage: scripts/build-macos-bundle.sh [target-triple]
# Defaults to the host's native target triple.

set -euo pipefail

TARGET="${1:-$(rustc -vV | sed -n 's/host: //p')}"
case "$TARGET" in
  aarch64-apple-darwin) CEF_ARCH=aarch64 ;;
  x86_64-apple-darwin) CEF_ARCH=x86_64 ;;
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

# The cef-dll-sys build directory name is content-hash-based, and stale
# entries can accumulate across rebuilds with different CEF versions -- take
# the most recently written one.
CEF_BUILD_DIR=$(find "$RELEASE_DIR/build" -maxdepth 1 -name 'cef-dll-sys-*' -print0 2>/dev/null \
  | xargs -0 ls -dt 2>/dev/null | head -1)
if [ -z "$CEF_BUILD_DIR" ]; then
  echo "error: no cef-dll-sys build output found under $RELEASE_DIR/build" >&2
  exit 1
fi
FRAMEWORK_SRC="$CEF_BUILD_DIR/out/cef_macos_${CEF_ARCH}/Chromium Embedded Framework.framework"
if [ ! -d "$FRAMEWORK_SRC" ]; then
  echo "error: CEF framework not found at $FRAMEWORK_SRC" >&2
  exit 1
fi

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

# CEF framework: the dylib, GL/Vulkan Libraries/, and Resources/ (icudtl.dat,
# *.pak, locales/*.lproj) all travel together -- same "everything colocated"
# requirement as the Linux build, just expressed as a macOS framework bundle
# instead of a flat directory.
cp -R "$FRAMEWORK_SRC" "$CONTENTS/Frameworks/"

# Helper processes: CEF re-execs this same binary under these names to run its
# renderer/GPU/plugin/alerts subprocesses (see is_cef_helper_process() in
# tauri-runtime-cef's runtime.rs, which matches on the executable's filename
# suffix). Each needs its own .app shell so macOS treats it as a distinct
# process with its own bundle ID.
for variant in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
  helper_name="$APP_NAME Helper$variant"
  suffix=$(echo "$variant" | tr -d ' ()')
  helper_dir="$CONTENTS/Frameworks/$helper_name.app/Contents"
  mkdir -p "$helper_dir/MacOS"
  cp "$RELEASE_DIR/blockwork" "$helper_dir/MacOS/$helper_name"
  sed -e "s/__VERSION__/$VERSION/g" \
      -e "s/__HELPER_NAME__/$helper_name/g" \
      -e "s/__SUFFIX__/${suffix:+.$suffix}/g" \
      installer/macos/Info-Helper.plist.in > "$helper_dir/Info.plist"
done

# Ad-hoc code sign, innermost bundles first (macOS validates an outer bundle's
# seal without re-checking already-signed nested bundles' contents, so nested
# ones must be signed before the bundle that embeds them).
#
# This isn't optional and has nothing to do with Gatekeeper or notarization:
# on Apple Silicon the kernel refuses to exec *any* unsigned binary, even
# ad-hoc-signed ones are enough, but completely unsigned isn't. The "-"
# identity below is the free, local, ad-hoc signature -- no Apple Developer
# Program membership needed. Gatekeeper will still show an "unidentified
# developer" prompt on first launch (right-click > Open, or
# `xattr -dr com.apple.quarantine dist/Blockwork.app` after downloading);
# that's expected without a paid Developer ID for notarization.
find "$CONTENTS/Frameworks" -maxdepth 1 -name "*.app" -exec codesign --force --sign - {} \;
codesign --force --sign - "$CONTENTS/Frameworks/Chromium Embedded Framework.framework"
codesign --force --sign - "$DIST"

echo "Built $DIST"
