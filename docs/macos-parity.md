# macOS parity verification

Work in progress. Passing tests establish the behavior below, not complete application parity.

Blockwork is the first gate. The later blockwork-gd gate requires fresh imports of `~/Downloads/ChesszDC-Bloodbath-Infinity-zBot.gdr` against the local level named approximately **Bloodbath Infinity 2**, accurate completion at normal speed, and reliable repeated completion at QOL Mod 10x. The game gate completed normal-speed fresh imports, 20/20 direct 10x attempts, 20/20 queued 10x attempts, and a final 3/3 universal-package 10x batch. Current game evidence and remaining checks are documented in the sibling `blockwork-gd/docs/macos-parity.md`.

## Implemented and exercised

| Area | Evidence | Coverage limits |
|---|---|---|
| Keyboard | Native Unicode/surrogate pairs, extended keys, side-specific modifier ordering, explicit Fn/keypad cleanup, event-synthesis permission preflight; packaged text delivered exactly `Mac parity: café 中 🦀` | More keyboard layouts and physical devices |
| Recording/hotkeys | Hardware timestamps, modifier suppression, playback filtering, disabled-tap recovery; packaged recording/replay preserved a/Shift+b/c, original waits, and Escape stop | Long-duration recording |
| Mouse | Extra buttons, absolute event coordinates, drag deltas, fractional two-axis scrolling; queued-target tracking prevents stale live coordinates after asynchronous posting | Additional mouse hardware |
| Screen | Native/nominal clipped capture, explicit sRGB/RGBA, permission preflight; packaged colors, image search/click, OCR and HTTP detection all succeeded | Multiple displays, Retina scaling configurations |
| Detection | Local HTTP detector positive fixture; real shipped ONNX model Metal/CPU comparison on three deterministic inputs, maximum absolute difference 0.000519 | Positive trained Minecraft image and broader models |
| Windows | CoreGraphics + bounded Accessibility operations; focus, consecutive geometry operations, minimize/restore, fullscreen, close; geometry tolerates redacted CG titles | More third-party application window types |
| Application lifecycle | Native NSWorkspace close by canonical bundle path; fixture close/open/close passed without AppleEvents; reopening the packaged app recreated the editor from its background daemon | More application types |
| Picker | System/Cryptex/Utilities/user roots; symlink-loop and helper filtering; accessible search/select; native macOS icons displayed | Broader application catalog |
| Dialogs | Packaged text/yes/choice/message IPC returned `text=café 中 🦀;yes=true;choice=Second`; keyboard choice navigation and accessible actions; parent watchdog closed an orphan prompt in 0.651 seconds | More assistive-technology clients |
| Persistence | Restoring selected macros now restores saved lists; regression plus fresh packaged restart; accessible slider changes persisted | Broader settings combinations |
| Battery/priority | Reject invalid capacity; native interactive QoS verified on a playback thread | Battery transitions and sustained load |
| FFI | Explicit standalone system feature and native linking | Game integration remains a separate gate |
| Packaging/updating | macOS 13 floor, nested signing and 97 Mach-O dependency checks; same-volume staging with ditto, symlink preservation, rollback, deferred relaunch waiting for old daemon | Real downloaded release installation; Intel runtime |

Media keys now use native system-defined events, with a raw-integer keyboard tap because the CoreGraphics Rust event enum omits type 14. Constructor tests pass; a real native event-tap test captured and suppressed all six dispatched volume events before they reached the system. Evidence: `/tmp/blockwork-macos-final-native-tests.log`.

Vision OCR uses a bounded subprocess with concurrent output draining and kill/reap on timeout. Blank/transparent images return immediately. An automatic worker gets five seconds; after a timeout, CPU mode is cached. After an accurate CPU timeout, a fresh process uses Vision’s fast recognizer on the CPU. Each fallback has a fifteen-second timeout, preserving a 35-second initial budget; the fallback mode is cached. Injected regressions cover the mode sequence, caching and bounded failure. Sampling a stalled worker showed Apple Neural Engine compilation even with the CPU device selected. The fast recognizer read the same saved desktop crops in 0.17–0.32 seconds. Its recognition accuracy can differ from the accurate model; it is used only after a timeout.

## Packaged evidence

The exact rebuilt `dist/Blockwork.app` has Accessibility and Screen Recording enabled. Stale entries for an ad-hoc rebuilt bundle can still deny access: removing and re-adding this exact bundle, then restarting, resolved both. Verify actual delivery and capture after rebuilding. System Settings' “Quit & Reopen” launched the older installed `/Applications/Blockwork.app` during testing; launch the intended bundle by exact path.

Positive packaged screen workflow: focus + four move/resize operations, wait/find/click image, score-sorted HTTP detection, wait detection, OCR and text reporting. Result: `pixel=#ff0000;average=#ff0000;image=520,1020;detections=1`, with OCR containing `BLOCKWORK MACOS 12345`. Evidence: `/tmp/blockwork-approved-screen-result.json`. A previous negative test without Screen Recording correctly returned the explicit permission error while window geometry continued to work: `/tmp/blockwork-packaged-missing-screen-permission-result.json`.

Saved gesture replay preserved click (260,1158), drag (234,1158) to (294,1158), delta (60,0), scroll -2 at (294,1158), and exact text `aBc`. Evidence: `/tmp/blockwork-macos-fixed-gesture-replay-result.json`. CLI completion means events were posted; poll fixture delivery before evaluating the result.

Additional evidence: `/tmp/blockwork-packaged-dialogs-result.json`, `/tmp/blockwork-macos-prompt-parent-exit-result.json`, `/tmp/blockwork-macos-reopen-result.json`, `/tmp/blockwork-macos-native-app-lifecycle.log`, `/tmp/blockwork-updater-bundle-keyboard-result.json`.

## Reproducibility and test checkpoints

All three Blockstitch dependencies are pinned to published Git commit `250bf3620dc276eca96bb849ef765459b7dfe3c8`. It contains semantic accessibility groups that prevent exponential AX traversal of large macros, named icon buttons, and accessible switches. There is no active local Cargo path override. The release build and workspace suite passed against this Git pin. Flatpak source metadata is regenerated from Cargo.lock, including the updated pin and Metal dependencies.

The full workspace checkpoint passed 340 tests (78 app, 242 core, five integration, ten protocol, five web), with 19 opt-in tests ignored (`/tmp/blockwork-macos-final-workspace-tests.log`). This checkpoint precedes the final fast-recognizer change, which is covered by the subsequent native suite. Application QML passed 45 tests, including actual prompt-window navigation and parent-death behavior; shared editor QML passed 30. The final native fixture suite passed 245 core tests with no ignored tests, plus five integration tests and FFI (`/tmp/blockwork-macos-fast-ocr-native-tests.log`). The earlier attempt failed two OCR tests; the fast-recognizer change repaired both, and the real desktop OCR diagnostic also returned text. Core/FFI Clippy passes with warnings (`/tmp/blockwork-macos-final-clippy.log`).

Intel core/FFI compile checks passed including the native application and media additions (`/tmp/blockwork-macos-final-intel-check.log`). Apple Silicon runtime is tested locally. Linux workspace/Clippy and Apple Silicon + Intel core/FFI CI checks passed at commit `e285eac69af1759811b39aeedd861653bd06a30b`: https://github.com/Blockworked/Blockwork/actions/runs/40147159871. These are core/FFI runtime checks, not Intel GUI testing. Rosetta is absent, so no local Intel runtime test has run.

## Run native verification

```sh
BLOCKWORK_TEST_PYTHON=/path/to/python-with-numpy-pillow-scipy scripts/macos/verify.sh --offline --target-dir /path/to/test-cache
```

The runner builds a disposable AppKit fixture with a known red marker, label and editable view; launches a local detector; runs serial native tests; and cleans up its processes. The development executable needs its own Accessibility and Screen Recording grants. This suite uses core/system features and does not establish updater or ONNX coverage.

## Build and verify the app

Use a full Qt 6.10 SDK including Widgets:

```sh
QMAKE=/path/to/Qt/6.10.3/macos/bin/qmake scripts/build-macos-bundle.sh
python3 scripts/macos/verify-bundle.py dist/Blockwork.app
```

The validator checks nested signatures, architectures, escaping symlinks, host-specific search paths and every non-system Mach-O dependency. A successful macdeployqt exit alone is insufficient.

## Remaining gates

The final release is built and the bundle validates all 97 Mach-O images. The existing grants survived this rebuild: packaged Unicode input and the complete screen/window/detection/OCR workflow passed. An unnecessary refresh prompted for authentication; it was cancelled without changing the grants. Evidence: `/tmp/blockwork-final-packaged-input-result.json` and `/tmp/blockwork-final-packaged-screen-result.json`. No authentication remains pending. The reviewed macOS platform branches and exercised packaged workflows have no remaining known functional blocker. Actual Intel runtime, multi-display hardware and a real installed-release update remain coverage limits; these are not claimed as executed. Proceed to the separately ordered fresh-import GD work; do not reuse the user's existing replay as the acceptance fixture.

The universal embedder build isolates Cargo objects by deployment target. Reusing the Qt app’s cache had linked macOS 13 objects into an 11.0-targeted Geode library; a separate cache prevents that mismatch. Translation catalogs were refreshed with the repository workflow, and all 31 catalogs (37,275 messages) pass structural checks.

## Final coverage audit

The final installed Apple Silicon bundle passed Unicode delivery, window geometry, pixel/average colors, image search/click, HTTP detection and Vision OCR again after the dependency pins were updated. Durable local evidence is in `target/macos-parity-evidence/final-input-result.json` and `final-screen-result.json`. The original user macros are byte-identical, temporary test macros are archived, and the verified fresh Bloodbath replay remains selected and linked to the local level.

The post-authorship workspace suite also passed (`/tmp/blockwork-post-authorship-workspace-tests.log`). Checks at `30f92a46f19ebab1cd7bff7f6b8e9c984c17e51a` passed Linux workspace tests/Clippy and both native Mac core/FFI jobs: https://github.com/Blockworked/Blockwork/actions/runs/40231358858.

The app artifact and release matrices now include native Intel Macs, alongside Apple Silicon. Both Mac app jobs are required rather than experimental. Each builds and validates the complete bundle, starts its packaged executable with `--help`, and runs the application QML tests with the installed Qt runtime. This adds Intel application coverage; it does not establish permission-dependent desktop input/capture on that runner. The new jobs must pass before counting this gate as verified. Multi-display hardware and a real installed-release update remain unexecuted coverage gates.

The real-archive updater fixture passed with the published 0.5.4 Apple Silicon archive (180,394,802 bytes; SHA-256 `12d70042ea7282a269606d6c9592d234a88f1c5fb00e4d3e30f6ba0f6701694a`). It extracts the archive using ditto, creates a disposable installed copy, calls the production replacement function with the rebuilt app, verifies the previous release's plist and executable bytes in the backup, validates the promoted bundle's signatures/dependencies, and starts its packaged CLI. The test removes its disposable install afterward. The live user installation is not replaced. This narrows the updater coverage gap; live GUI relaunch and the updater's network selection/download path are not established by this fixture. Evidence: `target/macos-parity-evidence/update-fixture/source.json` and `test.log`.

To repeat this opt-in integration gate after downloading a release archive and building the new bundle:

```sh
BLOCKWORK_TEST_RELEASE_ZIP=/absolute/path/blockwork-macos-arm64.app.zip \
BLOCKWORK_TEST_UPDATE_APP=/absolute/path/Blockwork.app \
cargo test -p blockwork-core --no-default-features --features updater \
  macos_released_bundle_upgrades_to_rebuilt_app -- --ignored --nocapture
```
