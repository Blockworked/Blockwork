# Translations

Each `qml_<code>.ts` here is one UI language, translated on Blockwork's
Weblate (see `packaging/weblate/README.md`). The `.qm` files are compiled
from them by the Translations workflow and embedded by the build.

- `qml_en.ts` only holds English plural forms ("1 block", "2 blocks").
- `qml_pirate.ts` is pirate-speak English, and doubles as an example
  translation.

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

Run `python scripts/check-translations.py` from the repository root to check
catalog keys, placeholders, file filters and English plural forms. New plural
messages need both English forms filled in `qml_en.ts` before compiling.

Translate visible labels, help text, accessibility names and default dialog
messages. Keep protocol values, enum keys, HTTP methods, model class names,
code samples, URLs, file paths, icons, language autonyms and the Blockwork
brand literal unchanged. Translate the display label separately from its
stored value. File filter descriptions are translatable; their wildcard
patterns must stay unchanged. Date-format codes in help text must also stay
unchanged.

Use `AppDialog` for application dialogs so their standard buttons follow the
selected language. Use its `acceptText` property for a custom OK label.

Run log messages, the tray menu and template names come from the daemon
and are English only for now.
