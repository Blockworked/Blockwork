# Working on Blockwork

## Layout

| Crate | What it is |
| --- | --- |
| `blockwork-core` | The engine: the macro document (`macros/`), the block runner (`macros/runner.rs`), input, screen, browser and system helpers, and the wire shapes the editor speaks (`frontend/`). |
| `blockwork-app` | Every command the editor can issue and the background watchers that fire trigger blocks (`*_watch.rs`). No UI toolkit. |
| `blockwork-daemon` | The long-lived process that owns `blockwork-app`'s state, hotkeys and playback. |
| `blockwork-protocol` | Messages between the daemon and the Qt UI. |
| `blockwork-qt` | The Qt Quick editor (`qml/`) and its Rust/C++ glue (`src/`). |
| `blockwork-web` | The same editor commands in WebAssembly, for the website's demo. |
| `blockwork-ffi` | A C ABI for recording and playback inside another process (the Geometry Dash mod). |
| `blockwork-linux-bridge` | A helper for Proton/Wine hosts. |

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

```bash
just                  # build
just run              # build and launch
just test             # every Rust test
python3 blockwork-qt/tests/run.py ../blockstitch   # QML tests (needs Qt 6's qmltestrunner)
just translations     # refresh blockwork-qt/i18n after changing UI text
```

Two tests check generated files and rewrite them with `BLESS=1`:
`--test operator_descriptors` (the reporter table in `BlockCatalog.qml`) and
`--test editor_baseline` (a saved macro in the current format).

## Adding a block

The compiler catches most of these, since the runner and the `BlockKind`
impl match every kind. The rest are listed so nothing is missed.

1. `blockwork-core/src/macros/mod.rs`: add the `InstructionKind` variant with
   a doc comment. A field added to an existing block needs `#[serde(default)]`
   so older files still load.
2. `blockwork-core/src/macros/block_kind.rs`: list its value slots in
   `visit_values_mut`, its nested bodies in `body`/`body_mut`, and add it to
   `is_header` if it is a trigger.
3. `blockwork-core/src/macros/fields.rs`: a `FieldId` per value slot, and
   its line in `value_slot_mut`. Add it to `requires_integer` if it only takes
   whole numbers.
4. `blockwork-core/src/macros/runner.rs`: what it does, in `run_block`.
5. `blockwork-core/src/capabilities.rs`: what it can do (commands, network,
   screen, ...), if anything. This drives the import review and the
   permission prompts for shared macros.
6. `blockwork-core/src/frontend/instructions.rs`: its line in `wire_blocks!`,
   and a case in `blockwork-core/tests/fixtures/wire_blocks.json`.
7. A trigger also needs a watcher in `blockwork-app` (see `system_watch.rs`)
   and a line in `macros/trigger.rs` for the run log.
8. `blockwork-qt/qml/BlockCatalog.qml`: a palette section entry, a row in
   `register()` (label, value and dropdown pieces, icon) and a prefab if a
   fresh block needs defaults.
9. `blockwork-qt/qml/BlockHelp.qml`: one sentence on what it does.
10. `just translations`.

A reporter is an `Op` in `blockwork-core/src/input/value.rs` plus a line in
`BlockCatalog.qml`'s `extraOperators`; rerun the `BLESS=1` test above.

## Rules

- Keep comments short and plain. No em-dashes.
- A change to saved files needs a loader for the old shape (see
  `macros/saved_shapes.rs` and `macros/migrate.rs`).
- Changelog entries go under the unreleased version in `CHANGELOG.md`.
