#!/usr/bin/env bash
# Native macOS integration tests against disposable local fixtures.
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then
    echo 'This test runner requires macOS.' >&2
    exit 1
fi
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
PYTHON="${BLOCKWORK_TEST_PYTHON:-python3}"
"$PYTHON" -c 'import numpy, PIL, scipy' || {
    echo 'Install numpy, Pillow and scipy into the test Python environment first.' >&2
    exit 1
}
TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/blockwork-macos-test.XXXXXX")"
FIXTURE_PID=''
DETECTOR_PID=''
cleanup() {
    if [[ -n "$FIXTURE_PID" ]]; then kill "$FIXTURE_PID" 2>/dev/null || true; wait "$FIXTURE_PID" 2>/dev/null || true; fi
    if [[ -n "$DETECTOR_PID" ]]; then kill "$DETECTOR_PID" 2>/dev/null || true; wait "$DETECTOR_PID" 2>/dev/null || true; fi
    echo "Native test artifacts: $TEST_DIR"
}
trap cleanup EXIT
APP="$TEST_DIR/BlockworkTestWindow.app"
mkdir -p "$APP/Contents/MacOS"
swiftc -module-cache-path "$ROOT/target/macos-swift-cache" \
    scripts/macos/test-window.swift -o "$APP/Contents/MacOS/BlockworkTestWindow"
cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>BlockworkTestWindow</string>
<key>CFBundleIdentifier</key><string>com.blockworked.MacOSTestWindow</string>
<key>CFBundleName</key><string>Blockwork Test Window</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
codesign --force --sign - "$APP"
export BW_LIVE_WINDOW='Blockwork macOS Test Window'
export BLOCKWORK_TEST_EVENT_LOG="$TEST_DIR/input-events.jsonl"
export BLOCKWORK_TEST_READY_FILE="$TEST_DIR/ready"
PORT="$("$PYTHON" -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
export BLOCKWORK_TEST_DETECTOR="http://127.0.0.1:$PORT/detect"
"$APP/Contents/MacOS/BlockworkTestWindow" > "$TEST_DIR/window.log" 2>&1 &
FIXTURE_PID=$!
"$PYTHON" examples/detector/detector.py --color ff0000 --label player --port "$PORT" > "$TEST_DIR/detector.log" 2>&1 &
DETECTOR_PID=$!
# Wait for the known processes, not an arbitrary delay before running tests.
"$PYTHON" - "$PORT" "$BLOCKWORK_TEST_READY_FILE" <<'PY'
import pathlib, socket, sys, time
port, path = int(sys.argv[1]), pathlib.Path(sys.argv[2])
deadline = time.monotonic() + 15
while time.monotonic() < deadline:
    try:
        with socket.create_connection(('127.0.0.1', port), timeout=0.2):
            if path.exists():
                break
    except OSError:
        pass
    time.sleep(0.05)
else:
    raise SystemExit('Native fixture or detector did not become ready; inspect the test artifacts.')
PY
# The caller/test executable needs Accessibility and Screen Recording grants.
# Run serially because window/input tests share the disposable desktop fixture.
cargo test -p blockwork-core -p blockwork-ffi --no-default-features \
    --features blockwork-core/system "$@" -- --include-ignored --test-threads=1 --nocapture \
    2>&1 | tee "$TEST_DIR/tests.log"
