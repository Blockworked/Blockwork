# Custom block menus (0.7)

In **Make a Block**, add inputs as usual. Open **Inputs** to give a text input
**Dropdown choices**, one per line. A line can be just a value, or `Label | value`
when the displayed label should differ from the value passed to the block body.
Choose the starting value there too. Reporter expressions still fit these inputs.

Add a **Button** piece to open your own settings screen. In **Inputs**, give that
button a screen title and select the inputs it edits. The screen offers text,
number, boolean, multiline and dropdown controls, including editable inputs inside
reporters. Enable **Show only in a settings screen** to move an input off the
block's inline row. Every hidden input must belong to a button's screen.

**Apply** commits the changed arguments in one undoable edit. **Cancel** discards
the draft. Other arguments, expressions, branch bodies and saved reporter values
are preserved. If a touched argument or the definition changes while the screen
is open, reopen the screen before applying. Menus travel with saved macros and
community packs. Format 6 files require a version of Blockwork that supports them;
older macro files still load.

## Shared framework

The implementation lives in blockstitch. Custom and built-in blocks share its
`BlockRegistry` row pieces, `BlockUi.choices`, `BlockUi.open` screen routing and
`BlockSettingsDialog` forms. Blockwork supplies native screen adapters for model,
image, app, launch, USB, time, schedule, signal and global pickers. Machine-specific
pickers stay in the host. Imported custom definitions contain data, never QML,
JavaScript or host command names.

A definition can contain the following pieces, alongside its labels and branches:

```json
[
  {"kind":"Input","id":"mode","name":"mode","ui":{
    "choices":[{"label":"Fast","value":"fast"},{"label":"Careful","value":"careful"}],
    "screen_only":true
  }},
  {"kind":"Button","id":"options","text":"Options","title":"Configure action","inputs":["mode"]}
]
```

Buttons reference stable input IDs, so renaming or reordering inputs keeps their
settings attached. Buttons do not consume runtime arguments. Dropdowns contain
at most 128 choices with unique non-empty values. Their defaults must be one of
those text values. A button selects 1 to 128 existing inputs. Invalid metadata is
refused during definition edits and pack import/export.

## Working on both repositories

Blockwork pins the shared implementation published on blockstitch's `dev`
branch, so a fresh checkout resolves the same framework. To work on both
repositories locally, use `just blockstitch-local ../blockstitch` (the generated
`.cargo/config.toml` is local and ignored). Run Rust tests in both repositories,
then `just test-qml` in each. Use `just blockstitch-published` to return to the
pinned dependency. When publishing another shared change, pass its commit to
that command and refresh the Flatpak Cargo sources too.
