# Controllers

Blockwork reads physical game controllers for triggers and reporters, can
record them, and on Linux can play controller input back through a virtual
gamepad. Everything here is off until it's turned on in
Settings > Integrations.

The code lives in `blockwork-model/src/input/gamepad.rs` (logical controls,
deadzone, hysteresis), `blockwork-core/src/input/controllers.rs` (the reader),
`blockwork-app/src/watch/controllers.rs` (triggers),
`blockwork-core/src/input/virtual_gamepad.rs` and `native_gamepad.rs`
(virtual output) and `blockwork-core/src/recording/controller.rs` (recording).

## Logical controls

Blocks name controls by position, not by a device's own numbering, so one
macro works with different brands:

- Buttons: `South`, `East`, `North`, `West` (the four face buttons, by where
  they sit), `LeftShoulder`, `RightShoulder`, `LeftStick`, `RightStick` (stick
  clicks), `Select`, `Start`, `Mode`, `DPadUp`, `DPadDown`, `DPadLeft`,
  `DPadRight`.
- Axes: `LeftX`, `LeftY`, `RightX`, `RightY` from -1 to 1, and
  `LeftTrigger`, `RightTrigger` from 0 to 1.

## Choosing a controller

A block's controller slot holds a **profile**: the controller's model id and
name. An empty slot means "the only controller connected". The controller
picker lists what's connected now. If two identical controllers are
connected, a profile matches both, and the block reports an error instead of
guessing; disconnect one, or pick by another profile. Blockwork's own virtual
gamepad is never listed.

## Triggers

- **When controller button is pressed/released** fires on each change,
  including presses shorter than the poll, in order.
- **When controller connects/disconnects** fires when the profile appears or
  goes away.
- **When controller axis crosses its threshold** fires when the axis's
  distance from center (after the deadzone) reaches the threshold (0.05 to
  1), or falls back below it. An axis counts as above from the threshold and
  stays above until it drops 0.05 below it (halfway down for thresholds under
  0.1), so a noisy stick resting near the threshold fires once.

A state that was already true when a strand appeared, when Blockwork started
or when a controller (re)connected is a baseline, never a trigger. A
disconnected controller's buttons and axes are unknown rather than released:
unplugging while holding a button fires only the disconnect trigger, and
plugging back in with the button held fires only the connect trigger. If
events arrive faster than they can be handled, the queue is dropped and
triggers resynchronize without replaying stale presses; the run log says so.

The **deadzone** setting (default 12%) zeroes small stick movement near the
center and rescales the rest, so the output is still continuous from 0 to 1.

## Virtual controller output (Linux)

The controller blocks (press/release/click a button, set a stick, set a
trigger, reset) drive one virtual gamepad, "Blockwork Virtual Gamepad",
created through `/dev/uinput`. It needs Virtual controller output turned on
and write access to `/dev/uinput`. Windows and macOS have no virtual
controller output; the blocks report that.

One strand owns the virtual gamepad at a time. When that strand stops, is
paused, fails to send, or the setting is turned off, every button is
released and every stick and trigger is centered.

## Recording

With **Record controller input** on (Recording settings, needs controller
input), the Record button records the first controller that changes. Button
changes become press and release blocks; sticks and triggers become set
blocks, kept to two decimals and sampled at most every 16 ms. Movement inside
the deadzone isn't recorded. A recording that ends with a button held or a
stick off center ends with a reset block, so playback never leaves the
virtual gamepad pressed. Recorded blocks play back through the virtual
gamepad, so they need Linux to replay.

## Starter template

**Controller to keyboard** holds Space while the controller's bottom face
button is held: two strands, one on press and one on release.

## Limits

- One virtual gamepad; force feedback and motion sensors aren't read or sent.
- Physical controllers are read through gilrs (evdev on Linux, XInput and
  Windows.Gaming.Input on Windows, IOKit on macOS).
- Disconnect/reconnect, hot plugging and multi-controller setups are covered
  by tests with simulated devices; they still need qualification on real
  hardware.
