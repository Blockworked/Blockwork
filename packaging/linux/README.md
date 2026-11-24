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
  the `COPR_PROJECT` variable (`owner/project`). The chroots built are the ones
  enabled on the COPR project; Qt 6.10 or newer is needed, so Fedora 43 and up.
- **AUR** (`aur.yml`): `packaging/aur/render.sh` fills in versions and checksums
  for `blockwork` (from source), `blockwork-bin` (the release AppImages,
  extracted to `/opt/blockwork`) and `blockwork-git`, builds `blockwork-bin` once
  as a check, writes the `.SRCINFO` files and pushes the packages that changed.
  Needs the `AUR_SSH_PRIVATE_KEY` secret: a key added to the AUR account that
  maintains the packages. Commits are made as `AUR_GIT_NAME`/`AUR_GIT_EMAIL`
  (variables), Ethan Stokes by default.

Both workflows can also be run by hand for an existing tag; the AUR one only
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
```
