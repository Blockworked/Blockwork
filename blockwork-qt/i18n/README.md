# Translations

Each `qml_<code>.ts` here is one UI language, translated on Blockwork's
Weblate (see `packaging/weblate/README.md`). The `.qm` files are compiled
from them by the Translations workflow and embedded by the build.

- `qml_en.ts` only holds English plural forms ("1 block", "2 blocks").
- `qml_pirate.ts` is pirate-speak English, and doubles as a complete
  example.

Every `.qm` here shows up in Settings > Language by itself, named with
Qt's own name for the code (`pt_BR` is "Português (Brasil)").

## Translating

Use Weblate. Keep `%1`, `%2` and `%n` placeholders. Short block words
("of", "to", "in") are pieces of a block's sentence, so check them on a
real block. Some buttons and drop-downs have a fixed width, so keep labels
short.

## For developers

After changing UI text, run `just translations` to update the `.ts` files
(and compile the `.qm` files locally), then commit both. Merge Weblate's
open pull request first, since both rewrite the `.ts` files.

Run log messages, the tray menu and template names come from the daemon
and are English only for now.
