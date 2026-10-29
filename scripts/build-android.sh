#!/usr/bin/env bash
# Requires Qt 6.10.3 + host tools, SDK 36, NDK r27c, JDK 17, CMake and Ninja.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
abi="${1:-arm64-v8a}"
case "$abi" in
    arm64-v8a) target=aarch64-linux-android; qt_abi=android_arm64_v8a; clang_target=aarch64-linux-android ;;
    x86_64) target=x86_64-linux-android; qt_abi=android_x86_64; clang_target=x86_64-linux-android ;;
    *) echo 'Usage: build-android.sh [arm64-v8a|x86_64]' >&2; exit 2 ;;
esac
: "${QT_ROOT:?Set QT_ROOT to the directory holding Qt 6.10.3}"
: "${ANDROID_SDK_ROOT:?Set ANDROID_SDK_ROOT to SDK 36}"
: "${ANDROID_NDK_ROOT:?Set ANDROID_NDK_ROOT to NDK r27c}"
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17}"
if ! "$JAVA_HOME/bin/java" -version 2>&1 | head -1 | grep -q '"17\.'; then
    echo 'JDK 17 is required' >&2; exit 1
fi
if ! grep -q 'Pkg.Revision = 27.2.12479018' "$ANDROID_NDK_ROOT/source.properties"; then
    echo 'NDK r27c (27.2.12479018) is required' >&2; exit 1
fi
qt="$QT_ROOT/6.10.3/$qt_abi"
host="$QT_ROOT/6.10.3/gcc_64"
test -f "$qt/lib/cmake/Qt6/qt.toolchain.cmake"
test -f "$ANDROID_SDK_ROOT/platforms/android-36/android.jar"
export PATH="$JAVA_HOME/bin:$PATH"
case "$(uname -s)" in
    Linux) ndk_host=linux-x86_64 ;;
    *) echo 'This build script requires a Linux host' >&2; exit 1 ;;
esac
compiler="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/$ndk_host/bin"
rust_target="${target//-/_}"
export "CC_$rust_target=$compiler/${clang_target}30-clang"
export "AR_$rust_target=$compiler/llvm-ar"
export "CARGO_TARGET_${rust_target^^}_LINKER=$compiler/${clang_target}30-clang"
# Android 15+ can use 16 KB pages. Apply to Rust as well as the Qt executable.
export "CARGO_TARGET_${rust_target^^}_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-soname,libblockwork_android.so"
cd "$root"
cargo build -p blockwork-android --release --target "$target" --target-dir "$root/target"
runtime="$root/target/$target/release/libblockwork_android.so"
build="$root/target/android/$abi"
# Blockstitch QML is the tracked source snapshot shared with desktop.
# Other checkouts must match Cargo.lock.
BLOCKSTITCH_DIR="$root/vendor/blockstitch-qml"
if [[ -z "${BLOCKSTITCH_DIR:-}" || -z "${LUCIDE_QT_DIR:-}" ]]; then
    mkdir -p "$root/target/android"
    python3 "$root/scripts/android-deps.py" > "$root/target/android/deps-paths.txt"
    mapfile -t deps < "$root/target/android/deps-paths.txt"
    BLOCKSTITCH_DIR="${BLOCKSTITCH_DIR:-${deps[0]}}"
    LUCIDE_QT_DIR="${LUCIDE_QT_DIR:-${deps[1]}}"
fi
python3 - "$root/Cargo.lock" "$BLOCKSTITCH_DIR" "$LUCIDE_QT_DIR" <<'PY'
import pathlib, subprocess, sys, tomllib
lock = tomllib.loads(pathlib.Path(sys.argv[1]).read_text())
for package, directory in zip(['lucide-qt'], sys.argv[3:]):
    source = next(p['source'] for p in lock['package'] if p['name'] == package)
    expected = source.split('#')[-1]
    actual = subprocess.check_output(['git', '-C', directory, 'rev-parse', 'HEAD'], text=True).strip()
    if actual != expected:
        raise SystemExit(f'{package}: expected {expected}, found {actual}')
PY
cmake -S "$root/android" -B "$build" -G Ninja \
    -DCMAKE_TOOLCHAIN_FILE="$qt/lib/cmake/Qt6/qt.toolchain.cmake" \
    -DQT_HOST_PATH="$host" -DANDROID_SDK_ROOT="$ANDROID_SDK_ROOT" \
    -DANDROID_NDK_ROOT="$ANDROID_NDK_ROOT" -DANDROID_ABI="$abi" \
    -DANDROID_PLATFORM=android-30 -DCMAKE_BUILD_TYPE=Release \
    -DBLOCKSTITCH_DIR="$BLOCKSTITCH_DIR" -DLUCIDE_QT_DIR="$LUCIDE_QT_DIR" \
    -DBLOCKWORK_RUNTIME="$runtime"
mkdir -p "$root/dist/android"
# Optimize the native code but use Gradle's debug signing for an installable
# development APK. Release signing keys are deliberately outside this script.
cmake --build "$build" --target blockwork-android_prepare_apk_dir
"$host/bin/androiddeployqt" --input "$build/android-blockwork-android-deployment-settings.json" \
    --output "$build/android-build" --apk "$root/dist/android/blockwork-$abi-debug.apk" \
    --android-platform android-36 --gradle --jdk "$JAVA_HOME"
echo "APK: $root/dist/android/blockwork-$abi-debug.apk"
