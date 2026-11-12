# Recording

The Record button turns what you do into blocks. Settings for it are in the
Recording settings dialog (the gear beside Record).

## Sources

| Source | Becomes | Setting |
| --- | --- | --- |
| Keys | key down/up blocks | always |
| Mouse buttons and scroll | button and scroll blocks | always |
| Mouse movement | move blocks, coalesced | Record mouse movement |
| Touchscreens (Linux) | one touch paths block per gesture | Record touchscreen gestures |
| A controller | button, stick and trigger blocks | Record controller input (needs controller input) |

Waits between blocks come from when each event happened, so the recording
replays at the speed it was made. See [touch.md](touch.md) and
[controllers.md](controllers.md) for what each of those records.

## Mixed input

All sources share one timeline. A wait is measured from the end of the
previous block's own time, so:

- A touch gesture plays its whole path before the next block, so input made
  while a finger is down is recorded straight after the touch block, with no
  wait.
- A stick or trigger moving every frame is kept to one block per 16 ms.
- A source that reports late (touchscreens read on their own thread) never
  makes a wait negative; the waits only grow.

When recording stops, anything still held is finished: an open gesture ends
with its fingers lifted, and a controller left pressed or off center gets a
reset block.

## Review

With **Review recordings before adding them** on (the default), stopping opens
a review instead of inserting straight away. It shows how many blocks of each
kind were recorded, how long the recording is, and the first 300 blocks.
Before inserting you can:

- leave out keys, mouse buttons, scroll, movement, touch or controller blocks
  (waits around a removed block are merged, so the timing stays the same),
- trim the wait before the first block,
- cap every wait at a number of seconds,
- add releases for keys and buttons still held at the end.

Insert adds the result to the macro you recorded in, as one undo step;
Discard throws it away. A review belongs to the macro it was recorded in:
switching macros keeps it until you come back. With review off, blocks are
inserted as they arrive.

## Input diagnostics

Settings > Integrations > Input diagnostics shows what this computer offers:
the display server and monitors, which capture and output backends work,
whether `/dev/uinput` is writable, touchscreens found for recording, touch
playback support, connected controllers, UI automation and startup state.
Copy puts the report on the clipboard for a bug report; it holds no keys,
text or macro contents.
