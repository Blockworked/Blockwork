# Background input

The **In window** block (Windows section) runs its blocks aimed at one
window. In the background, keys, clicks, typing and scrolling go only to that
window, positions count from its top left, and color, text and find blocks
look at that window even while it's covered. Your own keyboard, mouse, focus
and clipboard stay where they are.

**Brought to front** focuses the window first and then uses the normal
desktop input, so it works everywhere the window blocks do. **Else bring it
to front** tries the background first and falls back when it can't.

## Where it works

Only KDE Plasma 6 on Wayland, with the Blockwork KWin plugin installed.
Settings > Integrations > Background input says whether it's ready.

Other routes (uinput, libei, the remote desktop portal, KWin's fake input)
all type into whichever window has focus, so they can't do this. The plugin
gives KWin a few extra input seats instead: each lane is a second keyboard
and pointer that only its window sees.

| App | Works |
|---|---|
| Qt, GTK 3 and 4, SDL3, Godot, winit apps | yes |
| Chromium, Electron, GLFW, SDL2 | no: they only use the first seat |
| XWayland apps | no: X11 has one seat |
| Firefox | only while it has one window open |

GTK apps only see seats that existed when they started, so an app started
before the plugin loaded needs a restart.

## Installing the plugin

KWin only loads plugins built for its exact version, so the plugin comes as a
distro package built against the distro's own KWin (`blockwork-kwin`), and
needs a rebuild for each Plasma release. To build it yourself:

```bash
cmake -S blockwork-kwin -B build-kwin -DCMAKE_INSTALL_PREFIX=/usr
cmake --build build-kwin
sudo cmake --install build-kwin
```

Then log out and back in. It needs KWin's development headers, Qt 6,
wayland-server, wayland-scanner and libxkbcommon. A Flatpak Blockwork uses
the host's plugin; it can't add one itself.

## How it fits together

- `blockwork-core/protocols/blockwork-seat-host-v1.xml`: the private Wayland
  protocol between Blockwork and the plugin. Only unsandboxed clients, or a
  sandbox with the app id `com.blockworked.Blockwork`, may bind it.
- `blockwork-kwin/`: the plugin. Four seats made at KWin startup; a lane is
  lent to one connection and returned (with every key and button released)
  when it closes. It also keeps the window rendering while it's covered or
  minimized, and tells Blockwork when you touch the same window, so the macro
  waits a moment for you.
- `blockwork-core/src/macros/lane/`: the `Lane` trait and `LaneBackend`, which
  stands in for the strand's input backend inside the block, so every input
  block works unchanged. Typing sends a keymap made for the text, so your
  layout doesn't matter.
- `InWindow` in `macros/runner/control.rs` swaps the backend, the strand's
  origin and the screen source (`screen::set_capture_source`, KWin's
  `CaptureWindow`) for its body and puts them back after.

## Known gaps

- Grabbing popups (menus) of the target app can take the user's keyboard,
  since KWin routes popups through its one main seat.
- Signals sent inside the block run on the desktop, not in the window.
- Clipboard blocks still use the desktop clipboard.
- On KWin 6.0 to 6.7, window capture needs Blockwork's desktop file to list
  `org.kde.KWin.ScreenShot2` in `X-KDE-DBUS-Restricted-Interfaces`.

The seat-pool approach follows [Seatgeist](https://github.com/seatgeist/seatgeist)'s
agent seat (MIT or Apache-2.0); the code here is Blockwork's own.
