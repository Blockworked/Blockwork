# Touch paths

Blockwork plays touch input as **touch paths**: complete, timed frames of
every finger on the screen. The format is the same on every platform and is
meant for other tools too (a future Android adapter reads it unchanged), so
this page is its public description.

The code lives in `blockwork-model/src/input/touch.rs` (format, limits and
gesture planner), `blockwork-core/src/input/touch_playback.rs` (preflight and
cleanup), `blockwork-core/src/input/native_touch/` (platform providers) and
`blockwork-core/src/recording/touch.rs` (recording).

## Format, version 1

A touch path is one JSON object:

```json
{
  "version": 1,
  "frames": [
    { "at_ms": 0,   "contacts": [ { "id": 0, "x": 200000, "y": 500000 } ] },
    { "at_ms": 16,  "contacts": [ { "id": 0, "x": 230000, "y": 500000 },
                                  { "id": 1, "x": 600000, "y": 500000 } ] },
    { "at_ms": 120, "contacts": [] }
  ]
}
```

- `version` is `1`. Anything else is refused, so a newer file never plays as
  if it were an older one.
- `frames` is the whole screen at each moment, not a list of changes. A
  finger present in one frame and absent from the next lifts between them; a
  finger absent and then present goes down.
- `at_ms` is milliseconds from the first frame. It starts at `0` and strictly
  increases.
- `id` names one finger for as long as it stays down, `0` to `9`. An id may
  be reused after its finger lifts.
- `x` and `y` are integers. Their unit depends on the block that holds the
  path (see Coordinates).

### Rules

A path that breaks any of these is refused before anything is sent:

| Rule | Limit |
| --- | --- |
| Frames | 2 to 40,000 |
| Duration (last `at_ms`) | at most 600,000 ms (ten minutes) |
| First frame | at `0`, with at least one contact |
| Last frame | no contacts, so every finger is released |
| Times | strictly increasing |
| Contacts in a frame | unique ids, each `0` to `9` |
| Fingers across two neighbouring frames | at most 10 different ids |
| Size | at most 2 MiB as JSON |

### Coordinates

The **touch paths** block has two coordinate modes:

- **Normalized** (`normalized: true`): `0` to `1,000,000` on each axis of the
  target monitor, `0` being its left or top edge and `1,000,000` its right or
  bottom edge. A point outside that range is refused. Recordings are always
  normalized, so they replay on a screen of another resolution.
- **Screen**: desktop pixels, the same space as mouse blocks. Every point
  must fall inside the target monitor.

`monitor` is the monitor's index in Blockwork's monitor list (`0` is the
first). The monitor's geometry is checked again while a path plays; if it
moves or changes size, playback stops and releases every finger.

The **touch gesture** block plans a path from a few numbers: tap, long press,
swipe (1 to 10 fingers, `spread` pixels apart), pinch (two fingers, `amount`
is the scale from 0.01 to 20) and rotate (two fingers, `amount` in degrees,
within 3,600). In normalized mode its positions are percentages of the
monitor (0 to 100). Moving gestures get a frame every 16 ms.

## Playback

- One strand plays touch at a time. A second strand that tries while one is
  playing fails with "Another strand is using touch input" rather than
  mixing fingers.
- The whole path is checked before the first event.
- Stopping, pausing, a monitor change, a send failure or falling more than
  250 ms behind the schedule releases every finger at once.
- Touch can't run under the synchronous host clock used by some tests.

## Recording

With **Record touchscreen gestures** on (Recording settings), the Record
button also reads every direct-touch screen. Touchscreens are read without a
grab, so the desktop still gets every touch while you record. Each gesture,
from the first finger down to the last finger up, becomes one touch paths
block in normalized coordinates on monitor 0. Frames closer than 16 ms with
the same fingers are merged, and a gesture is cut at one minute or 5,000
frames. Blockwork's own virtual touchscreen is never read, so playback isn't
recorded back.

Recording reads the kernel's multitouch protocol B (`ABS_MT_SLOT`); a finger's
slot number is its id. Single-touch and protocol A screens aren't recorded.

While a gesture is held, other input (a key, a click) is recorded straight
after it without a wait: a touch block plays its whole path before the next
block starts. See [recording.md](recording.md).

## Platform support

| | Linux | Windows | macOS | Android (future) |
| --- | --- | --- | --- | --- |
| Touch gesture and touch paths blocks | Yes, experimental: a uinput touchscreen, one monitor only | Yes: the public touch injection API (`InitializeTouchInjection`/`InjectTouchInput`) | No | Planned, reading this format |
| Recording touchscreens | Yes, protocol B screens, read without a grab | No | No | Planned |
| Needs | write access to `/dev/uinput` | Windows 8 or newer; not yet qualified on a real desktop | | |
| Raw touchpad injection | No (pointer and scroll blocks still work) | No | No | |

"Experimental" means the software is written and tested against the kernel
interface, but not yet qualified on enough real touchscreens and desktops.
Settings > Integrations > Input diagnostics shows what this computer offers.

## For other implementations

An adapter that reads or writes touch paths should:

1. Refuse any `version` it doesn't know.
2. Apply every rule in the table above before acting on a path.
3. Treat each frame as complete: lift any finger the frame leaves out.
4. Release every finger when playback ends for any reason.
5. Keep ids stable for a finger's whole contact, and within `0` to `9`.
