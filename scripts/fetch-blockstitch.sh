#!/usr/bin/env bash
# Checks out the blockstitch commit Cargo.toml pins into a folder (default
# ../blockstitch), so CI's QML tests and packages use the same one as the build.
set -euo pipefail
cd "$(dirname "$0")/.."
dest="${1:-../blockstitch}"
rev=$(sed -nE 's/^blockstitch-core *=.*rev *= *"([^"]+)".*/\1/p' Cargo.toml)
[ -n "$rev" ] || { echo "no blockstitch-core rev in Cargo.toml" >&2; exit 1; }
git init -q "$dest"
git -C "$dest" remote add origin https://github.com/Blockworked/blockstitch.git 2>/dev/null || true
git -C "$dest" fetch --depth 1 origin "$rev"
git -C "$dest" checkout -q --detach FETCH_HEAD
echo "blockstitch $rev in $dest"
