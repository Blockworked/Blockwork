# Blockwork for agents: CLI and MCP server

Design for letting an agent (a human via a terminal, or an LLM through an
MCP client) do two things:

1. Drive Blockwork itself: create, edit, delete and run macros, change
   settings.
2. Use Blockwork as a desktop harness: take screenshots, find things on
   screen, read text, and synthesize input.

No new daemon, transport or execution engine. The client is one binary, the
commands share one set of per-id kernels in the backend, and every contract
surface is versioned or test-covered. Everything below is written so a
second implementation does not drift from the first.

## Locked decisions

- Structured errors in v1: `{code, message, hint?}` with codes `NOT_FOUND`,
  `CONFLICT`, `BAD_ARGS`, `BUSY`, `FORBIDDEN`, `DAEMON_UNAVAILABLE`,
  `UNSUPPORTED`.
- Tool granularity: ~35 fine tools in v1, one per command. No batching.
- Daemon gate: persisted `agent_enabled` in `AppSettings` (UI toggle + CLI),
  not a startup flag. `BLOCKWORK_AGENT=1` is an override.
- Input synthesis gate: separate persisted `agent_input_allowed`, default off.
  Channel on does not imply input on.
- Headless editing: document store now. `AppState` owns
  `HashMap<MacroId, Document>`; `current_macro` is the selected id.
- `window::list` ships for all backends in v1 (Hyprland, Sway, KDE, X11,
  Windows, macOS).
- Screenshots: temp-file handoff. Inline base64 only for small regions.
- Wire format: `dto_version` on `MacroDto` + golden fixtures in phase 1.

## Design rules

- Reuse, do not fork. Screen, input, windows, running and editing already exist
  as macro blocks and commands. The agent surface is one more caller of the
  same code, not a parallel engine.
- Kernels before clients. Backend library/settings logic is extracted into
  per-id functions (`rename_macro(id, ...)`, `set_macro_settings(id, ...)`)
  that the UI and the agent both call. The agent adds explicit addressing,
  never new semantics.
- Contracts are frozen or tested. The wire DTO and the allowlist are covered
  by a test in phase 1, not by hope.
- One binary. `blockwork-agent` is the CLI and, with the `mcp` subcommand,
  the MCP server. No second binary and no alias to keep a packager honest.

## What exists today

| Piece | Where | Notes |
| --- | --- | --- |
| Daemon socket | `blockwork-protocol` | NDJSON over a per-user Unix socket (named pipe on Windows). Daemon owns the socket file, so it doubles as the single-instance lock. Debug builds use `daemon-dev`. |
| Client kinds | `blockwork-protocol/src/lib.rs` | `Hello` (the editor: at most one, gets `State` pushes, daemon exits when it disconnects) and `Script` (only `list_macros`, `run_macro_named`, `stop_macro_named`, no pushes, never counts as a window). The legacy `blockwork list/run/stop` CLI lives in `blockwork-qt/src/cli.rs` and speaks `Script`. |
| Command surface | `blockwork-app/src/dispatch.rs` | ~150 commands: library, per-macro settings, global settings, hotkeys, import/export, recording, debugging, plus canvas commands delegated to `blockwork-model/src/editing.rs`. The match is already ~670 lines. |
| Canvas editing | `blockwork-model/src/editing.rs` | Strand/block/value/comment/variable/signal/list/custom-block ops with undo through the `Document` trait. Addressing is `strandId` + `InstrPath` (QML drag paths), not block ids. |
| App state | `blockwork-app/src/state.rs` | `AppState` holds `current_macro`, `macros_list`, `macro_strs` and one `History`, kept in sync by hand (`sync_variable_values`, `sync_list_values`, `set_selected_macro_id`). |
| Macro shape | `blockwork-model/src/macros/` | `Macro` = `BlockGraph` (strands, floating values, comments, variables, lists, custom blocks) + name/description + `MacroSettings`. Wire form is `MacroDto`/`InstructionDto` (`frontend.rs`) with the wire tables in `frontend/instructions.rs`. ~90 `InstructionKind`s, catalogued in `macros/info.rs`. |
| Execution | `blockwork-core/src/macros/runner.rs`, `backend/mod.rs` | `Run` walks strands on their own threads; `InputBackend` synthesizes key/button/move/scroll/text. `AppState.emulator` is shared; `run_token` locks it per token and tracks `pressed_keys`/`pressed_buttons` per strand. `trace.rs` reports active blocks; `run_log.rs` keeps the log. |
| Screen | `blockwork-core/src/screen/` | `capture` reuses a 60ms cache; `capture_fresh` bypasses it. `Shot` carries `origin` + `scale`. `image.rs` `to_png`/`from_png` exist. |
| Windows | `blockwork-core/src/window.rs` | `active`, `bounds`, `act`, `place`, `exists`, matched by title/app substring. Linux has four backends inside `window/linux.rs` (Hyprland, Sway, KDE, X11). No list-windows call yet. |
| Permissions | `blockwork-core/src/permissions.rs` | `GUARDED` kinds; `check()` returns OK for an empty `macro_id`. Agent input must not rely on this bypass. |
| Protocol size | `blockwork-protocol/src/lib.rs` | `MAX_MESSAGE_BYTES` is 8MB: fine for editor state, too small for a 4K PNG plus base64 inflation. |

## Gaps

1. **No agent client class.** A script client can only list and run macros.
   Widening `Script` would grant every local script input synthesis.
2. **Backend logic assumes the open macro.** `set_title`, `set_macro_*` and
   `AppDoc` all borrow `current_macro`; renaming or setting a macro by id is
   not possible without touching the editor selection.
3. **No document store by id.** `current_macro` is a copy; editing a non-open
   macro by clone-writeback drifts from the editor and from disk.
4. **No direct screen/input commands.** Screen reading and input synthesis
   exist only as macro blocks plus `screen_grab`/`window_bounds` for the
   region picker.
5. **No versioned wire contract.** `MacroDto` has no `dto_version` and no
   stability promise, so `wire_blocks!` renames would silently break agents.
6. **Existing CLI is Qt-bound and tiny.** `blockwork-qt/src/cli.rs` is three
   hand-rolled subcommands; it cannot run headless without Qt.

## Layout

One new workspace crate, `blockwork-agent`, with a shared client lib and one
binary that serves both surfaces:

```text
blockwork-agent
  src/lib.rs       DaemonClient: connect/spawn, Agent handshake, version check
  src/tools.rs     Tool table (name, description, input_schema_json, handler);
                   the single source of truth for CLI, MCP and daemon names
  src/cli.rs       clap subcommands, generated from tools.rs
  src/mcp.rs       stdio MCP server (rmcp), tools from tools.rs
```

The binary defaults to the CLI; `blockwork-agent mcp` runs the MCP server.
Arg defaults and tool names are defined once in `tools.rs`, so the parity
test (`CLI == MCP == dispatch ids`) only passes when all three agree.

## Protocol

`blockwork-protocol/src/lib.rs`:

- `Agent { client: String, protocol: u32 }` handshake, sibling to `Hello` and
  `Script`. The client sends
  `{"type":"agent","client":"blockwork-agent/1.0","protocol":1}`. The daemon
  answers `Hello(accepted)` with `data` carrying `{daemon_version,
  protocol_min, protocol_max}`. An unsupported version returns
  `accepted:false` plus a hint, rather than a mid-session parse failure.
- The daemon has `serve_agent`, a clone of `serve_script` with its own
  allowlist. Agent connections do not take the UI slot, do not receive `State`
  pushes, and the daemon survives their disconnect.
- The allowlist is derived from the agent handler table, or tested equal to
  it. Two hand-kept lists are not acceptable at ~35 commands.

## Backend

### Per-id kernels (do first)

In `commands/library.rs` and `commands/settings.rs`, parameterize the helpers
that currently assume `current_macro` by macro id:

- `rename_macro(state, app, id, name)`
- `set_macro_settings(state, app, id, settings)` replacing the seven
  `set_macro_*` spread with one struct write
- `set_macro_folder_by_id`, `duplicate_macro_by_id`, `delete_macro_by_id`
- `get_macro_by_id`, `list_macros_detailed` from `library_entry`

UI dispatch becomes: resolve the open id, call the kernel,
`emit_state_updated`. Agent dispatch becomes: call the kernel with the
explicit id. Agent adds no new semantics, only new addressing.

### Agent dispatch module

`blockwork-app/src/commands/agent/` owns the agent surface:

- `mod.rs` - the `Tool` handler functions, one per tool
- `dispatch.rs` - `dispatch_agent(cmd, args)` called from one branch in
  `Backend::dispatch`
- `editing.rs` - the canvas helpers described below
- `harness.rs` - screen, input, window, clipboard, notify

Arg structs are `#[derive(Deserialize)]` per tool. A test asserts
`agent allowlist == handled set == tools.rs names`.

### Document store and editing

`AppState` owns `documents: HashMap<MacroId, Document>` where
`Document { macro: Macro, history: History<UndoSnapshot>, buffers:
ValueBuffers, revision: u64 }`. `current_macro` becomes the selected id;
`macros_list` is a view over the map (name/folder/has_triggers computed on
demand); `macro_strs` is derived the same way. `AppDoc` is the `Document`
impl over one entry; `editing::Parts` drops the `open: Option<&mut Macro>`
ambiguity in favor of a borrowed entry or a clone-in/commit-out helper.

Every agent edit carries `base_revision: u64`. Mismatch is a `CONFLICT`
error (`macro changed since revision N`), never silent overwrite. Editor
edits bump the same counter, so UI and agent cannot clobber each other.

Block addressing: `find_path_by_id` walks `MacroGraph` and returns
`(strandId, InstrPath)`, handling `Vec<Instruction>` bodies for
If/Repeat/While/TryCatch. `agent_add_block`, `agent_edit_block`,
`agent_remove_block` resolve ids internally. `agent_move_block` is copy +
remove + insert on the graph, not the QML `merge_tail` drag path.

Value args use the existing `Value` JSON (`{"text": ...}`, `{"op": ...}`),
parsed from one string helper shared by CLI and MCP. The palette catalog is
exposed as `agent_describe_blocks` from `InstructionKind` + `fields.rs` +
the wire tables, so clients never guess field names.

### DTO stability

- Add `dto_version: u32` (starts at 1) to `MacroDto`; unknown future versions
  are rejected with a hint, never silently accepted.
- Serialize `dto_version` in every agent reply that returns a macro.
- Golden JSON fixtures for `InstructionDto` round-trips live next to
  `wire_blocks` tests. Field renames are breaking changes by policy.

## Commands

Error shape for all of them: `{code, message, hint?}` with codes
`NOT_FOUND`, `CONFLICT`, `BAD_ARGS`, `BUSY`, `FORBIDDEN`,
`DAEMON_UNAVAILABLE`, `UNSUPPORTED`. The legacy `data` string stays for the
UI; the API-level `error` field becomes an object on agent replies.

| Group | Commands (wire names; MCP tool names match without `agent_` prefix) |
| --- | --- |
| Library | `agent_list_macros`, `agent_get_macro`, `agent_create_macro`, `agent_create_from_template`, `agent_duplicate_macro`, `agent_delete_macro`, `agent_rename_macro`, `agent_get_macro_settings`, `agent_update_macro_settings`, `agent_get_global_settings`, `agent_set_global_settings`, `agent_get_hotkeys`, `agent_set_hotkeys`, `agent_list_templates`, `agent_describe_blocks`, `agent_export_macro`, `agent_import_macro`, `agent_confirm_import` |
| Canvas | `agent_add_strand`, `agent_remove_strand`, `agent_add_block`, `agent_edit_block`, `agent_remove_block`, `agent_move_block`, `agent_find_blocks`, `agent_add_comment`, `agent_remove_comment`, `agent_create_variable`, `agent_rename_variable`, `agent_delete_variable`, `agent_set_variable_value`, `agent_set_list_items`, `agent_create_list`, `agent_rename_list`, `agent_delete_list`, `agent_replace_canvas`, `agent_undo`, `agent_redo` |
| Run | `agent_run_macro`, `agent_stop_macro`, `agent_stop_all`, `agent_run_status`, `agent_run_log`, `agent_trace`, `agent_emergency_stop`, `agent_resume_triggers` |
| See | `agent_screenshot`, `agent_ocr`, `agent_find_image`, `agent_pixel`, `agent_cursor`, `agent_windows`, `agent_active_window`, `agent_window_bounds` |
| Act | `agent_move_mouse`, `agent_move_mouse_rel`, `agent_click`, `agent_scroll`, `agent_key`, `agent_type_text`, `agent_button`, `agent_window_action`, `agent_window_move`, `agent_clipboard_get`, `agent_clipboard_set`, `agent_notify`, `agent_list_apps`, `agent_launch_app` |

Library notes: `agent_run_macro` takes `id` first and accepts `name` as
resolution only (same `find_id` rule: exact, case-insensitive, then id;
duplicate names are an error). `agent_create_macro` reuses
`creation::from_args` and forces `permissions: Some(vec![])` (restricted)
unless the caller passes explicit grants. `agent_export_macro` reuses the
existing import-review flow rather than inventing a second one.

## Desktop harness

All harness input goes through `AppState.emulator` so the daemon stays the
single writer. The runner locks per token, so an agent action can interleave
mid-macro. That is prevented by an arbiter:

- `agent_move_mouse`, `agent_move_mouse_rel`, `agent_click`, `agent_scroll`, `agent_key`,
  `agent_type_text`, `agent_button` first check `run_registry`: while any
  input-driving macro is running they return `BUSY`, or require
  `force: true`. A `force` action logs one `run_log` line with source `agent`
  saying what it preempted.
- `agent_key` emits a held chord (press in order, release in reverse), matching
  the macro runner; `Control+N` holds Control. A sequential tap would send
  plain letters. `hold_ms` keeps the chord down for that many ms before
  releasing; `action: press` / `action: release` hold a key across calls.
- `agent_button` `action: press` holds the button down until a later
  `action: release` call; `action: click` is an immediate press+release.
- `agent_move_mouse_rel` (`dx`, `dy`) needs no portal grant and is the only
  way to look around in pointer-lock games (e.g. Hytale), where absolute
  reads report `ok` but the cursor cannot move.
- `agent_launch_app` opens an installed app by name (or direct command) with
  optional `args` (e.g. a file path), detached via the host spawn shim, and
  returns receipts. Prefer it over Meta+type+Enter launcher navigation.

Seeing:

- `agent_screenshot` uses `capture_fresh()` (never the shared 60ms cache),
  PNG-encodes via `Shot::to_png()`, writes to a temp file, and returns
  `{path, width, height, origin, scale, downscale, monitors}`. MCP returns
  only this JSON (path + metadata); the agent reads the PNG file with its
  native image reading to see it.
- `agent_ocr` wraps `read_text` over a region. `agent_find_image` wraps
  `find_image_in`/`find_image_all` over an optional region and returns
  screen-space centers (with `diff`; `all:true` lists every match). A template
  cropped from the same frame matches its own origin - that self-match is not
  confirmation the right widget was captured; Read the crop first and use
  region/`all` to disambiguate similar buttons.
- `agent_windows` returns `WindowInfo {title, app, rect, focused}` from a
  per-platform `list()`: Hyprland (`hyprctl clients -j`), Sway (`swaymsg -t
  get_tree`), KDE (qdotool), X11 (`_NET_CLIENT_LIST`), Windows
  (`EnumWindows`), macOS (`NSWorkspace`). All six land in v1.

## Safety

- Agent channel: persisted `agent_enabled` in `AppSettings`, toggled in the
  editor Settings page or via CLI. `BLOCKWORK_AGENT=1` overrides per process.
  A tray daemon picks the setting up on the next handshake.
- Input synthesis: separate persisted `agent_input_allowed`, default off,
  toggled in the same place. `agent_enabled` alone does not enable clicks or
  typing. Every harness action is run-logged with source `agent`.
- Agent-created macros start restricted (`permissions: Some(vec![])`),
  reusing `GUARDED`. This is unrelated to the input gate above and both apply.
- `agent_emergency_stop` + `agent_resume_triggers` are exposed for any
  session. Duplicate macro names returned by `agent_list_macros` are surfaced
  so automation does not silently resolve to the wrong target.
- No new network surface. The daemon socket is per-user; the loopback IPC
  server (`blockwork-core/src/ipc`) is unchanged.

## CLI

Bin `blockwork-agent`, subcommands generated from `tools.rs`:

```text
blockwork-agent list | show <id|name> [--json]
blockwork-agent create <name> [--description D] [--from-template T] [--json]
blockwork-agent delete <id|name>|copy <id|name>
blockwork-agent settings <id|name> [--set k=v ...] | global-settings [--set k=v ...]
blockwork-agent add-strand <id> [--header Type] [--at x,y]
blockwork-agent add-block <id> <strand|last> <Type> [--set field=value ...] [--file dto.json]
blockwork-agent edit-block <id> <block-id> [--set field=value ...]
blockwork-agent rm-block <id> <block-id> | find <id> [--type T] [--text S]
blockwork-agent set-var <id> <name> <value> | set-list <id> <name> <json>
blockwork-agent undo|redo <id> [--base-revision N]
blockwork-agent run <id|name> [--wait] [--timeout s] | stop <id|name>|--all
blockwork-agent status|log [--follow] [--last n]
blockwork-agent shot [--region x,y,w,h] [--window TITLE] [-o out.png]
blockwork-agent ocr [--region x,y,w,h] | find-image <png|path>
blockwork-agent click <x,y> | move <x,y> | scroll <n> | key <combo> | type <text>
blockwork-agent windows|active-window|focus-window <title>
blockwork-agent clipboard get|set <text> | notify <title> <msg> | apps
blockwork-agent agent input on|off | agent channel on|off
blockwork-agent mcp    (execs the MCP server mode)
blockwork-agent version
```

Global flags: `--daemon PATH`, `--json`, `--socket NAME`, `--timeout SECS`.
`--json` prints raw daemon JSON for every command. Legacy
`blockwork list/run/stop` becomes a deprecated shim that execs this binary.

## MCP server

`blockwork-agent mcp` uses `rmcp` (official Rust SDK) over stdio; no
hand-rolled JSON-RPC loop. Tools come from `tools.rs`; schemas are derived
with `schemars` from the same arg structs the CLI parses.

- Tools: the command table above, without `agent_` prefix outside the wire
  (`screenshot`, `run_macro`, `edit_block`, ...). The wire mapping lives in
  one place in `tools.rs`.
- Resources: `blockwork://macros`, `blockwork://macro/{id}` (`MacroDto` with
  `dto_version`), `blockwork://state`, `blockwork://runlog`.
- Prompts: `build_macro` (describe, build, run, read log, iterate),
  `locate_and_act` (find, click, screenshot, verify).

`agent_wait_run` is client-side polling (`agent_run_status` + `agent_trace`
+ `agent_run_log` with `since_revision`); there is no server-side wait verb
beyond the CLI's `--wait` loop.

## Coordinate spaces

Screen tools speak screen coordinates: the space `MoveMouse` absolute mode
and `Shot::origin/scale` use (logical pixels on scaled displays; physical on
Windows where the daemon is DPI aware). Screenshots return `origin` +
`scale` alongside the path so the agent can map model pixels to screen points.
The server downscales by default to a model-friendly size and reports the
factor; `full_res: true` returns native. On Wayland the cursor position is
tracked, not queried, so `agent_cursor` may lag fast moves - call
`agent_move_mouse` first to anchor it.

## Packaging

One binary beside the daemon: `target/release/blockwork-agent`.

- `justfile`: `AGENT` var beside `TARGET`/`DAEMON`; `just install` installs it
  to `LIBDIR` and symlinks `/usr/bin/blockwork-agent`.
- Flatpak: the manifest builds the workspace, so the binary appears with it;
  `flatpak-sources` refreshes `cargo-sources.json` after adding `clap`,
  `rmcp` and `schemars`.
- Inno Setup, MSIX (`scripts/build-msix.ps1`) and the macOS bundle script
  copy it beside `blockwork-daemon`, resolved via `current_exe` as
  `daemon_exe()` already does. No `BLOCKWORK_UI_EXE`-style env.

## Phases

1. **Kernels + protocol + contract tests** (~3-4 days): per-id extraction in
   `commands/`, document store refactor (phase 1 owns it mid-flight:
   `documents` map + selected id + `base_revision` bumps), `Agent` handshake
   with version negotiation, `connect_or_spawn` into `blockwork-protocol`,
   `agent_enabled` + `agent_input_allowed` in `AppSettings`, allowlist ==
   handlers test, `dto_version` + golden fixtures, CLI/MCP/tool parity test
   skeleton. Exit: `blockwork-agent list/run/stop` works headless without Qt.
2. **Library + run** (~2 days): `agent_*` library, settings, hotkeys,
   import/export, and run/status/log/trace with structured errors. Exit:
   headless macro lifecycle without the editor.
3. **Headless canvas** (~4-5 days): document-store editing path +
   `find_path_by_id`, canvas command set, `agent_describe_blocks`,
   `agent_replace_canvas` with validation. Exit: an agent builds a macro from a
   description and runs it, editor closed.
4. **Desktop harness** (~4 days): screenshot file handoff, OCR, find, pixel,
   `window::list` for all six backends, input arbiter + input gate,
   clipboard/notify. Exit: locate-and-click loop works on the mock desktop
   (`just mock-desktop`).
5. **MCP + packaging** (~2 days): `rmcp` stdio server, resources, prompts,
   packagers, user guide, CI binary-exists check. Exit: release artifacts ship
   `blockwork-agent` next to the daemon.

Total: ~3 weeks for a first cut, single-threaded.

## Implementation status

Phases 1-5 are implemented (no `rmcp`/`clap` dependency: the MCP server is a
minimal stdio JSON-RPC implementation and the CLI is hand-rolled from the
same tool table - drop-in replacements for both are still worthwhile).

- `blockwork-protocol`: `ClientMessage::Agent` handshake, `AGENT_COMMANDS`,
  `AGENT_PROTOCOL` version negotiation, `AgentError` structured errors.
- `blockwork-core`: `AppSettings.agent_enabled`/`agent_input_allowed`,
  `window::WindowInfo` + `list()` for all six backends, `Rect` derives,
  `BLOCK_KINDS` from the `wire_blocks!` tables.
- `blockwork-app`: per-id kernels (`rename_macro_by_id`,
  `set_macro_settings_by_id`, `save_macro_by_id`), document store
  (`agent_histories` + `doc_revisions` with `base_revision` conflicts),
  `commands/agent.rs` dispatch (all 66 commands, input gate + `BUSY` arbiter,
  screenshot file handoff downscaled by default with `downscale` factor,
  desktop calls on `spawn_blocking`).
- `blockwork-agent`: the single binary (`mcp` subcommand, tools parity
  tests), legacy `blockwork list/run/stop` as a shim, `just install` wiring.
- Flatpak builds the workspace, so the binary ships with it; rerun
  `just flatpak-sources` after the lockfile change.

Known limitations: nested-body blocks are edited via `replace_canvas` only
(top-level `find_blocks`/`remove_block`/`move_block` walk nested bodies, but
`edit_block` fields surgery needs an `InstructionDto` round-trip); `trace`
returns `[]` (trace streaming is deferred); Wayland absolute input can block
on the libei portal (same as macro `MoveMouse` blocks today) - use
`agent_move_mouse_rel` instead, which needs no grant. `find_image` without a
region searches the whole desktop, so prefer a `region` (or at least a tight
one-shot crop) on large multi-monitor desktops.

Template matching for icons is covered: `screenshot`/`crop` write PNGs to
temp files, `find_image` locates the crop, then click the returned center.
Clicks report `ok` even when they miss, so always verify with `cursor` or a
fresh screenshot. Input verbs accept `expect` (an app/title substring the
focused window must match, checked in the daemon right before the input
fires, returning `FORBIDDEN` on mismatch) and the results include
`after:{active, cursor}` receipts. Use `window_action Focus` before input when focus may be
elsewhere, hover (move) before clicking so focus/hover settles, and prefer
standard shortcuts (`Control+s/n`, `Alt+f` menus) over mouse work when known.
After any launch/search/Enter action, check `active_window` and only proceed
when the expected app is focused - pinned taskbar icons are not
self-describing, so disambiguate by checking `windows` app/title first.
After any no-op or wrong-window surprise, stop and re-read `active_window` +
screenshot to re-establish state instead of retrying the same coordinates.

## Alternatives considered

- **Widen `Script`**: no. Script is deliberately tiny and used by launchers;
  widening it grants every script input synthesis.
- **Drive the editor UI (QML `invokeCommand`)**: no. It requires the window
  open, fights the single-editor slot, and cannot run headless.
- **Keep the legacy `blockwork list/run/stop` CLI**: no. Two binaries with the
  same subcommand names are a support cost. It becomes a shim.
- **Python sidecar / FastMCP**: no. Blockwork is Rust; an in-language MCP
  binary keeps one dep tree and one sandbox story.
- **Bypass the daemon (embed `blockwork-core`)**: no. That double-runs the
  input grab, hotkeys and watchers and splits macro state.
- **Hand-rolled MCP stdio**: no. The spec moves; `rmcp` amortizes that.
