# How Blockwork fits together

Blockwork is two processes on the desktop: the **daemon**, which owns every
macro, hotkey, watcher and run, and the **editor**, a Qt Quick window that
only draws what the daemon tells it. Closing the window leaves the daemon in
the tray, so hotkeys and schedules keep working.

```text
 blockwork (Qt)                       blockwork --daemon
 ┌──────────────────────┐   socket    ┌───────────────────────────────┐
 │ QML  ── invoke() ──► │ ──────────► │ server.rs ─► Backend::dispatch│
 │ app_bridge.rs        │  NDJSON     │              commands/*.rs    │
 │ daemon_client.rs     │ ◄────────── │ AppHandle ◄─ emit_state       │
 │ appState ◄ stateJson │   State     │ watchers, runner threads      │
 └──────────────────────┘             └───────────────────────────────┘
```

## A command, start to finish

Take dragging a block onto the canvas.

1. **QML.** The canvas (from blockstitch) asks the host to add an
   instruction. `Main.qml`'s `invoke(command, args)` hands the name and a JSON
   object to `bridge.invokeCommand` (`blockwork-qt/src/app_bridge.rs`).
2. **Editor glue.** `app_bridge.rs` queues a `Request::Invoke` for
   `daemon_client.rs`, which writes a `ClientMessage` line to the daemon's
   socket (`blockwork-protocol`: one JSON message per line, a Unix socket or a
   named pipe on Windows).
3. **Daemon.** `blockwork-daemon/src/server.rs` reads the line and calls
   `Backend::dispatch(command, args)`.
4. **Dispatch.** `blockwork-app/src/dispatch.rs` reads the camelCase
   arguments and calls the matching function under `blockwork-app/src/commands/`
   (`editing.rs`, `strands.rs`, `blocks.rs`, ...). Edits that only touch the
   block graph go through blockstitch's `EditCommand`s; rules that are
   Blockwork's own live on `Macro` (`blockwork-core/src/macros/document_rules.rs`).
5. **Save and publish.** A command that changes a macro pushes an undo step,
   saves it (`blockwork-core/src/config/persistence.rs`), and calls
   `emit_state_updated` (`blockwork-app/src/state.rs`). That builds a
   `StateDto`, serializes it once, and sends `Event::State` on the
   `AppHandle`'s broadcast channel.
6. **Back to the editor.** The server forwards the state to the connected
   window. `daemon_client.rs` turns it into `Event::State`, `app_bridge.rs`
   sets `stateJson`, and `Main.qml` parses it into `appState`, which every
   page binds to.

A command's return value (an id, a fragment) comes back the same way as a
reply tagged with the request's key, for the callers that wait on one.

## Running a macro

`commands/running.rs` starts a run with `Macro::run` (`blockwork-core/src/macros/runner.rs`)
on its own thread (`macros_thread.rs`). The runner walks the strands, sends
input through the platform's `InputBackend` (`macros/backend/`), and marks
which blocks are running in `macros/trace.rs`. A `trace-watch` thread in
`Backend::start` sends that as `Event::Trace`, which the editor draws as the
run highlight.

Trigger blocks (`When...`) are started by watchers in `blockwork-app`, one
file each (`key_watch.rs`, `time_watch.rs`, `window_watch.rs`, ...). Each
watcher is started from `Backend::start` in `blockwork-app/src/lib.rs`, looks
for its `InstructionKind` in every macro, and starts the strand when its
condition happens.

## The web build

`blockwork-web` is the same editor without the daemon, for the website's
demo. It compiles to WebAssembly and keeps one in-memory session per tab
(`session.rs`, `Demo`). The page's canvas is blockstitch's web build, which
calls the session through blockstitch's C ABI (`blockstitch_ffi::export_abi!`),
so a command goes straight from the canvas to `Demo::dispatch` and the new
state comes back as the call's answer. Nothing is saved and nothing runs.

Most commands therefore exist twice: in `blockwork-app/src/commands/` and in
`blockwork-web/src/session.rs`. Keep both thin by putting shared rules on
`Macro`. `blockwork-app/src/web_parity.rs` replays one script through both and
fails if their documents differ, and another test fails when a web command is
missing from that script.

## Embedding

`blockwork-ffi` is a C ABI over the same core for running inside another
process (the Geometry Dash mod). It has no daemon or editor; see its
[README](../blockwork-ffi/README.md).
