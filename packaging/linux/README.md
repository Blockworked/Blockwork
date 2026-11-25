# Linux packages

Every Linux package installs Blockwork the same way, through `install.sh`:

| Path | What |
| --- | --- |
| `/usr/lib/blockwork/blockwork`, `blockwork-daemon` | the UI and the daemon, side by side (the UI starts the daemon from beside itself) |
| `/usr/bin/blockwork` | a link to the UI |
| `/usr/share/applications/com.blockworked.Blockwork.desktop` | desktop entry, named after the app ID the metainfo uses |
| `/usr/share/metainfo/com.blockworked.Blockwork.metainfo.xml` | software center listing |
| `/usr/lib/udev/rules.d/60-blockwork-uinput.rules` | lets the user at the seat use `/dev/uinput` for playback |
| `/usr/lib/modules-load.d/blockwork-uinput.conf` | loads `uinput` at boot |

Recording and hotkeys read `/dev/input/event*`, which still needs the user in
the `input` group. The packages don't add anyone to it.

## Release automation

`release.yml` runs these after the GitHub release is published:

- **COPR** (`copr.yml`): `source-tarball.sh` makes `blockwork-<version>.tar.xz`
  with every crate vendored and the QWindowKit source, so the build needs no
  network. `packaging/rpm/build-srpm.sh` turns it and `blockwork.spec.in` into a
  source RPM, and `copr-cli` submits it. Needs the `COPR_CONFIG` secret (the
  contents of `~/.config/copr` from https://copr.fedorainfracloud.org/api/) and
  the `COPR_PROJECT` variable (`owner/project`, `ethanstokes/Blockwork` when unset). The chroots built are the ones
  enabled on the COPR project; Qt 6.10 or newer is needed, so Fedora 43 and up.
- **AUR** (`aur.yml`): `release.yml` builds the `blockwork` source recipe on
  Arch Linux x86_64 and Arch Linux ARM aarch64 and attaches the native packages
  to the GitHub release. `blockwork-bin` uses those packages and depends on
  system Qt. `packaging/aur/render.sh` sets versions and checksums for
  `blockwork`, `blockwork-bin` and `blockwork-git`. The AUR workflow checks
  `blockwork-bin`, writes the `.SRCINFO` files and pushes the packages that changed.
  Needs the `AUR_SSH_PRIVATE_KEY` secret: a key added to the AUR account that
  maintains the packages. Commits are made as `AUR_GIT_NAME`/`AUR_GIT_EMAIL`
  (variables), Ethan Stokes by default.

- **Debian and Ubuntu** (`release.yml` itself, attached to the GitHub release):
  `packaging/deb/build-deb.sh native` packages a build linked against the
  distro's own Qt, made in `ubuntu:26.04` and `ubuntu:26.10` containers, and
  depends on that release's Qt and QML packages. `build-deb.sh bundled` packages
  the AppImage's AppDir, Qt included, in `/opt/blockwork` for releases whose Qt is
  older than 6.10 (Debian 13, Ubuntu 24.04). The AppImage job runs on Ubuntu
  24.04 for this, since the bundle needs at least the glibc it was built with.
  There's no apt repository yet.

- **Flatpak repo** (`flatpak-repo.yml`): `packaging/flatpak/update-repo.sh`
  adds the release's Flatpak bundles to a GPG-signed OSTree repo, keeping the
  last three commits per arch with static deltas between them, and writes
  `blockwork.flatpakrepo` and `blockwork.flatpakref`. The repo lives in a
  Cloudflare R2 bucket (`FLATPAK_R2_BUCKET`, default `blockwork-flatpak`) served
  at `https://flatpak.blockworked.com` (`FLATPAK_REPO_URL`); each run downloads
  it, adds the bundles and uploads objects before the summary. Needs the
  `FLATPAK_GPG_PRIVATE_KEY`, `R2_ACCOUNT_ID`, `R2_ACCESS_KEY_ID` and
  `R2_SECRET_ACCESS_KEY` secrets. With `packaging/flatpak/blockwork.gpg` (the
  public key) in the tree, the release bundles also name the repo, so installing
  one adds it as a remote. Users install with
  `flatpak install https://flatpak.blockworked.com/blockwork.flatpakref`.

COPR, AUR and the Flatpak repo can also be run by hand for an existing tag; the AUR one only
pushes when asked.

## Nix

`flake.nix` at the repo root builds `packaging/nix/package.nix`:

```bash
nix run github:Blockworked/Blockwork
```

On NixOS, import `nixosModules.default` and set `programs.blockwork.enable = true;`
to install it with the udev rule and `uinput` loaded.

## Building locally

```bash
packaging/linux/source-tarball.sh dist              # dist/blockwork-<version>.tar.xz
packaging/rpm/build-srpm.sh dist/blockwork-*.tar.xz dist   # on Fedora
rpmbuild --rebuild dist/blockwork-*.src.rpm

# on Ubuntu 26.04 or newer, after cargo build --release (with QMAKE=qmake6
# and RUSTFLAGS=-Clink-arg=-fuse-ld=lld)
packaging/deb/build-deb.sh native dist
```
