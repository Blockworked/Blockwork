#!/usr/bin/env bash
# Live AT-SPI checks against a GTK and a Qt fixture, each in a private
# display/session bus/config; no input device access or real desktop edits.
# Usage: scripts/test-native-ui.sh [gtk|qt|all]   (default: all)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
if [[ "${1:-}" != --inside ]]; then
    toolkits="${1:-all}"
    case "$toolkits" in gtk|qt|all) ;; *) echo "usage: $0 [gtk|qt|all]" >&2; exit 2 ;; esac
    if [[ -z "${DISPLAY:-}" ]]; then
        command -v xvfb-run >/dev/null || { echo 'Install Xvfb/xauth or set DISPLAY to a test desktop.' >&2; exit 1; }
        exec xvfb-run -a "$0" "$toolkits"
    fi
    for toolkit in $([[ "$toolkits" == all ]] && echo gtk qt || echo "$toolkits"); do
        temporary="$(mktemp -d)"
        mkdir -p "$temporary/runtime" "$temporary/config" "$temporary/cache" "$temporary/data"
        chmod 700 "$temporary/runtime"
        status=0
        env XDG_RUNTIME_DIR="$temporary/runtime" XDG_CONFIG_HOME="$temporary/config" XDG_CACHE_HOME="$temporary/cache" XDG_DATA_HOME="$temporary/data" \
            dbus-run-session -- "$0" --inside "$toolkit" || status=$?
        rm -rf "$temporary"
        [[ $status == 0 ]] || exit $status
    done
    exit
fi
toolkit="$2"
unset NO_AT_BRIDGE
export BLOCKWORK_UI_FIXTURE="$toolkit"
fixture="$(mktemp -d)"
trap 'if [[ -n "${fixture_pid:-}" ]]; then kill "$fixture_pid" 2>/dev/null || true; wait "$fixture_pid" 2>/dev/null || true; fi; rm -rf "$fixture"' EXIT
if [[ "$toolkit" == gtk ]]; then
    export GTK_MODULES=atk-bridge
    # shellcheck disable=SC2046
    cc "$root/blockwork-core/tests/fixtures/native-ui.c" -o "$fixture/native-ui" $(pkg-config --cflags --libs gtk+-3.0)
    "$fixture/native-ui" &
else
    qml="${QML:-$(command -v qml6 || command -v qml || true)}"
    [[ -n "$qml" ]] || { echo 'Install Qt 6 (qml6) or set QML to its qml runner.' >&2; exit 1; }
    QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_QPA_PLATFORM=xcb QT_QUICK_CONTROLS_STYLE=Basic \
        "$qml" "$root/blockwork-core/tests/fixtures/native-ui.qml" &
fi
fixture_pid=$!
cd "$root"
cargo test -p blockwork-core --test native_ui_live -- --ignored --nocapture --test-threads=1
