#!/usr/bin/env bash
# Editor timings: tests/perf/run.sh [blockstitch checkout] [test filters...].
# Prints PERF lines.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
# Not named tst_*, so the regular QML test run skips them.
for f in "$here"/bench_*.qml; do cp "$f" "$tmp/tst_$(basename "$f")"; done
BLOCKWORK_QML_TESTS="$tmp" python3 "$here/../run.py" "${1:-$here/../../../../blockstitch}" "${@:2}"
