# Android development port

This development port includes the runtime/build foundation, accessibility
touch/element automation, native picking/capture, foreground app triggers,
Android alarms and a compact touch editor. The full Android release gate is
**not met**. The remaining phone target-size audit, reliable long-running jobs
without accessibility, upstream QML publication and device validation are unfinished.

## What is implemented

- Qt 6.10.3 shared editor, packaged by CMake for API 30–36, ARM64 and x86-64.
- One Rust `blockwork-app` runtime per application process. Android's
  `Application` loads it independently of Qt. `QtQuickView` attaches the editor
  to an ordinary Android activity; destroying that view releases its event
  subscription without stopping the runtime. The standard `QtActivity` is not
  used because its destruction calls `System.exit`.
- Private storage through the existing config-directory override. Import uses
  `ACTION_OPEN_DOCUMENT` or `ACTION_SEND`, copies the document into private
  storage, then uses the existing import-review command. Export uses
  `ACTION_CREATE_DOCUMENT`; content URIs never masquerade as filesystem paths.
- Host startup policy disables desktop input hooks, tray/autostart integration,
  update checks, browser/webhook servers and polling watchers. CUDA and local
  object-detection dependencies are absent from the Android dependency graph.
- Host capabilities and readiness in `StateDto.capabilities`, separate from
  per-macro grants. Palette filtering and canvas markers preserve unavailable
  imported blocks. Native startup/settings commands cannot enable desktop
  services. Desktop startup defaults remain enabled.
- `Touch` blocks: tap, long press, swipe, Back, Home and Recents. Gestures are
  serialized across strands and advance only after Android's completion
  callback. Cancellation, timeouts, service loss and locked-device failures
  reach the runner. Coordinates are physical default-display pixels, including
  system bars, as reported by `Display.getRealMetrics`.
- Focused text insertion via editable accessibility nodes exposing text and
  selection. Other controls return an error; no raw key injection or passive
  touch recording is advertised.
- Native prompts, with a background notification and cancellation. A floating
  accessibility-overlay Stop button and notification action stop active runs.
  Notifications require the user's notification permission on Android 13+.
- A run-boundary journal records interrupted workflows on next process start.
  It restores documents, never partially executed actions.
- Element blocks resolve package plus resource ID, text, description and class
  together against the current application accessibility trees. Click, long
  click, scrolling, explicit text replacement, existence/text reads and waits
  are implemented. Actions and text reads require a unique match. Every request
  traverses again; nodes are released before its reply.
- Selector fields match exactly and are case-sensitive. Resource IDs use the
  complete Android name, such as `com.example.app:id/save`. Empty optional
  fields are ignored; the package and at least one matching field are required.
  Element reads require the per-workflow screen grant; modifying actions also
  require input. Device accessibility readiness remains separate from both.
- Android element existence and text reporters accept the same five selector
  fields. Existence returns false for absence and true for any match; text reads
  reject ambiguity, missing text and password fields. Blocks can also store
  existence/text results in a workflow variable.
- Native point, region and element picking in `TYPE_ACCESSIBILITY_OVERLAY`.
  Switch to a target app before tapping Select. Confirm/Cancel remain visible;
  Move relocates the controls, Retry returns to app navigation, and dragging a
  region corner resizes it. Point/region picking includes a magnifier. Touch
  blocks expose separate start/end picks; element blocks expose Pick element.
- Element picks traverse the frontmost application window, exclude Blockwork
  and system/keyboard overlays, omit editable/password text from selectors, and
  revalidate selector uniqueness against a fresh tree on confirmation. All
  available ID, static text, description and class fields match together;
  dynamic fields can be removed in the editor afterward.
- Accessibility screenshots feed existing pixel/average-color reporters and
  Find/Click/Wait-for-image blocks through the core screen backend. Android
  monitor geometry uses physical default-display pixels, origin (0, 0), scale 1.
  Screenshot pixel data crosses JNI directly rather than passing JSON images.
- Captures share the core's short reuse window. Native callers coalesce, with
  attempts at least 500 ms apart, one delayed retry for Android rate errors,
  and a callback timeout. Stop/picker overlays are hidden for two compositor
  frames before capture and restored after completion/failure. Lock, service
  loss and geometry changes invalidate pending captures and cached pixels.
- Confirmed picker images/colors are captured before reopening the editor.
  Picking shows reference display dimensions and cancels on rotation/geometry
  changes, lock or service loss. No separate overlay permission is requested.
  The navigation drawer offers point and region tools; OCR remains unavailable.
- `WhenForegroundApp` is a separate Android header for exact, case-sensitive
  package entry/exit. It observes default-display application windows, preferring
  active/focused windows. Keyboard and accessibility/system overlays, plus the
  Android/System UI/standard permission-controller packages, are excluded.
  Home/launcher and the Blockwork editor are real app transitions.
- Foreground observations are debounced by 100 ms. Duplicate observations do
  not fire; the first observation after startup, unlock, service reconnection or
  picking establishes a baseline. Picking suspends observation, cancels pending
  trigger gates and reestablishes the baseline afterward. Paused triggers still
  consume observations, so Resume never replays transitions seen while paused.
  Other macros follow the existing Always listen setting. Imported desktop
  power/window constraints explicitly skip the event and log why; Android
  cannot yet evaluate those constraints. Active hours/days retain their existing
  local-time behavior.
- Foreground-triggered bodies require the workflow's screen grant. Native
  readiness is checked again after permission prompts; denial or service loss
  prevents the body from starting. Temporary permission answers are cleared even
  when a trigger ends before a strand starts. Completed trigger results refresh
  the open editor's variable/list previews as well as saved documents.
- `LaunchAndroidApp` launches a package's main activity in the current profile
  through Android's package launcher intent. It requires the workflow's commands
  grant, an enabled accessibility service and an unlocked/interactive device.
  Use an Element Wait after launching; acceptance of an intent does not mean
  the destination is ready. Missing/disabled/non-launchable apps and Android
  launch denials return errors, including inside Try/Catch.
- Both Android app blocks have a searchable app chooser, limited to launcher
  apps visible in the current profile. The chooser also searches package names,
  fits a narrow window, and reduces its columns on phones. Package names remain
  editable. The manifest queries launcher activities; it does not request
  QUERY_ALL_PACKAGES. No force-close, shell strings, intent URLs or arbitrary
  desktop launch options are exposed by these blocks.
- `WhenTime`, `WhenCron` and `WhenAt` use AlarmManager on Android. Daily,
  weekly, monthly, yearly, five-field cron and once-at schedules share due-time
  calculation with desktop catch-up. Android uses the Java system's IANA time
  zone through the bundled `chrono-tz` rules. Saved variables and pure built-in
  expressions work in schedule values; live host reporters, random/current-time
  reporters and custom calls are rejected during planning.
- Best-effort RTC wakeup alarms are the default. Per-workflow **Exact time on
  Android** requests `setExactAndAllowWhileIdle` and requires
  `SCHEDULE_EXACT_ALARM` access on Android 12+. Missing or revoked access blocks
  that workflow's schedule with a visible status/notification; it never
  downgrades to best-effort. Android 11 needs no separate exact-alarm grant.
  Both modes remain subject to Android idle restrictions and quotas.
- Private `android-alarms.json` persists occurrence tokens and consumed local
  date/minute identities. The history is written, fsynced and atomically renamed
  before a run starts. Duplicate/stale/early broadcasts cannot replay an action.
  Repeated DST wall minutes and a backward clock change preserve consumed
  identities. Unreadable history blocks scheduling instead of forgetting past
  deliveries. Workflow documents cannot grant themselves scheduling access.
- Future alarms reconcile after edits, variable/time changes, startup, reboot,
  package replacement, timezone/clock changes and exact-access changes. The
  native process checks desired state once a second; OS alarms own delivery.
  Catch-up coalesces missed occurrences to the latest one within a week.
  On recovery, an already-armed occurrence up to 90 seconds overdue is treated
  as a late delivery, matching the desktop scheduler's short-gap tolerance;
  older missed occurrences honor the workflow's catch-up setting.
  A genuinely delivered inexact alarm can be late; lateness alone is not a
  reason to downgrade or replay it. Force-stop prevents delivery until reopening.
- Scheduled UI automation needs an unlocked interactive device, connected
  accessibility service and no active picker. Unready occurrences are consumed
  and logged as skipped, so unlocking does not replay them. Permission prompts
  recheck readiness before the body starts. Desktop power/window constraints
  explicitly block Android schedules; active hours/days retain their normal
  behavior. Emergency Stop's trigger pause persists across restarts until Resume.
- Logic-only schedules can start without accessibility. The receiver holds a
  short wake lock and at most an eight-second execution lease. Android may stop
  an unbound process after that; reliable long jobs without accessibility need
  a JobService/foreground host and remain **unfinished**. Bound accessibility
  normally keeps UI runs alive, but this still needs device/Doze validation.
  Process death is journaled; a consumed occurrence never replays partial input.
- Macro format 7 preserves Android blocks/settings and the updated master pack
  metadata on desktop and Android (3 added touch, 4 elements, 5 foreground
  events/launches, 6 exact-time settings, 7 combines the pack-state formats).
  Earlier formats
  upgrade through the existing migration steps. Desktop `WhenApp`, `OpenApp`
  and `CloseApp` retain their original behavior and stay unavailable on Android.

Android does not expose a public method to cancel an already dispatched
accessibility gesture. Stop cancels the Rust wait and subsequent/queued
actions. The native queue retains its active slot until that gesture's callback,
so a later run cannot overlap it. A gesture already in progress may finish.

Below 720 logical pixels the editor keeps the full canvas visible and opens
searchable Add Blocks and Activity sheets. The shared Blockstitch canvas handles
one-finger empty-space pan, block/value drag, long-press menus and two-finger
pan/pinch. A second contact cancels an armed/active block drag without a drop;
lifting one finger does not resume that drag. Tap a canvas block to select its
following insertion gap (shown in blue), then tap a palette block to insert.
Without a valid gap, insertion finds free space in the visible canvas. Event
headers always create their own strand. Mouse double-click positions and desktop
wheel/keyboard behavior are preserved.

Common menu rows, search fields, sheet actions and Android navigation use
48-pixel targets. Default dialogs are bounded and scroll when needed, and a
keyboard resize pans a focused canvas field into view. The plain Android Activity
forwards Back to Qt: dismiss the keyboard first, then the topmost popup/menu,
then the editor sheets/navigation, then background the Activity. Runtime work
continues. Native Back/IME behavior still requires device validation. Some inline
block controls and custom dialogs need a complete target-size/large-text audit;
milestone 4 is not yet fully validated.

## Device validation fixture

A separate native fixture app and eleven instrumentation cases now build for
the Android host. They exercise real workflows against deterministic controls,
text/password fields, replacement/delayed nodes, a scrolling viewport and a
four-color gesture/capture target. A strict protected-window check exposes the
remaining secure-layer masking gap. Device execution is unverified; no device
is connected here. See [android-device-validation.md](android-device-validation.md)
for build/install commands, explicit-device selection, evidence collection and
the optional service-revocation check.

## Build an installable development APK

Use a Linux host with Python 3.11+, Rust stable, CMake, Ninja and JDK **17**.
Install Android command-line tools and accept the SDK licenses, then install:

```sh
sdkmanager 'platforms;android-36' 'build-tools;36.0.0' 'platform-tools' 'ndk;27.2.12479018'
rustup target add aarch64-linux-android x86_64-linux-android
python3 -m pip install aqtinstall==3.3.0
python3 -m aqt install-qt linux desktop 6.10.3 linux_gcc_64 -O "$PWD/Qt" --archives icu qtbase qtdeclarative qtsvg qttools
python3 -m aqt install-qt all_os android 6.10.3 android_arm64_v8a -O "$PWD/Qt"
python3 -m aqt install-qt all_os android 6.10.3 android_x86_64 -O "$PWD/Qt"
```

The newer Qt Android kits use the `all_os` feed, not `linux android`.
The host kit must match the target kit exactly. ICU is needed by the Linux
host tools even though it is not linked into the Android application.

```sh
export QT_ROOT="$PWD/Qt"                     # contains 6.10.3/
export ANDROID_SDK_ROOT=/path/to/android-sdk
export ANDROID_NDK_ROOT="$ANDROID_SDK_ROOT/ndk/27.2.12479018"
export JAVA_HOME=/path/to/jdk-17
export LD_LIBRARY_PATH="$QT_ROOT/6.10.3/gcc_64/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
scripts/build-android.sh arm64-v8a
scripts/build-android.sh x86_64
adb install -r dist/android/blockwork-x86_64-debug.apk
adb shell am start -n com.blockworked.blockwork/.BlockworkActivity
```

Cargo and Android CMake use the same tracked `vendor/blockstitch-qml` source
snapshot. It is based on Blockstitch `0bfc7874bc864024e15ea43cd6aace34b9027d85`
with the shared touchscreen changes from the sibling checkout. Core/FFI keep
their existing Git pin. Publishing the shared QML changes upstream and replacing
the snapshot with an updated Git pin remain follow-up work; a clean build does
not depend on unpublished Git objects. `android-deps.py` fetches Lucide at its
`Cargo.lock` revision. Set `LUCIDE_QT_DIR` to reuse an existing checkout; the
build verifies its HEAD. The Android CMake
adapter maps the shared manifest's `QuickShapes` component name to the
`QuickShapesPrivate` target exported by Qt's Android kit.

Rust and Qt code are optimized; Gradle debug-signs the APK so it can be installed
directly. Release signing and publishing are not part of this script. ELF load
segments use 16 KB alignment, and the runtime has a stable library SONAME.
The `Android APK` GitHub workflow builds both ABIs and uploads debug APKs.

In a proxied build environment, configure Gradle's JVM proxy and truststore using
the environment's documented proxy and CA. Java does not automatically inherit
the HTTP proxy variables used by Cargo and aqt. Keep certificate verification
enabled.

## Validation

```sh
cargo test -p blockwork-android --lib
cargo test -p blockwork-app --no-default-features --lib
cargo test -p blockwork-core --no-default-features --features system,ipc --lib --tests
python3 blockwork-qt/tests/run.py
# After building an ABI:
(cd target/android/arm64-v8a/android-build && ./gradlew testDebugUnitTest)
```

The runtime test races Application/service startup, disconnects the editor while
a real workflow runs, waits for its saved variable result, reconnects, tests
cancelled prompts, and simulates gesture callbacks, concurrent strands,
cancellation and service loss. It also round-trips touch blocks through frontend
DTOs and saved macros. Element checks exercise retry-after-absence, text result
storage, live reporter evaluation and a catchable timeout while the native reply
is stalled. Screen checks cover shared captures, ARGB conversion, error
propagation, late replies after revocation and invalidation during rotation.
JVM tests cover selectors and physical-pixel point/region bounds, reverse drags
and landscape. QML checks cover confirmed images, touch endpoints and selector
replacement without recapturing the editor. Foreground checks exercise entry/exit,
repeat observations, baseline resets, pause/resume, grant denial and revocation
while a prompt is pending. Launch failures are exercised through the real
runner's Try/Catch path. These checks do not substitute for Android device tests.

On a test device, enable Blockwork automation from Accessibility settings using
the in-app explanation. APK installations on some Android versions also need
App info → menu → Allow restricted settings. Confirm that disabling the service
and locking the screen stop active runs. Test a gesture and focused insertion
against a fixture application before running against other apps.

## Remaining release work

1. Run the compiled fixture instrumentation on devices, then extend coverage
   to native picking/custom blocks, window layering, overlay exclusion, magnifier
   alignment and density/orientation. Real service/lock lifecycle results remain
   unverified.
2. Secure-window behavior across Android versions. Screenshot API errors are
   surfaced explicitly, including secure-content denial. Android versions that
   mask secure layers in default-display screenshots may return black pixels
   without an error; reliable detection of that case is **not implemented**.
   The plan's secure-content error requirement is not yet satisfied on those
   devices. Reference dimensions are displayed during picking; persisting a
   per-workflow reference display remains unfinished.
3. Phone UI device validation (touch, nested menus, keyboard/Back, safe areas,
   rotation and large text), the remaining inline/custom-dialog 48-pixel target
   audit, and publication of the shared Blockstitch QML changes to update the
   upstream Git pin. A tracked shared snapshot currently keeps builds repeatable.
4. Android alarm device validation: best-effort/exact delivery, denied/revoked
   exact access, Doze, boot, clock/timezone changes, late delivery, force-stop
   recovery and permission prompts. Reliable long-running logic-only jobs need
   an Android job/foreground host. Native foreground events and app launches
   still need device checks, including
   OEM system dialogs, keyboard transitions, split-screen focus, background
   launch restrictions, missing packages and managed profiles.
5. Android 11 and 16 device/emulator validation, a physical ARM64 16 KB-page
   device, and the complete phone-built workflow in the plan's release gate.

The Android host starts no desktop polling scheduler. Its time headers use
AlarmManager; foreground headers use accessibility window observations.
Process-based `WhenApp` headers stay preserved and unavailable on stock Android.
