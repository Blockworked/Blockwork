#!/usr/bin/env bash
# Build the standalone fixture and the instrumentation APK for an existing host build.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
abi="${1:-x86_64}"
case "$abi" in arm64-v8a|x86_64) ;; *) echo 'Usage: build-android-validation.sh [arm64-v8a|x86_64]' >&2; exit 2 ;; esac
: "${ANDROID_SDK_ROOT:?Set ANDROID_SDK_ROOT to SDK 36}"
: "${JAVA_HOME:?Set JAVA_HOME to JDK 17}"
export PATH="$JAVA_HOME/bin:$PATH"
stage="$root/target/android/$abi/android-build"
if [[ ! -x "$stage/gradlew" ]] || ! cmp -s "$root/android/build.gradle" "$stage/build.gradle"; then
    echo "Run scripts/build-android.sh $abi first to stage the current Android package" >&2; exit 1
fi
python3 - "$root" "$stage" <<'PY'
from pathlib import Path
import sys
root, stage = map(Path, sys.argv[1:])
for source in [*(root / 'android/src').rglob('*.kt'), *(root / 'android/instrumentation').rglob('*.kt')]:
    staged = stage / source.relative_to(root / 'android')
    if not staged.exists() or staged.read_bytes() != source.read_bytes():
        raise SystemExit(f'{source.name} is not staged; rebuild the main APK first')
PY
"$stage/gradlew" --project-dir "$root/android-fixture" --console plain assembleDebug
"$stage/gradlew" --project-dir "$stage" --console plain assembleDebugAndroidTest
mkdir -p "$root/dist/android"
cp "$root/android-fixture/build/outputs/apk/debug/BlockworkFixture-debug.apk" "$root/dist/android/blockwork-fixture-debug.apk"
cp "$stage/build/outputs/apk/androidTest/debug/android-build-debug-androidTest.apk" "$root/dist/android/blockwork-$abi-androidTest.apk"
echo "Fixture and $abi instrumentation APKs: $root/dist/android"
