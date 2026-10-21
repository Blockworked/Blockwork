# Mock desktop

`scripts/mock-desktop.sh` runs Blockwork's live screen and window tests, or the
app itself, on a headless desktop (Wayland by default) with a stand-in Minecraft window
(`scripts/mock-desktop/fake-minecraft.qml`: sky, ground, a walking blocky
player, and a pure red marker at the crosshair). It is meant for cloud
containers and CI on Ubuntu 24.04, where there is no real session, no GPU
and no `/dev/uinput`; fake input devices stand in for the keyboard and mouse.

```bash
sudo scripts/mock-desktop.sh setup          # once: packages, Qt 6.10, kdotool, Plasma 6 root
export QMAKE=/opt/qt6/bin/qmake6
cargo build --release --workspace --exclude blockwork-linux-bridge

scripts/mock-desktop.sh run                 # live screen tests on sway (the defaults)
scripts/mock-desktop.sh run ./target/release/blockwork
scripts/mock-desktop.sh run x11 ./target/release/blockwork   # X11 instead
just mock-desktop                           # the same, with `just mock-desktop x11 ...` for X11
```

The desktop name is optional and defaults to `sway`. Any command can follow
it; it runs with `WAYLAND_DISPLAY` (or `DISPLAY` on x11), the session bus,
`QT_QPA_PLATFORM` (`wayland`, or `xcb` on x11), `BW_LIVE_WINDOW=Minecraft`
and the Qt libraries set up. A screenshot of the desktop lands in
`target/mock-desktop-<desktop>.png` (or `$MOCK_SHOT`) on sway and x11 when
the command ends. `MOCK_GAME=0` leaves out the stand-in game window, which
on sway gives the app the whole screen instead of half of it.

## Desktops

| Desktop | Capture path tested | Window info | Notes |
| --- | --- | --- | --- |
| `sway` (default) | `grim` | `swaymsg` | Everything passes. Windows tile, so the game fills half the screen. Input reaches apps through a virtual pointer and keyboard. |
| `x11` | X11 | X11 (EWMH, via openbox) | Everything in `screen::` passes. Input reaches apps through XTest. |
| `kwin5` | KWin ScreenShot2 | none | Plasma 5.27 from Ubuntu 24.04. kdotool and Blockwork's KWin scripts need Plasma 6, so window tests fail. |
| `kwin6` | none | kdotool + KWin scripts | Plasma 6.4 KWin in an Ubuntu 25.10 chroot. Without a GPU render node KWin 6 composites with QPainter, and ScreenShot2 needs OpenGL, so capture tests fail. |

So on KDE the capture path and the window path can each be exercised, but not
both in one session. A real Plasma 6 desktop is still needed for that.

## Input devices

Containers have no `/dev/uinput` or `/dev/input/event*`, so the script fakes
them (turn it off with `MOCK_INPUT=0`):

- `scripts/mock-desktop/fake-evdev.c` is built into an `LD_PRELOAD` shim
  that every command inherits. It turns opening `/dev/uinput` or
  `/dev/input/eventN`, and listing `/dev/input`, into Unix sockets served
  by the broker, and answers the evdev ioctls (capabilities, `EVIOCGKEY`,
  `EVIOCGRAB`) itself. Other paths go to the real calls.
- `scripts/mock-desktop/fake-input.py` is the broker. It serves a fake
  keyboard (`event0`) and mouse (`event1`). What a process writes to uinput
  is logged and injected into the desktop, the way the kernel would hand a
  virtual device to it: on sway as a `zwlr_virtual_pointer_v1` and a
  `zwp_virtual_keyboard_v1` (a small Python client for the Wayland wire
  protocol, with the default `us` keymap from libxkbcommon), on x11 with
  XTest. Events "typed" on the fake devices go to whoever opened them, and
  to the desktop directly while nobody holds a grab, as with real hardware.

So Blockwork's evdev backend runs unchanged: it grabs both devices, its
hotkeys, triggers and recording see the fake typing, and macros type into
the desktop. Drive it from the command with `mock-input`:

```bash
scripts/mock-desktop.sh run sh -c '
  mock-input target                      # a window that logs what it receives
  ./target/debug/blockwork-daemon & mock-input wait-grab
  mock-input combo leftctrl leftalt m    # Blockwork default: run the selected macro
  mock-input type "brb "                 # text expansion
  mock-input log                         # what Blockwork sent through uinput
  cat "$MOCK_INPUT_LOG"'                 # what the target window received
```

`mock-input --help` lists the commands (`tap`, `combo`, `down`/`up`,
`type`, `click`, `move`, `moveto`, `wheel`, `hwheel`, `scroll`, `status`,
`wait-grab`, `wait-log`). `mock-input type` waits 50 ms between keys unless
`--delay=MS` says otherwise. Absolute mouse moves through uinput need libei,
which isn't faked; `moveto` goes to the desktop directly instead.

## Driving the UI

`mock-input` also stands in for xdotool, and works the same on sway and
x11. Coordinates are screen pixels on the 1920x1080 desktop, from the top
left. `mock-input windows` prints each window's content rectangle and title
(`x,y widthxheight title`), so a point at `(px, py)` inside a window is at
`(x + px, y + py)` on screen. `mock-input shot [FILE]` saves a screenshot
(grim on sway, ImageMagick on x11).

| xdotool | mock-input |
| --- | --- |
| `xdotool mousemove X Y` | `mock-input moveto X Y` |
| `xdotool mousemove_relative DX DY` | `mock-input move DX DY` |
| `xdotool click 1` / `3` / `2` | `mock-input click` / `click right` / `click middle` |
| `xdotool mousedown 1` / `mouseup 1` | `mock-input down btn_left` / `up btn_left` |
| `xdotool click 4` / `5` (wheel up / down) | `mock-input wheel 1` / `wheel -1` |
| `xdotool type TEXT` | `mock-input type TEXT` |
| `xdotool key ctrl+a` | `mock-input combo leftctrl a` |
| `import -window root FILE` | `mock-input shot FILE` |
| `xdotool search` + `getwindowgeometry` | `mock-input windows` |

`wheel N` is a mouse wheel: on sway one frame per notch with axis source
`wheel` and a discrete step, which Qt reports with no scroll phase.
`scroll [--steps=N] DY [DX]` is a touchpad: small `finger` deltas over N
frames and then an axis stop, which Qt reports with scroll phases and a
pixel delta. Both count positive as up (or left). On x11, `scroll` falls
back to whole wheel notches (15 px each).

```bash
MOCK_GAME=0 scripts/mock-desktop.sh run sh -c '
  ./target/debug/blockwork & sleep 8
  mock-input windows                      # e.g. "2,25 1916x1053 Blockwork"
  mock-input moveto 400 300 && mock-input click
  mock-input type "hello" && mock-input combo leftctrl a
  mock-input wheel -3                     # three notches down
  mock-input scroll --steps=20 -200       # a touchpad flick down
  mock-input shot target/ui.png'
```

Drag with `down btn_left`, a few `moveto` steps, then `up btn_left`. Sway
gives a new window keyboard focus straight away; the app may take a few
seconds to draw its first frame on the software renderer.

The shim also shows `$FAKE_USB_DIR` as `/sys/bus/usb/devices`, so the USB
trigger, the "USB device plugged in?" reporter and the USB device picker
see fake devices: a root hub (skipped, like real hubs), a Logitech receiver
(`046d:c52b`) and a YubiKey (`1050:0407`). Plug in and unplug more while
Blockwork runs:

```bash
mock-input usb plug 0781:5581 "Ultra" SanDisk   # prints the new port, like 1-3
mock-input usb unplug 0781:5581
mock-input usb list
```

## What doesn't work here

- Input on KWin. The fake devices work on every desktop, but what Blockwork
  types is only injected into the desktop on `sway` and `x11`; on `kwin5`
  and `kwin6` it's logged (`mock-input log`) and nothing on screen reacts.
- Cursor position on Wayland: "this desktop doesn't share the cursor
  position" is expected.
- The desktop portal (GNOME's capture fallback) isn't set up.
- `download.qt.io` is blocked in some sandboxes, which breaks both `aqt` and
  CXX-Qt's `qt_minimal` download. `setup` installs Qt 6.10 from conda-forge
  into `/opt/qt6` instead (`QT_PREFIX` changes that).

## Container quirks the script handles

- `kwin_wayland` has file capabilities that can't be raised in a container,
  so it's copied to a plain binary. The copy has to keep the name
  `kwin_wayland`, or KWin refuses its own Qt platform plugin.
- kdotool and Blockwork pick KDE from `XDG_CURRENT_DESKTOP=KDE` and
  `KDE_SESSION_VERSION=6`, which the script sets.
- KWin loads scripts from files, so the Plasma 6 chroot gets `/tmp` (kdotool)
  and `$HOME` (`~/.cache/blockwork`) bind-mounted in.
- `KWIN_SCREENSHOT_NO_PERMISSION_CHECKS=1` lets the test binary call
  ScreenShot2 without an installed desktop file.
