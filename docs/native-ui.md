# Native UI automation

Blockwork can read and control other apps' buttons, fields and sliders
through the desktop's accessibility interface instead of the mouse. On Linux
that is AT-SPI, which GTK and Qt apps (and most others) provide. It is off
until **Native UI automation** is turned on in Settings > Integrations, and an
imported macro also needs the UI automation permission.

The code lives in `blockwork-model/src/input/ui.rs` (selectors),
`blockwork-core/src/accessibility/` (queries and actions) and
`blockwork-app/src/watch/native_ui.rs` (change triggers).

## Selectors

A selector describes an element; it is never a saved handle. Every block
looks the element up again when it runs, so a selector keeps working after
the app restarts.

Write it as `name=value` filters separated by semicolons:

```text
app=Calculator; name=Plus; role=push button
window=Preferences; name=Dark mode
name=Save
```

| Filter | Matches |
| --- | --- |
| `app` | the application's accessible name, ignoring case, anywhere in it |
| `window` | the title of the window or dialog the element is in, the same way |
| `name` | the element's accessible name, the same way |
| `role` | the role name exactly, ignoring case (`push button`, `text`, `slider`, `check box`, ...) |
| `id` | the toolkit's accessible id, exactly |
| `scope` | `Focused`, `App` or `Window` |

Without `scope`, a selector with `window` searches that window, one with
`app` searches that app, and one with neither searches the focused window.
App and Window scopes need their filter. A selector needs at least one of
`name`, `role` or `id`. A filter may appear once.

A selector can also be JSON with the same keys (`{"app": "Calculator",
"name": "Plus"}`), which allows semicolons in a value. Unknown keys are
refused.

The Focused scope means the window that has focus right now, as the app
itself reports it, so it works on any desktop.

## Picking an element

The scan button in a native UI block hides the editor and covers the screen
with the element picker. Point at a control in another app: it is outlined
and named (role, app and window underneath). Click it to write a selector
into the block, Escape or Cancel to stop, or **Search a list** to find it
by app and name instead.

The picker writes the shortest selector that finds the clicked element and
nothing else: its ID if it has one, else its role and name, adding the
window title only when the same control is in more than one window (titles
often change, an open document's name for example). When nothing can tell
it apart from a lookalike, such as two unnamed buttons, the list opens with
that app and name so you can choose another, and says how many match. A
window that describes none of its controls (many games and video surfaces)
is outlined as a whole and opens the list too.

To find which window is under the pointer the picker asks the window
manager: X11, Sway, Hyprland and KDE Plasma (through a KWin script). On
other Wayland desktops it falls back to the coordinates apps report
themselves, which some toolkits get wrong there; the list always works. The
overlay is see-through only where a compositor runs, so on a bare X11
window manager use the list.

## Blocks

- **Element exists / is enabled / text / value** reporters.
- **Press element** runs its primary action, **set text**, **set value** and
  **focus** do what they say.
- **When element's (exists / enabled / text / value) changes** fires when the
  property changes, checked when the app reports an accessibility event, at
  most four times a second. Changes in between are coalesced; only the
  latest value counts.

## Safety rules

- More than one match is an error, never a guess. If exactly one of the
  matches is on screen (the others in a closed menu or a hidden tab), that
  one is used. Otherwise narrow the selector.
- Disabled elements refuse actions.
- Password fields are never matched, listed or read.
- **Set value** must be a finite number within the element's own minimum and
  maximum.
- Text is at most 64 KiB; a selector at most 8 KiB.
- A search stops at 4,000 elements or 64 levels deep, and every call gives up
  after 3 seconds, so a huge or hung app can't stall a macro.

## Platforms

Linux only. Windows UI Automation and macOS accessibility adapters aren't
written yet; the blocks say so when run there.

## Testing

`scripts/test-native-ui.sh [gtk|qt|all]` starts a GTK and a Qt fixture app
(`blockwork-core/tests/fixtures/native-ui.c` and `.qml`), each in a private
display and session bus, and runs `blockwork-core/tests/native_ui_live.rs`
against it: queries, actions, events and every refusal above, then a stress
pass (100 write/read pairs, four threads querying at once, the event counter
under a burst). The GTK run also scans the fixture window with the screen picker's hit
test and checks the selectors it writes; with openbox installed the fixture
is a focused, managed window, so the Focused scope is checked too. It needs
Xvfb, `dbus-run-session`, at-spi2-core, GTK 3
headers and a Qt 6 built with its AT-SPI bridge (distribution Qt is; the
conda-forge build is not).
