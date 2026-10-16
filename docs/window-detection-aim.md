# Window-scoped detection and aim control - spec (draft)

Status: M1, M2 and most of M4 implemented (see section 11). Intended for legitimate automation:
single-player worlds, local test servers, accessibility, and UI testing.
Do not use against real players on servers without permission. That violates
server rules and can trigger anti-cheat.

## 1. Problem

`FindImage` in `blockwork-core/src/macros/mod.rs:543` finds one PNG template
with per-pixel tolerance (`blockwork-core/src/screen/find.rs:12`,
`blockwork-core/src/screen/mod.rs:149`). It is 1:1 scale with no rotation or
scale invariance, searched over the full desktop
(`blockwork-core/src/screen/mod.rs:98`).

A Minecraft player breaks every assumption: different skins, armor, pose,
distance changes pixel size, lighting and particles change colors, nametags
move. One template per skin does not scale, and absolute cursor moves do not
aim an FPS-style view.

Window info in `blockwork-core/src/window.rs:7` has title/app only, no bounds.
Mouse output is one absolute or relative jump
(`blockwork-core/src/input/types.rs:191`, `blockwork-core/src/macros/runner.rs:1342`).

## 2. Goals

- Find instances of a class ("player") inside one window, skin independent.
- Return window-relative and screen boxes with confidence scores.
- Track across frames and pick one target (closest to crosshair by default).
- Convert pixel error into relative look moves with calibrated gain.
- Compose with existing variables, lists, `Repeat`/`While`, `TryCatch`, reporters.

Non-goals:

- No memory reading, injection, or anti-cheat evasion.
- No multiplayer cheat support, no hidden behavior.
- No full bot framework (pathing, combat logic). This is perception plus one
  control primitive.
- No new UI toolkit dependency in `blockwork-core`.

## 3. Proposed blocks

All new `InstructionKind` cases follow the existing `FindImage` shape:
`Value` slots for numbers/text, `String` names for variable and list targets
(see `blockwork-app/src/state.rs:575` and `blockwork-core/src/macros/fields.rs:212`).

### 3.1 `DetectObjectsInWindow`

Purpose: one detection pass over a single window, results into a list.

Fields:

- `pattern: Value` - window match, same semantics as `WindowAction`.
- `by: String` - `Any`, `Title`, or `App` (reuse `MatchBy`).
- `model: String` - model id, for example `mc-player-320-v1`.
- `label: Value` - label filter, for example `player`. Empty means all labels.
- `confidence: Value` - 0-100, minimum score to keep.
- `results_list: String` - destination list name, replaced each run.
- `count_variable: String` - variable receiving detection count, optional.

List item shape: one JSON text per detection, sorted by score descending:

```json
{"x": 812.0, "y": 401.0, "w": 64.0, "h": 128.0, "label": "player", "score": 0.87}
```

Coordinates are screen coordinates, same space as `MoveMouse Abs` and
`Shot::to_screen`. Box `x,y` is top-left, `w,h` is size in screen units.
Center is `x + w / 2`, `y + h / 2`.

Failure: follows `FindImage` soft-fail in
`blockwork-core/src/macros/runner.rs:874`. Capture or inference errors fail
the block for `TryCatch`. Zero detections is success with count 0, not error.

### 3.2 `WaitForDetection`

Purpose: block until a label appears or disappears in a window.

Fields: `pattern`, `by`, `model`, `label`, `confidence`, `appear: bool`,
`timeout: Value` in ms, 0 means never. Same polling shape as `WaitForImage`.

### 3.3 `AimStep`

Purpose: one proportional relative move toward a screen point.

Fields:

- `target_x: Value`, `target_y: Value` - screen point from section 3.5.
- `gain: Value` - pixels of mouse delta per pixel of error, for example 0.4.
- `max_step: Value` - clamp on each axis per step, pixels.
- `deadzone: Value` - stop moving inside this pixel radius.

Behavior:

```text
err = target - crosshair
if len(err) <= deadzone: done
delta = clamp(err * gain, -max_step, max_step)
move_mouse_rel(round(delta.x), round(delta.y))
```

One step only. Callers loop it with `Repeat`/`While`. Uses the same emulator
lock path as `MoveMouse`. Reports no variables.

### 3.4 `AimAtDetection`

Purpose: closed-loop convenience wrapper for the common case.

Fields: `results_list: String`, `index: Value` (0-based), `gain`, `max_step`,
`deadzone`, `timeout: Value`, `settle_ms: Value`, `status_variable: String`.

Loop: re-run detection, read item `index`, `AimStep`, sleep one frame budget,
exit when centered for `settle_ms` or when `timeout` elapses. Writes `hit`,
`timeout`, or `lost` to `status_variable`. `TryCatch` sees `timeout` as error
only if caller sets a strict flag; default is status string.

### 3.5 Value reporters

Reuse the `ExtOperator` pattern in `blockwork-core/src/input/value.rs:10`:

- `DetectionCount(list)` -> number.
- `DetectionX(list, i)`, `DetectionY`, `DetectionW`, `DetectionH`,
  `DetectionScore` -> number. `DetectionLabel(list, i)` -> text.
- `DetectionCenterX(list, i)`, `DetectionCenterY(list, i)` -> number.
- `WindowX(pattern)`, `WindowY`, `WindowW`, `WindowH` -> number. Needs a new
  `window::bounds` API (section 4.1).
- `CrosshairX(pattern)`, `CrosshairY(pattern)` -> window center by default,
  with optional fixed offset args later.

Index out of range is an evaluation error so `TryCatch` and conditions see it.

## 4. Runtime architecture

New module `blockwork-core/src/detect/` with three traits so tests run without
models or a display:

```rust
trait WindowCapture { fn capture_window(&self, pattern: &str, by: MatchBy) -> Result<Shot, String>; }
trait Detector { fn detect(&self, image: &Image) -> Result<Vec<Detection>, String>; }
struct AimController { gain: f64, max_step: f64, deadzone: f64 }
```

### 4.1 Window bounds and capture

Add `window::bounds(pattern, by) -> Rect` per platform:

- Windows: `GetWindowRect` plus client-area mapping (existing `place` code in
  `blockwork-core/src/window.rs:678` shows the handle lookup to reuse).
- Linux X11: reuse `x11::X11` in `blockwork-core/src/window.rs:451`, add geometry
  query. Wayland compositors need per-compositor paths; same gaps as current
  window control.
- macOS: extend the `osascript` helper in `blockwork-core/src/window.rs:706`
  with position and size, or use ScreenCaptureKit in native code.

Capture only that rect, then reuse `Shot` origin/scale handling. If the window
is minimized, occluded, or off screen, return a clear error.

### 4.2 Inference backend

Phase 1 uses an HTTP sidecar so no new native dependency lands:

- `Detector` posts PNG to `http://127.0.0.1:PORT/detect`, parses boxes.
- Works today with existing `HttpRequest` blocks as a manual prototype.

Phase 2 adds a native ONNX backend behind a cargo feature, for example
`ort`. Model files live outside macros, versioned and hash-pinned, downloaded
on demand with user consent. Default model suggestion: YOLOv8-nano fine-tuned
on Minecraft players, 320 or 480 input, single class plus optional `nametag`
helper class. CPU target is 50-120 ms per frame at 320. GPU is optional.

Model inputs need varied skins, armor, biomes, day/night, rain, particles,
distances from 3 to 40 blocks, FOV 70-90, and common resource packs.
Nametag OCR (`ScreenText` in `blockwork-core/src/input/value.rs:160`) stays a
fallback for known names, not the primary signal.

### 4.3 Tracking and selection

Keep a small per-macro tracker keyed by `results_list`:

- Match boxes frame to frame by IoU plus center distance.
- Keep stable indices, smooth centers with EMA, drop tracks after N misses.
- Default target is smallest distance to crosshair. Alternative modes later:
  highest score, closest by box height (proxy for distance).

### 4.4 Aim math

Crosshair defaults to window center. Pixel error is target center minus
crosshair. Gain converts error pixels to relative mouse units. Gain depends on
in-game sensitivity, DPI, and FOV, so it must be user calibrated per setup.
Clamp each step, pause inside deadzone, require `settle_ms` centered before
declaring success. Expose gain and deadzone as `Value` slots so macros can
tune them live.

Pointer-lock note: when Minecraft captures the pointer, absolute moves do not
place the cursor on the target. Only small relative steps rotate the view.
`ClickImage` behavior in `blockwork-core/src/macros/runner.rs:893` (absolute
move plus click) is the wrong primitive here. `AimStep` plus re-detect is the
correct loop.

## 5. Permissions, packaging, privacy

- New capabilities `screen` and `input` already cover these blocks; see
  `blockwork-app/src/commands.rs:902`. Detection models that download files
  also need `network` on first fetch, shown before import runs.
- Model files ship out of band, pinned by hash. Flatpak and installer flows
  must include or fetch the default model the same way `cargo-sources.json`
  pins other artifacts.
- Captures stay local. The HTTP sidecar binds loopback only. No screenshots
  leave the machine unless the macro itself sends them.

## 6. Frontend and DTO changes

- `blockwork-app/src/state.rs`: `InstructionDto` cases mirroring section 3,
  plus `FieldId` entries in `blockwork-core/src/macros/fields.rs:12`,
  `visit_values_mut` coverage in `blockwork-core/src/macros/mod.rs:62`,
  hash arms, and serde migration entries.
- QML: window picker reuses the app/window picker, model picker is a dropdown
  of installed models, list/variable pickers reuse existing widgets. Region
  picker from image blocks can select the window plus an optional sub-rect.
- Docs strings for each block state the Fair Play note from the header.

## 7. Testing

- Unit: coordinate mapping, gain math, clamping, deadzone, tracker matching,
  reporter parsing, DTO round trips.
- Screenshot fixtures: players with different skins at several distances,
  plus negative scenes with no players. Assert box centers within tolerance
  and zero false positives on empty scenes.
- Live ignored tests follow `blockwork-core/src/screen/mod.rs:237`: real
  capture plus detector timing, run by hand with `--ignored`.
- Perf budget: capture plus 320px inference under ~150 ms on reference CPU,
  servo loop stable at 5-10 Hz without input starvation.

## 8. Rollout

1. M1: `window::bounds` plus window-scoped capture, no model. Test with
   cropped `FindImage`.
2. M2: HTTP sidecar detector plus `DetectObjectsInWindow` writing lists and
   reporters from section 3.5. Manual macro: detect, pick closest, `AimStep`
   loop.
3. M3: native ONNX backend and model manager, same block API.
4. M4: tracker, `WaitForDetection`, `AimAtDetection`, QML pickers.
5. M5: packaging, permissions strings, model download UX, docs.

## 9. Interim workaround with current blocks

No new code: run a local detector service, `HttpRequest` a screenshot or
window crop, parse JSON with `JsonGet`, store centers in variables, and loop
`MoveMouse Rel` deltas with `Repeat`/`While`. Latency is higher and setup is
manual, but it validates gain and deadzone values before native blocks exist.

## 10. Open questions

- Default model license and hosting location.
- Minimum input size that still finds far players without missing near ones.
- Wayland capture coverage per compositor.
- Pointer-lock edge cases on each OS backend (`evdev`, `libei`, X11, Windows,
  macOS).
- Whether `results_list` JSON text is enough or lists need a struct item type.
- Policy UX: confirm dialog before a detection macro drives input in a game.

## 11. Implementation status

Done:

- `window::bounds` on Hyprland, Sway, KDE (kdotool to find the window, a
  KWin script for its client area), X11, Windows (client area) and macOS
  (whole frame), and `screen::capture_window`. On KDE Wayland that captures
  just the window's area through KWin's ScreenShot2 (`src/kwin.rs`,
  `screen/linux.rs`), which KWin 6.8 lets any app call; elsewhere, or if
  KWin refuses, it crops a whole-desktop capture (grim, then the portal).
- `blockwork-core/src/detect/`: `WindowCapture`/`Detector` traits, the
  loopback HTTP detector (`detect/http.rs`, reply format in its doc comment),
  the tracker (`detect/track.rs`) and `AimController` (`detect/aim.rs`).
- Blocks `DetectObjectsInWindow`, `WaitForDetection`, `AimStep`,
  `AimAtDetection`, with DTOs, permissions and QML rows.
- Reporters `WindowInfo(pattern, field)` and `Detection(field, item)`.

Differences from the spec above:

- `model: String` is `detector: Value`, a local URL. Non-loopback hosts are
  refused so captures stay on the machine.
- Lists are 1-based like every other Blockwork list, so `index` is too.
- The per-field reporters (`DetectionX(list, i)`...) are one
  `Detection(field, item)` reporter fed by `item i of list`, and
  `WindowX`/`CrosshairX`... are one `WindowInfo(pattern, field)`.
  `DetectionCount` is the existing `length of list`.
- `DetectObjectsInWindow` has a `sort` choice (best, closest to crosshair,
  largest) instead of always sorting by score. Items carry a tracking `id`.
- `AimStep` takes a window `pattern`/`by` so it knows the crosshair.
- `AimAtDetection` repeats the query that last filled its list, follows the
  item's tracking id, and writes `stopped` when the run is stopped. There is
  no strict flag; it never raises on `timeout`/`lost`.

Smoke testing: `examples/detector/detector.py --color ff0000` finds blobs
of one color with no model (Pillow and numpy), or `--yolo weights.pt` runs
an Ultralytics model. With it running, `cargo test -p blockwork-core --lib
finds_a_red_square -- --ignored` checks the HTTP path end to end.

Also done: aim blocks ask once per macro per session before moving the
mouse (a yes/no dialog; "no" fails the block, cancel stops the run), and
the detect blocks take an optional `region` (`x,y,width,height` inside the
window, picked on screen with the block's region button). The crosshair
stays the window center when a region is set.

Not done: M3 (native ONNX backend, model manager and downloads) and
screenshot fixture tests (both need a chosen, trained model).
