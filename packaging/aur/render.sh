#!/usr/bin/env bash
# Fills in the AUR PKGBUILDs for a published release tag: versions and the
# checksums of what each one downloads. Writes OUT_DIR/<package>/PKGBUILD.
# usage: packaging/aur/render.sh TAG [OUT_DIR=dist/aur]
set -euo pipefail
cd "$(dirname "$0")/../.."
tag="${1:?TAG}"
out="$(mkdir -p "${2:-dist/aur}" && cd "${2:-dist/aur}" && pwd)"
version="${tag#v}"
repo=https://github.com/Blockworked/Blockwork
raw=https://raw.githubusercontent.com/Blockworked/Blockwork/$tag
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

sum() { # url -> sha256
    curl -fsSL --retry 3 "$1" -o "$tmp/f"
    sha256sum "$tmp/f" | cut -d' ' -f1
}

qwk_url=$(grep -oE 'https://codeload.github.com/stdware/qwindowkit/[^ ]+' blockwork-qt/native/CMakeLists.txt)
qwk_commit=${qwk_url##*/}
qwk_sha=$(grep -oE 'SHA256=[0-9a-f]+' blockwork-qt/native/CMakeLists.txt | cut -d= -f2)

mkdir -p "$out/blockwork" "$out/blockwork-bin" "$out/blockwork-git"
sed -e "s/@VERSION@/$version/g" \
    -e "s/@SHA256@/$(sum "$repo/archive/refs/tags/$tag.tar.gz")/" \
    -e "s/@QWK_COMMIT@/$qwk_commit/" -e "s/@QWK_SHA256@/$qwk_sha/" \
    packaging/aur/blockwork/PKGBUILD.in >"$out/blockwork/PKGBUILD"

sed -e "s/@VERSION@/$version/g" \
    -e "s/@LICENSE_SHA256@/$(sum "$raw/LICENSE")/" \
    -e "s/@RULES_SHA256@/$(sum "$raw/packaging/linux/60-blockwork-uinput.rules")/" \
    -e "s/@METAINFO_SHA256@/$(sum "$raw/res/com.blockworked.Blockwork.metainfo.xml")/" \
    -e "s/@X86_64_SHA256@/$(sum "$repo/releases/download/$tag/blockwork-linux-x86_64.AppImage")/" \
    -e "s/@AARCH64_SHA256@/$(sum "$repo/releases/download/$tag/blockwork-linux-arm64.AppImage")/" \
    packaging/aur/blockwork-bin/PKGBUILD.in >"$out/blockwork-bin/PKGBUILD"

cp packaging/aur/blockwork-git/PKGBUILD "$out/blockwork-git/PKGBUILD"
if grep -l '@[A-Z0-9_]*@' "$out"/*/PKGBUILD; then
    echo "unfilled placeholders above" >&2
    exit 1
fi
echo "$out"
