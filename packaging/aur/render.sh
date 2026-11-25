#!/usr/bin/env bash
# Writes OUT_DIR/<package>/PKGBUILD for a release tag.
# usage: packaging/aur/render.sh TAG [OUT_DIR=dist/aur] [--source-only]
set -euo pipefail
cd "$(dirname "$0")/../.."
tag="${1:?TAG}"
out="$(mkdir -p "${2:-dist/aur}" && cd "${2:-dist/aur}" && pwd)"
version="${tag#v}"
# pacman sorts 1.2.3pre1 below 1.2.3 and forbids a hyphen in pkgver.
pkgver="${version/-/pre}"
repo=https://github.com/Blockworked/Blockwork
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

sum() { # url -> sha256
    curl -fsSL --retry 3 "$1" -o "$tmp/f"
    sha256sum "$tmp/f" | cut -d' ' -f1
}

curl -fsSL --retry 3 "$repo/archive/refs/tags/$tag.tar.gz" -o "$tmp/source.tar.gz"
source_sha=$(sha256sum "$tmp/source.tar.gz" | cut -d' ' -f1)
tar -xOf "$tmp/source.tar.gz" "Blockwork-$version/blockwork-qt/native/CMakeLists.txt" > "$tmp/CMakeLists.txt"
qwk_url=$(grep -oE 'https://codeload.github.com/stdware/qwindowkit/[^ ]+' "$tmp/CMakeLists.txt")
qwk_commit=${qwk_url##*/}
qwk_sha=$(grep -oE 'SHA256=[0-9a-f]+' "$tmp/CMakeLists.txt" | cut -d= -f2)

mkdir -p "$out/blockwork"
sed -e "s/@VERSION@/$pkgver/g" -e "s/@TAG@/$tag/g" \
    -e "s/@SOURCE_VERSION@/$version/" -e "s/@SHA256@/$source_sha/" \
    -e "s/@QWK_COMMIT@/$qwk_commit/" -e "s/@QWK_SHA256@/$qwk_sha/" \
    packaging/aur/blockwork/PKGBUILD.in >"$out/blockwork/PKGBUILD"
if [[ ${3:-} == --source-only ]]; then
    cp "$tmp/source.tar.gz" "$out/blockwork/blockwork-$pkgver.tar.gz"
    exit
fi

mkdir -p "$out/blockwork-bin" "$out/blockwork-git"
x86_sha=$(sum "$repo/releases/download/$tag/blockwork-arch-x86_64.pkg.tar.zst")
arm_sha=$(sum "$repo/releases/download/$tag/blockwork-arch-aarch64.pkg.tar.zst")
sed -e "s/@VERSION@/$pkgver/g" -e "s/@TAG@/$tag/g" \
    -e "s/@X86_64_SHA256@/$x86_sha/" -e "s/@AARCH64_SHA256@/$arm_sha/" \
    packaging/aur/blockwork-bin/PKGBUILD.in >"$out/blockwork-bin/PKGBUILD"

cp packaging/aur/blockwork-git/PKGBUILD "$out/blockwork-git/PKGBUILD"
if grep -l '@[A-Z0-9_]*@' "$out"/*/PKGBUILD; then
    echo "unfilled placeholders above" >&2
    exit 1
fi
echo "$out"
