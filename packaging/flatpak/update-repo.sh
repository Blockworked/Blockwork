#!/usr/bin/env bash
# Adds Flatpak bundles to a signed OSTree repo for `flatpak update`.
#   update-repo.sh SITE_DIR GPG_KEY_ID BUNDLE...
# SITE_DIR holds repo/ (made when missing) and gets blockwork.flatpakrepo and
# blockwork.flatpakref beside it. The key must be in gpg's keyring
# ($GNUPGHOME). REPO_URL is where SITE_DIR is served
# (default https://flatpak.blockworked.com).
set -euo pipefail
site="${1:?SITE_DIR}"
key="${2:?GPG_KEY_ID}"
shift 2
[ $# -gt 0 ] || { echo "no bundles given" >&2; exit 1; }
url="${REPO_URL:-https://flatpak.blockworked.com}"
repo="$site/repo"
gpg_args=(--gpg-sign="$key")
[ -n "${GNUPGHOME:-}" ] && gpg_args+=(--gpg-homedir="$GNUPGHOME")

mkdir -p "$site"
[ -f "$repo/config" ] || ostree init --mode=archive-z2 --repo="$repo"
# Object storage keeps no empty folders, so a downloaded repo lacks these.
mkdir -p "$repo"/refs/{heads,remotes,mirrors} "$repo"/{objects,tmp/cache,extensions,state}

for bundle in "$@"; do
    flatpak build-import-bundle "${gpg_args[@]}" "$repo" "$bundle"
done

# Keep the last few commits per arch, with deltas between them, so an update
# downloads only what changed.
flatpak build-update-repo "${gpg_args[@]}" --title=Blockwork --default-branch=stable \
    --generate-static-deltas --prune --prune-depth=3 "$repo"

pubkey=$(gpg --export "$key" | base64 -w0)
cat >"$site/blockwork.flatpakrepo" <<REPO
[Flatpak Repo]
Title=Blockwork
Url=$url/repo/
Homepage=https://blockworked.com
Comment=Blockwork, a block editor for keyboard and mouse macros
Icon=$url/blockwork.png
GPGKey=$pubkey
REPO
cat >"$site/blockwork.flatpakref" <<REF
[Flatpak Ref]
Title=Blockwork
Name=com.blockworked.Blockwork
Branch=stable
Url=$url/repo/
SuggestRemoteName=blockwork
Homepage=https://blockworked.com
Icon=$url/blockwork.png
RuntimeRepo=https://dl.flathub.org/repo/flathub.flatpakrepo
IsRuntime=false
GPGKey=$pubkey
REF
install -m644 "$(dirname "$0")/../../res/icons/blockwork.png" "$site/blockwork.png"
