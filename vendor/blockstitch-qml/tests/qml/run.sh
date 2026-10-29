#!/usr/bin/env bash
# Runs the QML tests against qml/Blockstitch, laid out as qml/module.json says.
# Usage: tests/qml/run.sh [qmltestrunner]; QMLTESTRUNNER works too.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
temp="$(mktemp -d)"
trap 'rm -rf "$temp"' EXIT
module="$temp/com/blockworked/Blockstitch"
mkdir -p "$module"
cp -R "$root"/qml/Blockstitch/. "$module"/
python3 - "$root/qml/module.json" > "$module/qmldir" <<'PY'
import json, sys
m = json.load(open(sys.argv[1]))
print("module " + m["uri"])
for f in m["files"]:
    print(("singleton " if f in m["singletons"] else "") + f"{f[:-4]} {m['version']} {f}")
PY
QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}" QML_DISABLE_DISK_CACHE=1 \
    "${1:-${QMLTESTRUNNER:-qmltestrunner}}" -import "$temp" -input "$root/tests/qml"
