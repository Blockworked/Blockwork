#!/usr/bin/env bash
# Builds a universal (arm64 + x86_64) blockwork_ffi static library for macOS,
# for hosts built with `CMAKE_OSX_ARCHITECTURES "arm64;x86_64"`.
#
# Must run on an actual Mac (or macOS CI runner) — cross-compiling for
# Apple targets needs the Apple SDK sysroot, which can't be redistributed
# and isn't available on Linux dev boxes.
#
# Usage: blockwork-ffi/build-macos.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

for target in x86_64-apple-darwin aarch64-apple-darwin; do
    rustup target add "$target" >/dev/null
    cargo build -p blockwork-ffi --release --target "$target"
done

mkdir -p ../target/universal-macos
lipo -create -output ../target/universal-macos/libblockwork_ffi.a \
    ../target/x86_64-apple-darwin/release/libblockwork_ffi.a \
    ../target/aarch64-apple-darwin/release/libblockwork_ffi.a

echo
echo "Built: blockwork-ffi/../target/universal-macos/libblockwork_ffi.a"
echo "Header: blockwork-ffi/include/blockwork_ffi.h"
echo
echo "Pass to the host's CMake configure, e.g.:"
echo "  -DBLOCKWORK_FFI_LIB=$(cd .. && pwd)/target/universal-macos/libblockwork_ffi.a"
echo "  -DBLOCKWORK_FFI_INCLUDE_DIR=$(pwd)/include"
echo
echo "If the link step reports missing symbols, blockwork-core's macOS backend"
echo "(backend/macos.rs) may need another framework linked beyond"
echo "CoreFoundation/ApplicationServices — Security"
echo "is the most likely candidate (core-foundation/core-graphics sometimes"
echo "pull it in transitively). Verify against the actual linker error rather"
echo "than adding it speculatively."
