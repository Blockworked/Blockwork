# When system starts

The **When system starts** header runs its strand once when you log in. It
needs **Start at system startup** on in Settings; with it off, the header is
kept but never fires.

## What runs

At login, Blockwork runs every **When system starts** strand in:

- the macro open in the editor, and
- every macro with **Always listen** on.

Only a strand whose top block is the header counts. The strands run in the
background; the editor window doesn't open.

## Once per login session

A startup event is recorded before anything is checked, so one login runs
the header at most once, even if Blockwork restarts, a second startup entry
fires, or two copies start at the same moment. An event that was skipped
(startup off, triggers paused, or nothing listening) is used up too: turning
the setting on later doesn't replay it. The next login is a new session.

A session is identified by:

| Platform | Session |
| --- | --- |
| Linux | the kernel's boot id plus `XDG_SESSION_ID` (or `DISPLAY`) |
| Windows | `whoami /logonid` |
| macOS | the boot time plus `launchctl managerasid` |

Only a hash of it is stored, under Blockwork's config folder in
`startup-claims/`.

## How Blockwork is launched

Turning on Start at system startup registers `blockwork-daemon --background
--system-startup`:

| Platform | Entry |
| --- | --- |
| Linux | `~/.config/autostart/blockwork.desktop`, or the Background portal in Flatpak |
| Windows | the `HKCU\...\Run` value `blockwork` |
| macOS | a launchd agent |

`--system-startup` is the only thing that fires the header; opening Blockwork
by hand never does. If a daemon is already running, the new one hands the
event to it and exits. Entries from older versions (without the flag) are
rewritten once.

## Testing

`blockwork-app/src/watch/startup.rs` tests which strands run, that a session
runs once (including eight concurrent notifications), and that skipped events
are consumed. Logging in on each desktop (KDE, GNOME, Windows, macOS) is a
manual check.
