# Blockwork

A native Qt 6/QML app, backed by Rust through CXX-Qt, for visually creating
and running macros on Windows, Linux, and macOS.

## Building

### Build
```bash
git clone https://github.com/Blockworked/blockwork.git Blockwork && cd Blockwork
just
```

Cargo fetches the Lucide icons (`lucide-qt`) from its tagged release. The Blockstitch core and native QML frontend
are fetched from the same pinned Git revision, so their APIs stay in sync.
The build uses an installed Qt 6 when one is available; otherwise CXX-Qt's Qt
Minimal integration downloads the required Qt SDK. Release packaging uses
`windeployqt`/`macdeployqt` to include the dynamic Qt runtime.

### Installation

AUR: `blockwork`

```bash
just install
```

This builds before installing. On Windows it builds, signs, and installs a local
MSIX package (certificate installation requires elevation); on macOS it installs
the app bundle into `/Applications`.

To build and launch from the checkout, use `just run`. On Windows this stages the
editor, daemon, and Qt runtime in `dist/windows-local` before launching.

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
