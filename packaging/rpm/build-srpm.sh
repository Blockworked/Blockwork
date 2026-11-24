#!/usr/bin/env bash
# Renders the spec and builds a source RPM around a tarball from
# packaging/linux/source-tarball.sh. Run on Fedora (rpmbuild, rpm macros).
# usage: packaging/rpm/build-srpm.sh TARBALL [OUT_DIR=dist]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tarball="$(realpath "${1:?TARBALL}")"
out="$(mkdir -p "${2:-dist}" && cd "${2:-dist}" && pwd)"
version=$(basename "$tarball" .tar.xz)
version=${version#blockwork-}
top=$(mktemp -d)
trap 'rm -rf "$top"' EXIT
mkdir -p "$top/SOURCES" "$top/SPECS"
cp "$tarball" "$top/SOURCES/"
sed -e "s/@VERSION@/$version/g" -e "s/@DATE@/$(LC_ALL=C date -u '+%a %b %d %Y')/" \
    "$here/blockwork.spec.in" >"$top/SPECS/blockwork.spec"
rpmbuild -bs --define "_topdir $top" "$top/SPECS/blockwork.spec" >&2
cp "$top"/SRPMS/*.src.rpm "$out/"
ls "$out"/blockwork-"$version"-*.src.rpm
