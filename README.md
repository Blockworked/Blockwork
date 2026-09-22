# Blockwork

A native Qt 6/QML app, backed by Rust through CXX-Qt, for visually creating
and running macros on Windows, Linux, and macOS.

## Building

### Build
```bash
git clone https://github.com/Blockworked/blockstitch.git blockstitch
git clone https://github.com/Blockworked/blockwork.git Blockwork && cd Blockwork
just
```

Keep the two repositories as siblings: Blockwork consumes Blockstitch's native
QML module directly while Blockstitch keeps its browser frontend intact. The
build uses an installed Qt 6 when one is available; otherwise CXX-Qt's Qt
Minimal integration downloads the required Qt SDK. Release packaging uses
`windeployqt`/`macdeployqt` to include the dynamic Qt runtime. The old
Tauri/Vue sources remain in `src-tauri/` and `ui/` only as a migration
reference; they are no longer workspace members or release inputs.

### Installation

AUR: `blockwork`

```bash
just install
```

### Windows installer

Release builds of the Windows installer are published automatically on each
GitHub release. To build one locally:

1. Install [Inno Setup 6](https://jrsoftware.org/isdl.php) (one-time).
2. Run:
   ```powershell
   pwsh -File scripts/build-installer.ps1
   ```
   This produces `dist\blockwork-windows-x86_64-setup.exe`.

## Linux

Recording input and playing back macros uses `/dev/uinput` and `/dev/input/event*`
directly, so your user needs to be in the `input` group:

```bash
sudo usermod -aG input $USER
```

Log out and back in (or reboot) for the new group membership to take effect.
