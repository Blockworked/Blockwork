# Blockwork

A Tauri app to visually create and run macros on Windows, Linux, and macOS.

## Building

### Build
```bash
git clone https://github.com/Blockworked/blockwork.git blockwork && cd blockwork
just
```

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