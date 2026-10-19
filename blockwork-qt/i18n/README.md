# Translations

Each `qml_<code>.ts` here is one UI language; `just translations` updates
them from the QML and compiles the `.qm` files the build embeds.

- `qml_en.ts` only holds English plural forms ("1 block", "2 blocks").
- `qml_pirate.ts` is pirate-speak English, and doubles as a complete
  example.

## Adding a language

1. Copy `qml_pirate.ts` to `qml_<code>.ts`, where `<code>` is a language
   code like `de` or `pt_BR`, and set `language="<code>"` on its `<TS>` tag.
2. Translate it in Qt Linguist (`linguist6 blockwork-qt/i18n/qml_<code>.ts`).
   Keep `%1`, `%2` and `%n` placeholders. Short block words ("of", "to",
   "in") are pieces of a block's sentence, so check them on a real block.
3. Run `just translations`, then add the language to the list in
   `SettingsPage.qml` (search for "English (Pirate)"), in its own name.
4. Commit both the `.ts` and the `.qm`.

Run log messages, the tray menu and template names come from the daemon
and are English only for now.
