# Working on Blockwork

## Layout

| Crate | What it is |
| --- | --- |
| `blockwork-core` | The engine: the macro document (`macros/`), the block runner (`macros/runner.rs`), input, screen, browser and system helpers, and the wire shapes the editor speaks (`frontend/`). |
| `blockwork-app` | Every command the editor can issue and the background watchers that fire trigger blocks (`watch/`). No UI toolkit. |
| `blockwork-daemon` | The long-lived process that owns `blockwork-app`'s state, hotkeys and playback. |
| `blockwork-protocol` | Messages between the daemon and the Qt UI. |
| `blockwork-qt` | The Qt Quick editor (`qml/`) and its Rust/C++ glue (`src/`). |
| `blockwork-web` | The same editor commands in WebAssembly, for the website's demo. |
| `blockwork-ffi` | A C ABI for recording and playback inside another process (the Geometry Dash mod). |
| `blockwork-linux-bridge` | A native Linux input helper that the FFI starts when it runs under Proton/Wine. `blockwork-ffi/build-windows.sh` builds it, and the Geometry Dash mod ships it as `linux-input.so`. |

[docs/architecture.md](docs/architecture.md) follows one command from the
editor to the daemon and back.

The block canvas, palette and document model come from
[blockstitch](https://github.com/Blockworked/blockstitch), pinned to one commit
in `Cargo.toml`. `just blockstitch-local` builds against a sibling checkout.

## Words

- **Macro**: one document. It holds **strands** (stacks of blocks on the canvas).
- **Instruction** or **block**: one block. Its kind is `InstructionKind`.
- **Header** or **trigger**: a `When...` block at the top of a strand. `WhenRan`
  runs when you press Run; the others are started by a watcher in `blockwork-app`.
- **Value** or **reporter**: what goes in a block's slot (`Value`, `Op`).
- **Field id**: the name the editor uses for one value slot (`FieldId`).
- **Pack**: a set of custom blocks from the community library.

## Build and test

You need Rust 1.91 or newer, [just](https://github.com/casey/just), Qt 6.10 or newer (CI tests 6.10.3; the website builds the editor with the Qt in `blockworked.com/qt-demo/sources.env`)
(`qmake6` on PATH or `QMAKE` set; without one the build downloads a minimal
Qt) and Python 3 for the QML tests. `just` leaves out
`blockwork-linux-bridge`, which only builds on Linux; do the same with
`--exclude blockwork-linux-bridge` when calling cargo yourself. The QML tests need a
blockstitch checkout at the pinned commit (`just test-qml` fetches one into
`../blockstitch` if it is missing) and one build first, which stages the
Lucide icons.

```bash
just                  # build
just run              # build and launch
just test             # every Rust test
just test-qml         # QML tests (needs Qt 6's qmltestrunner)
just translations     # refresh blockwork-qt/i18n after changing UI text
```

Three tests check generated files and rewrite them with `BLESS=1`:
`--test operator_descriptors` (the reporter table in `BlockCatalog.qml`),
`--test editor_baseline` (a saved macro in the current format) and
`--lib capabilities` (`blockwork-core/capabilities.json`, which the community
site copies).

Rust is formatted with `cargo fmt --all` (rustfmt's defaults, as in
blockstitch), and CI fails on unformatted code. Whole-tree formatting commits
are listed in `.git-blame-ignore-revs`. CI also runs clippy and fails on any
warning, so run `cargo clippy --workspace --all-targets` before pushing.

A new `.qml` file goes in `blockwork-qt/qml/module.json` too; the build
refuses a file that isn't listed there, or a listed one that is missing.

## Adding a block

The compiler catches most of these, since the runner, the capability table and
the `BlockKind` impl match every kind, and the tests in `fields.rs` and
`blockwork-core/tests/wire_blocks.rs` check the slot, variable and fixture lists against each
other. The rest are listed so nothing is missed.

1. `blockwork-core/src/macros/kind.rs`: add the `InstructionKind` variant with
   a doc comment. A field added to an existing block needs `#[serde(default)]`
   so older files still load.
2. `blockwork-core/src/macros/block_kind.rs`: its nested bodies in
   `body`/`body_mut`, any variable or list names it holds in `for_each_ref`,
   and add it to `is_header` if it is a trigger.
3. `blockwork-core/src/macros/fields.rs`: a `FieldId` per value slot, and
   its slots in `slots_mut` (field id, value, type). That one list is what the
   canvas visits and what a drag or a typed edit resolves through. Add the
   field to `requires_integer` if it only takes whole numbers.
4. `blockwork-core/src/macros/runner/`: what it does, in its category's file
   (`control`, `input`, `data`, `dialogs`, `browser`, `vision`, `system`;
   `apps.rs` holds helpers for the system blocks), and
   its variant in that category's arm of `run_block` in `runner.rs`. Besides
   routing, `run_block` only handles tracing, the debugger, stop flags and
   call depth; a test fails if the arm and the file disagree.
5. `blockwork-core/src/capabilities.rs`: what it can do (commands, network,
   screen, ...), or add it to the arm that does nothing guarded. This drives
   the import review and the permission prompts for shared macros. If it
   can do something, rerun `BLESS=1 cargo test -p blockwork-core capabilities`
   to refresh `blockwork-core/capabilities.json`, then copy that into the
   community site (`node scripts/sync-capabilities.js` there).
6. `blockwork-core/src/frontend/instructions.rs`: its line in `wire_blocks!`,
   and a case in `blockwork-core/tests/fixtures/wire_blocks.json`.
7. A trigger also needs a watcher in `blockwork-app` (see `watch/system.rs`)
   and a line in `macros/trigger.rs` for the run log.
8. `blockwork-qt/qml/BlockCatalog.qml`: a palette section entry, a row in
   its palette category's function (`eventRows`, `inputRows`, ...: label,
   value and dropdown pieces, icon, and `shape: "header"` for a trigger;
   `blockwork-core/tests/wire_blocks.rs` lists any block without one and
   checks the shape against `is_header`), a prefab if a
   fresh block needs defaults, and its variable dropdowns in
   `registerVariableFields`.
9. `blockwork-qt/qml/BlockHelp.qml`: one sentence on what it does.
10. `just translations`.

## Adding a reporter

1. `blockwork-core/src/input/value.rs`: an `ExtOperator` (name, arity,
   default args, evaluation). One that reads the machine calls
   `permissions::require` first; one with side effects goes in
   `SIDE_EFFECT_OPERATORS`.
2. `blockwork-core/src/capabilities.rs`: its capability in
   `operator_capability`, or its name in `UNGUARDED_OPERATORS`. A test fails
   until it is in one of them. A guarded one also needs the
   `capabilities.json` refresh described under adding a block.
3. `BLESS=1 cargo test -p blockwork-core --test operator_descriptors` to
   regenerate `hostDescriptors`.
4. `blockwork-qt/qml/BlockCatalog.qml`: a palette section entry and its
   label pieces and result type in `operatorRows()`; the palette's
   `extraOperators` list is built from that and the descriptors.
5. `blockwork-qt/qml/BlockHelp.qml` and `just translations`, as for a block.

`--test operator_descriptors` fails until steps 3 to 5 are done.

## Adding a command

1. A canvas edit (anything the website's editor should do too) goes in
   `blockwork-core/src/editing.rs`: a function over `D: Document`, its arm in
   `dispatch` and its name in `COMMANDS`. Both the app and the page then
   answer it. Give it a step in `blockwork-app/src/web_parity.rs`.
2. Anything else is app-only: the function in `blockwork-app/src/commands/`,
   in the file for its area (`library`, `settings`, ...). It takes
   `&SharedState` and `&AppHandle`, and calls `emit_state_updated` after
   changing state. Then its arm in `blockwork-app/src/dispatch.rs`.
3. Arms read arguments with `arg`. QML spells argument names in camelCase.
4. QML calls it with `invoke("name", {...})`. `blockwork-app/tests/qml_commands.rs` fails
   if QML names a command neither dispatcher has, or sends an argument key in
   a `{...}` literal that the command's arm never reads.

## Adding a setting

1. `blockwork-core/src/config/settings.rs`: an `Option<T>` field on
   `AppSettings`, so older settings files still load.
2. `blockwork-app/src/state.rs`: the field on `StateDto` and its line in
   `build_state_dto`. A setting only the UI reads comes straight from the
   settings file there (like `animation_speed`); one the backend checks
   often also lives on `AppState`, loaded in `lib.rs` at start (like
   `close_to_tray`).
3. A `set_...` command (see above) in `commands/settings.rs` that checks the
   value and calls `save_setting`, which saves it and publishes the state.
4. QML reads it from `appState` until the first state snapshot arrives, so
   give it a fallback where it is read (`appState.x ?? default`), as
   `animation_speed` does in `Main.qml`. Then its control in
   `SettingsPage.qml`, and `just translations`.

## Rules

- Keep comments short and plain. No em-dashes.
- A change to saved files needs a loader for the old shape (see
  `macros/saved_shapes.rs` and `macros/migrate.rs`).
- Canvas edits live once, in `blockwork-core/src/editing.rs`. A rule that
  isn't about one command goes on `Macro` (`macros/document_rules.rs`, or
  blockstitch if it isn't Blockwork-specific). A command only the website
  demo answers (`blockwork-web/src/session.rs`) gets a step in the parity
  script, or a line in its `NOT_REPLAYED` list saying why not; a test fails
  until it has one.
- Changelog entries go under the unreleased version in `CHANGELOG.md`.
