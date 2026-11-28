# Window blocks on Linux

Wayland leaves window control to each desktop, so the window blocks
(Window action, Move window, Set window, the window reporters and Use
coordinates) talk to each one its own way. The code is in
`blockwork-core/src/window/linux.rs` and `window/linux/`. The In window
block's background mode only works on KDE: see
[background-input.md](background-input.md).

| Desktop | How | Can't do |
|---|---|---|
| X11 (any desktop) | X11 window properties | |
| Hyprland | `hyprctl` | minimize, rename |
| Sway | `swaymsg` | |
| KDE Plasma | `kdotool` and KWin scripts | rename |
| GNOME | the Blockwork windows Shell extension, over D-Bus | rename |
| COSMIC | its toplevel info and management Wayland protocols | move, resize, change monitor, keep on top, transparency, rename, process ID |

## GNOME

GNOME on Wayland gives other apps no way to see or move windows, so
Blockwork ships a small Shell extension in
`packaging/gnome-extension/blockwork-windows@blockworked.com`. Settings >
Integrations > GNOME window extension copies it to
`~/.local/share/gnome-shell/extensions/` and turns it on; GNOME starts it at
the next login. It owns `com.blockworked.Windows` on the session bus with two
methods: `List` (a JSON array of windows, front to back) and
`Act(id, action, a, b)`. Hide is a minimize on GNOME.

## COSMIC

The protocol files in `blockwork-core/protocols/` come from
[cosmic-protocols](https://github.com/pop-os/cosmic-protocols). COSMIC says
where each window is but not its stacking order, so the focused window counts
as frontmost and the rest follow in the order COSMIC lists them.

Neither GNOME nor COSMIC support has been tried on a real session yet.
