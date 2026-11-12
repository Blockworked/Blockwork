#!/usr/bin/env bash
# Live touch checks: records gestures from the mock desktop's fake multitouch
# screen and plays touch paths into its fake /dev/uinput. Needs no kernel
# input devices; see scripts/mock-desktop/fake-evdev.c.
# Usage: scripts/test-touch.sh [cargo test args...]
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [[ -z "${DISPLAY:-}" ]]; then
    command -v xvfb-run >/dev/null || { echo 'Install Xvfb/xauth or set DISPLAY to a test desktop.' >&2; exit 1; }
    exec xvfb-run -a -s "-screen 0 1920x1080x24" "$0" "$@"
fi
work="$(mktemp -d)"
trap 'kill "${broker:-}" 2>/dev/null || true; rm -rf "$work"' EXIT
cc -shared -fPIC -O2 -o "$work/fake-evdev.so" "$root/scripts/mock-desktop/fake-evdev.c" -ldl
export FAKE_EVDEV_DIR="$work/evdev"
# The broker stays off the X server so the fake screen only reaches readers.
env -u DISPLAY python3 "$root/scripts/mock-desktop/fake-input.py" serve "$FAKE_EVDEV_DIR" &
broker=$!
for _ in $(seq 50); do [[ -S "$FAKE_EVDEV_DIR/control.sock" ]] && break; sleep 0.1; done
cd "$root"
cargo test -p blockwork-core --test touch_live --no-run
LD_PRELOAD="$work/fake-evdev.so" cargo test -p blockwork-core --test touch_live -- --ignored --nocapture --test-threads=1 "$@"
