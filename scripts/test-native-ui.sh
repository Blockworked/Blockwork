#!/usr/bin/env bash
# A private display/session bus/config; no input device access or real desktop edits.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "${1:-}" != --inside ]]; then
    if [[ -z "${DISPLAY:-}" ]]; then
        command -v xvfb-run >/dev/null || { echo 'Install Xvfb/xauth or set DISPLAY to a test desktop.' >&2; exit 1; }
        exec xvfb-run -a "$0" "$@"
    fi
    temporary="$(mktemp -d)"
    trap 'rm -rf "$temporary"' EXIT
    mkdir -p "$temporary/runtime" "$temporary/config" "$temporary/cache" "$temporary/data"
    chmod 700 "$temporary/runtime"
    env XDG_RUNTIME_DIR="$temporary/runtime" XDG_CONFIG_HOME="$temporary/config" XDG_CACHE_HOME="$temporary/cache" XDG_DATA_HOME="$temporary/data" \
        dbus-run-session -- "$0" --inside
    exit
fi
shift
unset NO_AT_BRIDGE
export GTK_MODULES=atk-bridge
export BLOCKWORK_UI_FIXTURE=gtk
fixture="$(mktemp -d)"
trap 'if [[ -n "${fixture_pid:-}" ]]; then kill "$fixture_pid" 2>/dev/null || true; wait "$fixture_pid" 2>/dev/null || true; fi; rm -rf "$fixture"' EXIT
# shellcheck disable=SC2046
cc "$root/blockwork-core/tests/fixtures/native-ui.c" -o "$fixture/native-ui" $(pkg-config --cflags --libs gtk+-3.0)
"$fixture/native-ui" &
fixture_pid=$!
cd "$root"
cargo test -p blockwork-core --test native_ui_live -- --ignored --nocapture
