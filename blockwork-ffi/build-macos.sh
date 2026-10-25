#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) blockwork_ffi static library for macOS,
# for hosts built with `CMAKE_OSX_ARCHITECTURES "arm64;x86_64"`.
#
# Must run on an actual Mac (or macOS CI runner) - cross-compiling for
# Apple targets needs the Apple SDK sysroot, which can't be redistributed
# and isn't available on Linux dev boxes.
#
# Usage: blockwork-ffi/build-macos.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"
REPO_ROOT="$(cd .. && pwd)"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"
# Cargo can reuse pure-Rust objects produced for a different deployment floor.
# Keep this embedder build separate from the Qt app's macOS 13+ cache.
BUILD_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target/macos-ffi-$MACOSX_DEPLOYMENT_TARGET}"
case "$BUILD_TARGET_DIR" in
    /*) ;;
    *) BUILD_TARGET_DIR="$REPO_ROOT/$BUILD_TARGET_DIR" ;;
esac
mkdir -p "$BUILD_TARGET_DIR"
export CARGO_TARGET_DIR="$BUILD_TARGET_DIR"

for target in x86_64-apple-darwin aarch64-apple-darwin; do
    rustup target add "$target" >/dev/null
    cargo build -p blockwork-ffi --release --locked --target "$target"
done

mkdir -p ../target/universal-macos
lipo -create -output ../target/universal-macos/libblockwork_ffi.a \
    "$BUILD_TARGET_DIR/x86_64-apple-darwin/release/libblockwork_ffi.a" \
    "$BUILD_TARGET_DIR/aarch64-apple-darwin/release/libblockwork_ffi.a"

echo
echo "Built: blockwork-ffi/../target/universal-macos/libblockwork_ffi.a"
echo "Header: blockwork-ffi/include/blockwork_ffi.h"
echo
echo "Pass to the host's CMake configure, e.g.:"
echo "  -DBLOCKWORK_FFI_LIB=$(cd .. && pwd)/target/universal-macos/libblockwork_ffi.a"
echo "  -DBLOCKWORK_FFI_INCLUDE_DIR=$(pwd)/include"
echo
echo "Deployment target: $MACOSX_DEPLOYMENT_TARGET"
echo "Native frameworks: AppKit, Foundation, CoreGraphics, ApplicationServices,"
echo "CoreFoundation, Security and IOKit (plus the standard system/Objective-C libraries)."
echo "To inspect requirements after dependency changes, run cargo rustc for"
echo "blockwork-ffi with -- --print native-static-libs."
