# Changelog

All notable changes to Blockwork are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Tag releases (`0.x.y`, see `.github/workflows/release.yml`) publish the matching
section below as the GitHub release notes, followed by the downloads table.

## [Unreleased]

### Added
- Flatpak packaging for Flathub.
- Open App and Close App work in the Flatpak.

### Changed
- Split into a background daemon and the editor window; closing the window
  frees its memory.

### Fixed
- Freeze when closing to tray on Wayland.
- Missing maximize button on KDE.
- Crash when closing the window on Wayland.
- White flash when opening and closing the window.

## [0.5.3] - 2026-09-14

### Changed
- Project transferred to the Blockworked GitHub organization.
- App ID changed to com.blockworked.Blockwork

## [0.5.2] - 2026-09-14

### Added
- macOS self-updater: downloads the latest release's `.app.zip`, swaps the new
  `Blockwork.app` over the running bundle via the system `ditto`, and re-opens it.

## [0.5.1] - 2026-09-13

### Added
- Custom block coloring.

### Changed
- Custom block rework and QoL fixes.
- Show the correct operator type in the sidebar.
- Bump Tauri and blockstitch, adapt to blockstitch's new `Call` shape.
- Dependency updates and regenerated Flatpak sources.

## [0.5.0] - 2026-09-04

### Added
- Renamed to Blockwork with a new logo and icons.
- Migrated the block editor to blockstitch (style, pan/zoom, drag handling).
- Loops, booleans, and If / If-Else blocks.
- Reworked comments and Block Details pane.
- Time/date blocks, app open/quit blocks, battery event and operator.
- System tray with quit-from-UI support.
- Mouse movement recording.
- Wayland support for Linux packaging.

### Fixed
- AppImage packaging fix.
- Flatpak Wayland fixes and source regeneration.
- Warning when OpenRazer is present.
- `If` / `IfElse` sidebar entries, boolean layout, selection, outline, and block style fixes.

### Changed
- blockstitch dependency bumps.

## [0.4.0] - 2026-07-30

### Added
- Introduced the Scratch-like block editor experience, replacing the libcosmic list view.
- Variables with live value preview.
- Operators: text block, Join, Random, newline/tab constants, and more operators.
- Reworked sidebar and custom blocks.
- Windows arm64 build and Windows installer artifacts in CI.

### Changed
- Removed the libcosmic list-view UI; desktop app is Tauri + CEF.
- General visual rework and codebase refactor.

### Fixed
- Hotkeys breaking after Alt+Tab.
- Flatpak, Windows build, and macOS bundle fixes.

## [0.3.1] - 2026-07-07

### Added
- New config directory layout; first AppImage and Flatpak packaging attempts.

### Fixed
- Flatpak build fixes.

## [0.3.0] - 2026-06-20

### Added
- Windows installer (Inno Setup) and self-updater.
- Global hotkeys on Windows and Linux; record-hotkey and record-input support.
- Local TCP control server with server settings page.
- Floating-point `Wait`, randomized `Wait` variance, and wait-at-start-of-recording.
- Force-quit a running macro.
- Native input backends: evdev on Linux, Windows API on Windows.

### Changed
- More precise playback timing (`spin_sleep`).
- Windows input emulation via enigo; libcosmic updates.
- Releases trigger on any pushed tag.

### Fixed
- Server stop, icon handling, and Windows packaging fixes.
- Key handling: ignore the app's own keypresses, support invalid text and modifiers.
- evdev touchpad fix.

## [0.2.1] - 2026-06-05

### Added
- "Run script" repurposed into run-any-command.
- Warning when the user is not in the `input` group (Linux).

### Changed
- libcosmic bump.

## [0.2.0] - 2026-05-20

### Added
- Record Input, custom hotkey settings menu.
- Undo/redo and comments.
- Autoscroll; relative mouse moves.

### Fixed
- Performance fix for large macros.

## [0.1.0] - 2026-05-15

Initial release.

### Added
- libcosmic list-view macro editor: instruction sequences, per-macro files, full instruction editing GUI.
- Shell command support, `Wait` blocks, key scanning with modifier support.
- Global shortcuts (navigation, loop mode) on Linux; global keybinds on Windows.
- App icons, updater and confirm dialogs, Wayland support.

[Unreleased]: https://github.com/Blockworked/Blockwork/compare/0.5.3...HEAD
[0.5.3]: https://github.com/Blockworked/Blockwork/compare/0.5.2...0.5.3
[0.5.2]: https://github.com/Blockworked/Blockwork/compare/0.5.1...0.5.2
[0.5.1]: https://github.com/Blockworked/Blockwork/compare/0.5.0...0.5.1
[0.5.0]: https://github.com/Blockworked/Blockwork/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/Blockworked/Blockwork/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/Blockworked/Blockwork/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/Blockworked/Blockwork/compare/0.2.1...0.3.0
[0.2.1]: https://github.com/Blockworked/Blockwork/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/Blockworked/Blockwork/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/Blockworked/Blockwork/releases/tag/0.1.0
