# Changelog

All notable changes to Blockwork are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Tag releases (`0.x.y`, see `.github/workflows/release.yml`) publish the matching
section below as the GitHub release notes, followed by the downloads table.

## [Unreleased]

## [0.6.0] - 2026-10-21

### Added
- File blocks:
  - Write or append to a file, read a file, check a file exists, list a folder.
  - "When file changes" trigger.
- Web blocks:
  - Send GET/POST/PUT/PATCH/DELETE requests into a variable, or fetch a URL.
  - Read values out of JSON.
  - Webhook trigger: an opt-in local server (Settings > Integrations) that starts macros on a web request with a token, passing the request body.
- Shell reporters: a command's output and whether it succeeded.
- Notify block for desktop notifications.
- Dialog blocks: ask for text, ask yes or no, choose from a list, show a message.
  - They work with the editor closed.
  - Cancelling stops the run unless a try block catches it.
- Windows:
  - Active window title/app reporters and a "when window is focused/unfocused" trigger (Windows, macOS, X11, Hyprland, Sway, KDE with kdotool).
  - Focus, minimize, maximize, restore, close, move or resize a window, matched by title or app (picked from a dropdown), plus a "window exists?" reporter.
- Window detection blocks:
  - Detect objects in a window with a local detector service, and wait for a detection.
  - Aim one step toward a point, or at a tracked detection.
  - "Window x/y/width/height/center" and "detection of" reporters.
  - Detectors must run on this computer.
- Screen blocks, in a new Screen palette category:
  - Color at a point, average color of a region, colors match.
  - Text on screen (OCR): Windows/macOS built-ins, or Tesseract on Linux.
  - Mouse position, screen size, monitor count, monitor position/size.
- Image blocks:
  - Find an image on screen, click on it, and wait for it to appear or disappear, with a tolerance.
  - Capture the image from the screen or load a PNG.
- Screen picker in the editor toolbar:
  - A see-through overlay on every monitor: click a point or drag a region to get its coordinates and color, and add matching blocks.
  - On Wayland, captures go through the desktop portal (KDE and GNOME may ask for permission once) or `grim` on wlroots desktops.
- "Wait until" block that pauses until a condition is true, with a time limit.
- Error handling:
  - Try/on error, throw error, error message, stop macro.
  - Retry block: runs its body again after a failure, up to N attempts.
  - Per-macro time limit and loop limit.
- Emergency stop hotkey (Ctrl+Alt+Shift+Esc) that stops every run and pauses triggers until resumed.
- Run feedback:
  - The block each running strand is on is outlined live on the canvas.
  - Blocks that failed in the last run get a red marker; click a logged error to jump to its block.
  - Activity strip under the canvas shows the latest run event; click it to open the run log.
  - Tray menu "Recent activity" lists the latest runs, triggers and errors.
- Debugger:
  - Right-click a block to toggle a breakpoint.
  - Pause, resume, step one block at a time (F8/F10) or stop from the bar above the canvas, which shows the paused strand's variables and lists.
- Macros calling macros:
  - Run macro (and continue, or wait for it to finish), stop macro by name, "macro is running?" reporter.
  - Signals: "send signal (and wait)" starts this macro's "when I receive" strands, sharing the run's variables.
  - "With lock" block: only one strand, across every macro, runs inside a lock of the same name at a time.
- Global variables:
  - Set/change global blocks and a "global" reporter, shared by every macro and kept across restarts.
  - Listed in Settings, where they can be deleted.
- Secrets:
  - Named passwords and API keys in Settings > Integrations, kept in the system keyring (Secret Service, Credential Manager, Keychain) and read with the "secret" block.
  - Values are masked in the run log, previews and saved or exported macro files.
  - The import review warns about macros that read them.
- Text and data blocks:
  - Contains / starts with / ends with / matches regex.
  - Replace, regex replace and find; split into a list or take one part; letters from/to; trim.
  - Number formatting, and timestamps with date formatting, parsing and parts.
- Text expansion:
  - A "when I type" trigger that fires on an abbreviation like `;addr` typed anywhere, optionally erasing it.
  - A paste block that pastes through the clipboard and puts the old clipboard back.
  - The Text snippet template uses them.
- More triggers and reporters:
  - App starts or exits, after N minutes idle, screen locks or unlocks.
  - Network connects or disconnects, USB device plugged in or unplugged.
  - Mouse button pressed or released (optionally with modifier keys).
  - Reporters: "app is running?", "seconds idle", "screen is locked?", "online?", "USB device plugged in?".
- Schedules:
  - "On schedule" trigger with cron expressions (`*/15 9-17 * * mon-fri`, `@hourly`); a broken expression is reported in the run log.
  - "Once at" trigger for a single date and time.
  - Per-macro "catch up on missed schedules" setting that runs schedules missed while Blockwork was closed or the computer slept.
- Per-macro conditions in Macro settings: triggers and hotkeys only start the macro while a given window is focused, on AC or battery power, between two times, or on chosen weekdays.
- Macro library:
  - The macro picker is now a searchable list with folders.
  - Open, duplicate, move to a folder, export or delete a macro from it.
  - Import and export use the system file dialog.
- Importing and sharing macros:
  - The import review shows what a macro can do (commands, web, files, input, clipboard, windows, triggers) before it's added.
  - Bundles: export a library folder or a macro (with the macros it runs) as a `.blockwork` file and import it with the same review.
  - Permissions: imported macros ask before running commands, going online, writing files, reading secrets or reading the screen, with "allow this time" and "always allow".
  - Choose what to allow in the import review or in Macro settings > Permissions; grants are kept outside the macro file.
- Block library:
  - My Blocks > Browse lists shared block packs from library indexes set in Settings > Editor; the built-in one is https://community.blockworked.com.
  - Packs are checked against the SHA-256 their index pins, reviewed before they're added, and replace their older versions in place.
  - Packs published by the Blockworked team show a blue verified badge next to their publisher in Browse library. Only the built-in library can mark packs this way.
  - Right-click a custom block to share it, and the blocks it uses, as a `.blockpack` file.
  - After saving a pack, "Submit to community library..." opens the library site, where you sign in with GitHub or Google and upload it for review.
- Translatable UI:
  - Every editor and dialog string goes through Qt's translation system, with a language setting in Settings > General.
  - Ships with English and pirate-speak English; every built-in language shows up in the setting by itself.
  - Translation happens on Weblate.
- Command line: `blockwork list`, `blockwork run <name> [--wait]` and `blockwork stop <name>` drive macros through the daemon without a window.
- Starter templates (auto clicker, text snippet, break reminder, website watcher) in the welcome screen and the macro menu's "New from template".
- UI animations: menus and drop-downs fade and grow open, dialogs ease in over a fading backdrop, and buttons, switches, check boxes and list rows ease between hover and pressed states. Panels like the sidebar and run log slide open and shut, and pages and Settings tabs fade in.
- Smooth scrolling: a mouse wheel notch glides to its spot instead of jumping, in the editor canvas, the block palette, Settings and the other scrolling lists. Jumping to a block from the run log glides there too.
- Settings > General > Animation speed, a slider from Instant to Slow. Instant turns every animation off.

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
- On Linux, evdev waits for held keys and mouse buttons to be released before
  grabbing an input device, preventing stuck input when Blockwork starts.
- A mouse wheel no longer scrolls far too fast; each notch now moves the normal system step. Touchpad scrolling is unchanged.
- Ctrl+scroll zooms the canvas around the mouse pointer instead of pushing the view right and down, and mouse wheel zoom now eases in (follows the animation speed setting).

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

[Unreleased]: https://github.com/Blockworked/Blockwork/compare/0.6.0...HEAD
[0.6.0]: https://github.com/Blockworked/Blockwork/compare/0.5.4...0.6.0
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
