# Changelog

All notable changes to Blockwork are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Tag releases (`0.x.y`, see `.github/workflows/release.yml`) publish the matching
section below as the GitHub release notes, followed by the downloads table.

## [Unreleased]

### Added
- File blocks: write or append to a file, read a file, check a file exists,
  list a folder, and a "when file changes" trigger.
- Web blocks: send GET/POST/PUT/PATCH/DELETE requests into a variable, fetch a
  URL, and read values out of JSON.
- Shell reporters: a command's output and whether it succeeded.
- Notify block for desktop notifications.
- Active window title/app reporters and a "when window is focused/unfocused"
  trigger (Windows, macOS, X11, Hyprland, Sway, KDE with kdotool).
- Activity strip under the canvas showing the macro's latest run event; click it to open the run log.
- Blocks that failed in the last run get a red marker; click a logged error to jump to its block.
- Tray menu "Recent activity" lists the latest runs, triggers and errors.
- Error handling: try/on error, throw error, error message, and stop macro.
- Emergency stop hotkey (Ctrl+Alt+Shift+Esc) that stops every run and pauses
  triggers until resumed, plus a per-macro time limit.
- The block each running strand is on is outlined live on the canvas.
- Retry block: runs its body again after a failure, up to N attempts.
- Per-macro loop limit that stops runaway loops.
- Window blocks: focus, minimize, maximize, restore, close, move or resize a
  window, matched by its title or its app (picked from a dropdown), plus a
  "window exists?" reporter.
- Run macro (and continue, or wait for it to finish), stop macro by name, and
  a "macro is running?" reporter.
- Global variables: set/change global blocks and a "global" reporter, shared
  by every macro and kept across restarts. Listed in Settings, where they can
  be deleted.
- Command line: `blockwork list`, `blockwork run <name> [--wait]` and
  `blockwork stop <name>` drive macros through the daemon without a window.
- Starter templates (auto clicker, text snippet, break reminder, website
  watcher) in the welcome screen and the macro menu's "New from template".
- Screen blocks, in a new Screen palette category: color at a point, average
  color of a region, colors match, text on screen (OCR), mouse position,
  screen size, monitor count and monitor position/size.
- Image blocks: find an image on screen, click on it, and wait for it to
  appear or disappear, with a tolerance. Capture the image straight from the
  screen or load a PNG.
- "Wait until" block that pauses until a condition is true, with a time limit.
- Screen picker in the editor toolbar: a see-through overlay on every
  monitor where you click a point or drag a region on the live screen to get
  its coordinates and color, and add matching blocks.
  On Wayland, captures go through the desktop portal (KDE and GNOME may ask
  for permission once) or `grim` on wlroots desktops. Text recognition uses
  Windows/macOS built-ins, or Tesseract on Linux.
- Importing a macro shows what it can do (commands, web, files, input,
  clipboard, windows, triggers) before it's added.
- Text expansion: a "when I type" trigger that fires on an abbreviation like
  `;addr` typed anywhere, optionally erasing it, and a paste block that
  pastes through the clipboard and puts the old clipboard back. The Text
  snippet template uses them.
- Triggers for when an app starts or exits, after N minutes idle, and when
  the screen locks or unlocks, plus "app is running?", "seconds idle" and
  "screen is locked?" reporters.
- "On schedule" trigger with cron expressions (`*/15 9-17 * * mon-fri`,
  `@hourly`); a broken expression is reported in the run log.
- Per-macro conditions in Macro settings: triggers and hotkeys only start the
  macro while a given window is focused, on AC or battery power, between two
  times, or on chosen weekdays.
- "With lock" block: only one strand, across every macro, runs inside a lock
  of the same name at a time.
- Debugger: right-click a block to toggle a breakpoint, and pause, resume,
  step one block at a time (F8/F10) or stop a running macro from the bar
  above the canvas, which shows the paused strand's variables and lists.
- Secrets: named passwords and API keys in Settings > Integrations, kept in
  the system keyring (Secret Service, Credential Manager, Keychain) and read
  with the "secret" block. Their values are masked in the run log, previews
  and saved or exported macro files, and the import review warns about
  macros that read them.
- Text and data blocks: contains / starts with / ends with / matches regex,
  replace, regex replace and find, split into a list or take one part, letters
  from/to, trim, number formatting, and timestamps with date formatting,
  parsing and parts.
- Macro library: the macro picker is now a searchable list with folders, and
  each macro can be opened, duplicated, moved to a folder, exported or
  deleted from it. Import and export use the system file dialog.
- Triggers for when the network connects or disconnects, when a USB device is
  plugged in or unplugged, and when a mouse button is pressed or released
  (optionally with modifier keys), plus "online?" and "USB device plugged in?"
  reporters.
- Dialog blocks: ask for text, ask yes or no, choose from a list and show a
  message. They work with the editor closed; cancelling stops the run unless
  a try block catches it.
- "Once at" trigger for a single date and time, and a per-macro "catch up on
  missed schedules" setting that runs schedules missed while Blockwork was
  closed or the computer slept.
- Signals: "send signal (and wait)" starts this macro's "when I receive"
  strands, sharing the run's variables.
- Webhook trigger: an opt-in local server (Settings > Integrations) that
  starts macros on a web request with a token, passing the request body.
- Bundles: export a library folder or a macro (with the macros it runs) as a
  `.blockwork` file and import it with the same review.
- Permissions: imported macros ask before running commands, going online,
  writing files, reading secrets or reading the screen, with "allow this
  time" and "always allow". Choose what to allow in the import review or in
  Macro settings > Permissions; grants are kept outside the macro file.

### Changed
- Macro files now carry a format version and are upgraded step by step when
  loaded. Blocks a version can't run become notes on the canvas instead of
  making the whole macro fail to load, and an upgraded file's original is
  kept next to it as `.v0.bak`.
- Macros saved by a newer Blockwork are reported in the run log instead of
  being skipped silently.
- Blocks that fail at run time are reported in the run log instead of only
  the console.
- Click-to-preview won't run shell or web blocks.

### Fixed
- Importing a macro that runs commands or has custom settings did nothing in
  the Qt editor; it now opens the review.

## [0.5.4] - 2026-09-20

### Added
- Flatpak packaging for Flathub.
- Open App and Close App work in the Flatpak.

### Changed
- Split into a background daemon and the editor window; closing the window
  frees its memory.
- The linux backend now uses libei for absolute mouse positions.

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

[Unreleased]: https://github.com/Blockworked/Blockwork/compare/0.5.4...HEAD
[0.5.4]: https://github.com/Blockworked/Blockwork/compare/0.5.3...0.5.4
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
