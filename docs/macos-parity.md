# macOS

How the macOS build behaves next to Windows and Linux, how to build it, and
how to run the native checks.

## Permissions

- **Accessibility** (Privacy & Security > Accessibility): pressing keys,
  moving the mouse, hotkeys, recording and window blocks. Without it, input
  blocks report the missing permission instead of claiming success.
- **Screen Recording** (Privacy & Security > Screen & System Audio Recording):
  colors, image search, text on screen and detection. Without it, capture
  blocks report the missing permission; macOS would otherwise return images
  with other apps left out.

An ad-hoc signed rebuild can keep a stale entry that still denies access.
Remove the app from both lists, add the exact bundle again and restart it.
System Settings' "Quit & Reopen" may start a different installed copy, so
launch the intended bundle by its path.

## Differences from Windows and Linux

- F21-F24 and Select have no native key code, so pressing them is an error on
  the block. Remap them when moving a macro to a Mac.
- PrintScreen, ScrollLock and Pause are pressed and matched as F13, F14 and
  F15, as macOS translates them from a PC keyboard. F14 and F15 are brightness
  shortcuts by default.
- Close app asks the chosen app bundle to quit, so an app with unsaved work
  may stay open behind a save prompt. Windows force-closes and Linux sends
  SIGTERM.
- Maximize fills the usable area of the window's screen; it doesn't enter
  native fullscreen. Restore undoes either.
- Text on screen uses Vision. After a stall it falls back to Vision's fast
  recognizer for ten minutes, then tries the accurate one again.
- Detection models run on the GPU through Metal when available.
- macOS 13 is the minimum, because of Qt 6.10.

## Build and verify the app

Use a full Qt 6.10 SDK including Widgets:

```sh
QMAKE=/path/to/Qt/6.10.3/macos/bin/qmake scripts/build-macos-bundle.sh
python3 scripts/macos/verify-bundle.py dist/Blockwork.app
```

The validator checks nested signatures, architectures, escaping symlinks,
host-specific search paths and every non-system Mach-O dependency. A
successful macdeployqt exit alone is not enough.

## Run the native checks

```sh
BLOCKWORK_TEST_PYTHON=/path/to/python-with-numpy-pillow-scipy scripts/macos/verify.sh --offline --target-dir /path/to/test-cache
```

This builds a disposable AppKit fixture window (`scripts/macos/test-window.swift`)
with a known red marker, label and text field, starts a local detector, runs
the ignored native tests serially and cleans up afterwards. The test
executable needs its own Accessibility and Screen Recording grants. It covers
the core system features, not the updater or ONNX models.

Other opt-in checks:

```sh
# Upgrade a real release archive to a rebuilt app, inside a temporary folder.
BLOCKWORK_TEST_RELEASE_ZIP=/absolute/path/blockwork-macos-arm64.app.zip \
BLOCKWORK_TEST_UPDATE_APP=/absolute/path/Blockwork.app \
cargo test -p blockwork-core --no-default-features --features updater \
  macos_released_bundle_upgrades_to_rebuilt_app -- --ignored --nocapture

# Compare Metal and CPU outputs for a model.
BLOCKWORK_TEST_MODEL=/path/to/model.onnx \
cargo test -p blockwork-core metal_matches_cpu_on_real_model_outputs -- --ignored --nocapture
```

The `blockwork-core/examples/macos-*-probe.rs` examples exercise app launch
and close, recording, and the network updater against disposable copies.

## CI

`checks.yml` runs core and FFI tests plus the bundle validator's own tests on
Apple Silicon and Intel runners. The artifact and release workflows build,
validate and start both Mac bundles and run the application QML tests on
them. Dispatching `checks.yml` with `qml_probe=true` runs only the Mac QML
tests, with `gc_limit` choosing Qt's GC time limit (`0` is what the app uses
on Qt 6.10, `1` the incremental collector that crashed there).

## Not covered yet

- Physical monitor hot-plug.
- Updating a live installed copy by clicking its update button.
- Permission-dependent input and capture on the Intel CI runner.
