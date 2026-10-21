# Mock desktop

`scripts/mock-desktop.sh` runs Blockwork's live screen and window tests, or the
app itself, on a headless desktop with a stand-in Minecraft window
(`scripts/mock-desktop/fake-minecraft.qml`: sky, ground, a walking blocky
player, and a pure red marker at the crosshair). It is meant for cloud
containers and CI on Ubuntu 24.04, where there is no real session, no GPU
and no `/dev/uinput`; fake input devices stand in for the keyboard and mouse.

```bash
sudo scripts/mock-desktop.sh setup          # once: packages, Qt 6.10, kdotool, Plasma 6 root
export QMAKE=/opt/qt6/bin/qmake6
cargo build --release --workspace --exclude blockwork-linux-bridge

scripts/mock-desktop.sh run x11             # live screen tests (the default command)
scripts/mock-desktop.sh run sway ./target/release/blockwork
```

Any command can follow the desktop name; it runs with `DISPLAY` or
`WAYLAND_DISPLAY`, the session bus, `BW_LIVE_WINDOW=Minecraft` and the Qt
libraries set up. A screenshot of the desktop lands in
`target/mock-desktop-<desktop>.png` (or `$MOCK_SHOT`) on x11 and sway.

## Desktops

| Desktop | Capture path tested | Window info | Notes |
| --- | --- | --- | --- |
| `x11` | X11 | X11 (EWMH, via openbox) | Everything in `screen::` passes. |
| `sway` | `grim` | `swaymsg` | Everything passes. Windows tile, so the game fills half the screen. |
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
  is logged and injected into X11 with XTest, the way the kernel would hand
  a virtual device to the desktop. Events "typed" on the fake devices go to
  whoever opened them, and to X11 directly while nobody holds a grab, as
  with real hardware.

So Blockwork's evdev backend runs unchanged: it grabs both devices, its
hotkeys, triggers and recording see the fake typing, and macros type into
the X server. Drive it from the command with `mock-input`:

```bash
scripts/mock-desktop.sh run x11 sh -c '
  mock-input target                      # a window that logs what it receives
  ./target/debug/blockwork-daemon & mock-input wait-grab
  mock-input combo leftctrl leftalt m    # Blockwork default: run the selected macro
  mock-input type "brb "                 # text expansion
  mock-input log                         # what Blockwork sent through uinput
  cat "$MOCK_INPUT_LOG"'                 # what the target window received
```

`mock-input --help` lists the commands (`tap`, `combo`, `down`/`up`,
`type`, `click`, `move`, `wheel`, `status`, `wait-grab`, `wait-log`).
`mock-input type` waits 50 ms between keys unless `--delay=MS` says
otherwise. Absolute mouse moves need libei, which isn't faked.

## What doesn't work here

- Input outside X11. The fake devices below work on every desktop, but what
  Blockwork types is only injected into the desktop on `x11`; elsewhere it's
  logged (`mock-input log`) and nothing on screen reacts.
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
