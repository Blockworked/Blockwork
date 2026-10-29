# Android validation record

Development build, 2026-10-29. See [android.md](android.md) for implemented
scope, reproducible commands and unfinished release work.

## Build artifacts

- `dist/android/blockwork-arm64-v8a-debug.apk`
- `dist/android/blockwork-x86_64-debug.apk`

Both builds use Qt and host tools 6.10.3, NDK r27c, JDK 17, minimum API 30,
and target/compile API 36. Gradle debug signing succeeds. `apksigner verify`
and `zipalign -c -P 16 4` pass for both APKs. Manifest inspection confirms
the ABI and SDK levels. No external-storage permission is requested.

The Rust ARM64 library's ELF load segments have `0x4000` alignment. The Qt host
links `libblockwork_android.so` by SONAME, with no build-machine path in that
dependency.

## Automated checks

- Shared core: 246 passed; six existing desktop/live-screen tests ignored.
- Application library with desktop features disabled: 93 passed.
- Embedded Android runtime lifecycle/automation test: passed.
- Shared QML editor suite: 216 passed, no failures. The source-only harness
  emits icon-resource and stale-callback warnings; it does not bundle the
  application icon resource.
- Focused host-vocabulary suite: eight passed. This includes Android palette
  filtering, recording availability, imported desktop block preservation,
  confirmed picker images/touch endpoints, selector replacement, Android app
  package selection and desktop preservation of Android headers/actions.
- Shared Blockstitch QML suite: 116 passed, including touch gestures, second-contact
  cancellation, nested insertion, keyboard resizing and bounded scrolling dialogs.
- Default desktop `cargo check -p blockwork-app` and `cargo check -p blockwork-qt`: passed. Android dependency
  inspection confirms no Enigo, clipboard/battery desktop libraries, updater,
  ONNX or CUDA in the native Android graph.

## Device validation is blocked

An Android 11 / API 30 Google APIs x86-64 emulator was created and booted with
software emulation because this environment has no `/dev/kvm`. Android's
system process repeatedly died before Blockwork could be installed. System UI,
phone and permission-controller crash logs contained `DeadSystemException`.

Two install attempts failed in the Android package service:

```text
cmd: Can't find service: package
cmd: Failure calling service package: Broken pipe (32)
```

Activity launches then failed with `Can't find service: activity`. These
failures preceded application startup. No app launch, accessibility gesture,
text insertion, document-provider flow or activity-destruction result is claimed
from this emulator. The host runtime test exercises editor detach and workflow
persistence in-process; it is not a substitute for that device validation.

Android 16, ARM64 hardware, 16 KB-page device execution, density/orientation
behavior, restricted settings, native prompt notifications and the full plan's
release workflow remain unverified.

## Element automation continuation

Element selectors/actions/reporters and timeout waits were added after the
foundation build. Four JVM selector tests pass, and the five focused host QML
checks pass with the Element palette entry and both Android reporters. The Rust
runtime test now exercises fresh existence retries, a text read stored in a
variable, reporter evaluation, all element actions through persistence/frontend
DTOs, and a stalled reply that times out inside Try/Catch.

No native element action on a device is claimed. The picker, fixture application
instrumentation, real-tree ambiguity/staleness tests and Android permission
revocation tests still require device validation.

## Native picking and capture continuation

The native accessibility-overlay picker and screenshot bridge now build in
both ABIs. Four additional JVM geometry tests pass (eight JVM tests total),
covering physical-pixel display edges, reverse region drags, landscape and
unavailable displays. The Rust runtime test additionally checks coalesced
captures, ARGB conversion, pixel/average-color reads, display-size mismatch,
revocation, late callbacks, rotation invalidation and capture-error propagation.
The complete QML suite reports 209 passed; the focused host suite reports seven.

These checks do not exercise Android's compositor, actual accessibility
screenshots, overlay window placement or native touch selection. No working
device/emulator is available here, so native picking, magnifier alignment,
overlay exclusion, rate-limit behavior, permission revocation and geometry
changes on Android remain unverified.

Secure-content error codes are propagated. Older Android default-display
captures can instead mask protected layers with black pixels. Detecting that
case is unresolved; the plan's secure-content error requirement remains open.
See the remaining release work in android.md.

## Foreground app events and launches continuation

`WhenForegroundApp` and `LaunchAndroidApp` are included in format 5, with
frontend/persistence round trips on desktop and Android. Twelve JVM checks pass
(four selectors, four pick geometry, four foreground-window/package checks).
The window checks cover keyboard/system/accessibility-overlay exclusion,
standard permission-controller exclusion, split-screen focus, default-display
filtering, launcher/editor windows and absence of app windows.

The native runtime integration test exercises real triggered strand execution
for entry/exit, duplicate suppression, baseline resets, pause/resume, a denied
screen grant, and service loss while a grant prompt is pending. It verifies
saved trigger results appear in the editor's previews. Imported desktop power
and window constraints are explicitly skipped with an error instead of using
unsupported platform fallbacks. It also routes a missing
launcher error through the actual runner's Try/Catch block. The app suite's
83 passing checks include foreground/launch import-review grants. The vocabulary
also builds with all system features disabled, and the default desktop app check
passes.

No Android device result is claimed for accessibility window delivery, OEM
permission dialogs, keyboard animations, split-screen transitions, package
visibility, managed profiles or background activity-launch restrictions.
These remain release validation work. AlarmManager scheduling and the complete
phone editor are still unfinished.

## Android alarms continuation

Format 6 adds per-workflow Android exact-time settings. The app suite now has
93 passing checks, including ten scheduler cases: persisted claims, stale and
early delivery, unreconciled timezone/time edits, exact denial/revocation, restart catch-up, pause restoration,
clock rollback, timezone changes, DST gaps/repeated minutes, short months/leap
years, bounded catch-up, unsafe schedule expressions and unreadable history.

The runtime integration test executes a scheduled logic strand, verifies its
result, rejects duplicate delivery, and consumes a UI occurrence skipped while
unready. It also checks that accessibility changes preserve exact-alarm access.
The shared core still has 246 passing checks (six existing ignored tests), the
full QML sweep has 216 passing checks, and the focused host suite has eight.
Fourteen JVM checks pass, including exact-versus-best-effort policy and opaque
broadcast-token validation. The default desktop and system-free vocabulary
build checks pass.

The manifest now requests RECEIVE_BOOT_COMPLETED, SCHEDULE_EXACT_ALARM and
WAKE_LOCK for scheduling. It does not request USE_EXACT_ALARM, all-package
visibility, external-storage access or a separate overlay permission.

These checks do not demonstrate OS alarm delivery, reboot recovery, real exact
access grant/revocation, Doze, force-stop or locked-device behavior. Those need
Android 11/16 devices. The receiver's short execution lease is not reliable
hosting for long logic-only runs without the bound accessibility service;
a job/foreground host remains release work.

## Phone editor continuation

Below 720 logical pixels, the full-width canvas uses searchable Add Blocks and
Activity sheets. Shared touch handling covers empty-space pan, block drag,
long-press menus, two-finger pan/pinch, immediate cancellation on the second
contact, and no resumed drag after one finger lifts. A selected block shows its
following insertion gap; palette taps insert there or into free visible space.
Equivalent backend state pushes preserve selection; document edits clear it.
Default dialogs are bounded and scroll, and a resized canvas reveals its focused
field. Native Activity Back is forwarded to Qt's popup stack before backgrounding
the editor. These native Back/IME paths are compiled, not device-validated.

The editor suite has 216 passing checks, including six phone-suite checks at
320/360 pixels and a 720-pixel layout transition. The shared suite has 116 passing
checks, including thirteen touch-canvas checks and three bounded-dialog checks.
The focused phone suite also passes against the final category-sheet layout.
A rendered 360-pixel sheet was inspected. CI now runs both source QML suites.
The source harness still emits existing resource/stale-callback warnings.

Both APKs were rebuilt. Signature and 16 KB ZIP alignment checks pass. All
packaged native ELF load segments have at least 16 KB alignment; runtime/editor
payloads match the local build outputs, staged Kotlin matches source, and the
Back bridge methods are present in DEX. Fourteen JVM checks pass. Artifact sizes:
ARM64 133,255,595 bytes; x86-64 148,813,123 bytes.

Cargo and CMake use the shared touch changes published on Blockstitch's `android`
branch at `26ad841108537238494441bf3d5fe0fad84586c1`. QML, core and FFI use
the same Git pin. The Android dependency helper fetches the locked source so a
clean build does not require a sibling checkout.

Real touch/IME/Back, safe areas, rotation, large text and all inline target sizes
still need device validation. Secure-layer detection, per-workflow reference
dimensions, reliable long background jobs without accessibility, Android alarms
under Doze/reboot, and the complete release-gate workflow remain unfinished.

## Fixture and instrumentation continuation

The standalone fixture APK and ABI-specific instrumentation APKs now build for
ARM64 and x86-64 alongside the main app. Eleven device cases compile, including
real workflow execution, fresh/ambiguous selectors, insertion/replacement,
password rejection, physical gestures, concurrent strands, cancellation,
screenshot colors/coalescing, package launch, strict secure capture and optional
service revocation. No test hooks were added to the production app. All five
APKs pass signature and alignment checks; instrumentation source matches both
staged builds, and test classes are absent from the main/fixture APKs.

Six fake-adb host checks pass for runner success/failure handling, setting
restoration, argument preservation, explicit-device failure and install-only
behavior. The existing fourteen JVM checks also pass. These are host checks.
`adb devices` has no connected device; **none of the eleven instrumented tests
has run here**. Android's secure-layer masking remains an unresolved requirement
and is intentionally a failing device assertion when a bitmap is returned.

See [android-device-validation.md](android-device-validation.md) for the fixture
surfaces, commands, limitations and recorded device evidence format. CI builds
and uploads the fixture and instrumentation APKs without claiming device runs.

## Rebase onto updated master

The earlier Android rebase incorporated master `18e71d4`. The shared QML touch changes
live in Blockstitch's `android` branch, based on its master `c6276ed`, preserving
the boolean-slot, block-editor and accessibility work. Blockwork pins QML, core
and FFI to `26ad841108537238494441bf3d5fe0fad84586c1` from that branch.
Macro format 7 combines master's pack-state changes with Android formats 3–6.
Migration checks preserve pack export metadata and touch actions from versions
3 and 6; private-pack migrations retain original-file backups for versions 2,
3 and 6. Base64 remains available on Android for the new community module. The shared
image-click hook delegates to master’s ordered desktop-click path while keeping
Android’s cancellable tap implementation; a regression test covers delegation.

Core checks pass (256 unit, three baseline and two descriptor checks, six existing
ignored tests), the app library has 99 passing checks, the embedded Android
runtime check passes, and ARM64 Android compilation passes. The shared QML suite
has 149 passing checks. Device execution remains unverified. Existing local APKs
predate this rebase; use the next CI build or rebuild them from this branch.

The editor QML suite has 281 passing checks and one failing drag-rejection check
(`PaletteDrag::test_rejected_drop_lets_go_and_shows_the_error`). That check passes
in isolation; in the failing class run no edit request reached the mock backend.
The test-order interaction remains unresolved.

After publishing the shared changes to Blockstitch's `android` branch, all 153
shared QML checks and six focused phone-editor checks pass. Fresh dependency
fetches resolve both locked revisions, and the Android build's revision check
accepts those checkouts and rejects a mismatched Blockstitch HEAD. ARM64 Rust
compilation and Android CMake configuration pass with the published dependency.
The desktop `blockstitch-qml` crate also compiles from the same Git pin.

## Rebase onto master 02e3e86

The Android branch now includes the 29 master commits through `02e3e86`, including
macOS parity, screen/keyboard fixes and the updated editor test harness. Desktop
battery validation and its tests are preserved behind the Android platform split.
Pixel reads preserve injected hosts' permission and secure-capture errors while
keeping the desktop off-screen error behavior.

Cargo, release/artifact workflows, the macOS QML probe, Android CMake dependencies
and Flatpak use the published Blockstitch commit
`26ad841108537238494441bf3d5fe0fad84586c1`. Flatpak's source list also includes the
four locked timezone dependencies required by Android scheduling. Every registry
package in `Cargo.lock` has a matching archive checksum in that list. No shared
Blockstitch source is copied into this repository.

Core checks pass (264 unit, three baseline and two descriptor checks, six existing
ignored tests), as do 100 app-library checks, the embedded Android runtime check,
ARM64 Android compilation and all 153 shared QML checks. Five release-preparation
checks and six device-runner host checks pass. Device execution remains unverified;
existing APKs predate this rebase.

The full editor QML run has 289 passing checks and one failure in
`MacroPrompts::test_text_keyboard_and_return` (the text field lacks active focus).
The same prompt class reproduces that failure on unmodified master `02e3e86`
with master's Blockstitch pin `c6276ed`, as well as on Android's pin. The earlier
drag-rejection failure now passes in the full editor run. The source harness
still emits resource/stale-callback warnings. Native macOS and Android device
execution were not repeated here.
