# How Blockwork fits together

Blockwork is two processes on the desktop: the **daemon**, which owns every
macro, hotkey, watcher and run, and the **editor**, a Qt Quick window that
only draws what the daemon tells it. Closing the window leaves the daemon in
the tray, so hotkeys and schedules keep working.

The editor starts the daemon if none is running: the `blockwork-daemon`
binary beside it, or `blockwork --daemon` inside an AppImage, which has one
executable (`blockwork-qt/src/daemon_client.rs`).

```text
 blockwork (Qt)                       blockwork-daemon
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
file each (`watch/key.rs`, `watch/time.rs`, `watch/window.rs`, ...). Each
watcher is started from `Backend::start` in `blockwork-app/src/lib.rs`, looks
for its `InstructionKind` in every macro, and starts the strand when its
condition happens.

## The web build

`blockwork-web` is the same editor without the daemon, for the home page demo
on blockworked.com. The page runs Blockwork's own `EditorPage.qml`, built with
Qt for WebAssembly (`blockworked.com/qt-demo`). In place of the daemon,
blockworked.com's `qt-demo/DemoBackend.qml` sends each command to a `Core` object that links
`blockwork-web` as a static library and calls it through blockstitch's C ABI
(`blockstitch_ffi::export_abi!`). `Demo::dispatch` (`session.rs`) keeps one
in-memory session and answers with the new state. Nothing is saved and
nothing runs.

Most commands therefore exist twice: in `blockwork-app/src/commands/` and in
`blockwork-web/src/session.rs`. Keep both thin by putting shared rules on
`Macro`. `blockwork-app/src/web_parity.rs` replays one script through both and
fails if their documents differ, and another test fails when a web command is
missing from that script.

## Embedding

`blockwork-ffi` is a C ABI over the same core for running inside another
process (the Geometry Dash mod). It has no daemon or editor; see its
[README](../blockwork-ffi/README.md).

## Versions and pins

Each repo builds against a fixed commit of the one it depends on. Move a pin
after the upstream change lands on its main branch.

| Pin | Where | Who moves it, and how |
| --- | --- | --- |
| blockstitch commit | `Cargo.toml` (`[workspace.dependencies]`), `packaging/flatpak/` | this repo: `node scripts/set-blockstitch-rev.js <sha>`, then `cargo update -p blockstitch-core -p blockstitch-qml -p blockstitch-ffi` |
| Blockwork commit for the website's editor | blockworked.com `qt-demo/sources.env` (`BLOCKWORK_REV`) | the website; blockstitch and lucide-qt follow from that commit's `Cargo.lock` |
| Qt for WebAssembly and Emscripten | blockworked.com `qt-demo/sources.env` | the website; desktop CI uses `QT_VERSION` in `.github/workflows/` |
| Blockwork commit for the community site | Blockwork-community-blocks `.blockwork-rev` | the community site; `node scripts/sync-capabilities.js` copies `capabilities.json` |
| Browser bridge protocol | `PROTOCOL` in `blockwork-core/src/browser.rs`, `PROTOCOL.md` in blockwork-browser | bump both together when a message changes |
| Minimum Rust | `rust-version` in `Cargo.toml` (and blockstitch's) | CI builds the workspace with it |
| C header for the Geometry Dash mod | `blockwork-ffi/include/`, built into blockwork-gd | regenerated by `blockwork-ffi`'s build; CI fails on a stale copy |
