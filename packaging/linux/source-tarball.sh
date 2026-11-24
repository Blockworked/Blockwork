#!/usr/bin/env bash
# Makes a source tarball that builds with no network: the checked-out tree,
# every crate vendored (blockstitch included) and the QWindowKit source.
# usage: packaging/linux/source-tarball.sh [OUT_DIR=dist]
# QWINDOWKIT_ARCHIVE names an already downloaded QWindowKit archive (checked).
# Prints the tarball path. Build it with CARGO_NET_OFFLINE=true and
# BLOCKWORK_QWINDOWKIT_SOURCE_DIR=$PWD/third_party/qwindowkit.
set -euo pipefail
cd "$(dirname "$0")/../.."
out="$(mkdir -p "${1:-dist}" && cd "${1:-dist}" && pwd)"
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
name="blockwork-$version"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

git archive --format=tar --prefix="$name/" HEAD | tar -x -C "$work"
mkdir -p "$work/$name/.cargo"
# Vendor from inside the copy so config.toml points at the relative "vendor".
(cd "$work/$name" && cargo vendor --locked --versioned-dirs vendor >.cargo/config.toml)

# The same QWindowKit archive blockwork-qt/native/CMakeLists.txt fetches.
url=$(grep -oE 'https://codeload.github.com/stdware/qwindowkit/[^ ]+' blockwork-qt/native/CMakeLists.txt)
sha=$(grep -oE 'SHA256=[0-9a-f]+' blockwork-qt/native/CMakeLists.txt | cut -d= -f2)
if [ -n "${QWINDOWKIT_ARCHIVE:-}" ]; then
    cp "$QWINDOWKIT_ARCHIVE" "$work/qwindowkit.tar.gz"
else
    curl -fsSL "$url" -o "$work/qwindowkit.tar.gz"
fi
echo "$sha  $work/qwindowkit.tar.gz" | sha256sum -c - >/dev/null
mkdir -p "$work/$name/third_party/qwindowkit"
tar -xzf "$work/qwindowkit.tar.gz" -C "$work/$name/third_party/qwindowkit" --strip-components=1

XZ_OPT="${XZ_OPT:--T0}" tar -C "$work" --sort=name --owner=0 --group=0 --numeric-owner -cJf "$out/$name.tar.xz" "$name"
echo "$out/$name.tar.xz"
