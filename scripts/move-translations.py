"""Carry translations along when QML moves to a new file.

Qt names a qsTr() string's context after its file, so moving code from
EditorPage.qml to ScreenPicker.qml would leave every translation of it behind.
This moves each message whose source the new file uses from one context to
another in every catalog. Run `just translations` afterwards to refresh line
numbers and compile the .qm files.

    python scripts/move-translations.py EditorPage blockwork-qt/qml/ScreenPicker.qml
    python scripts/move-translations.py InstructionBlock ../blockstitch/qml/Blockstitch/BlockMenu.qml

A string the old file still uses is copied rather than moved.
"""

import os
from pathlib import Path
import re
import sys

CATALOGS = Path(__file__).resolve().parents[1] / "blockwork-qt" / "i18n"
QSTR = re.compile(r'qsTr\("((?:[^"\\]|\\.)*)"')


def escape(text):
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("'", "&apos;").replace('"', "&quot;")


def context_block(catalog, name):
    return re.search(r"<context>\n    <name>" + re.escape(name) + r"</name>\n.*?</context>\n", catalog, re.S)


def main(old, new_file):
    new_file = Path(new_file)
    new = new_file.stem
    # Catalog locations are relative to the catalog folder, as lupdate writes them.
    location = os.path.relpath(new_file.resolve(), CATALOGS)
    lines = new_file.read_text(encoding="utf-8").split("\n")
    wanted = {}
    for number, line in enumerate(lines, 1):
        for match in QSTR.finditer(line):
            wanted.setdefault(match.group(1).replace('\\"', '"'), number)
    # The old file may sit in either repo (a string can move from blockstitch to Blockwork).
    roots = [CATALOGS.parent / "qml", CATALOGS.parents[2] / "blockstitch" / "qml" / "Blockstitch"]
    old_files = [root / f"{old}.qml" for root in roots if (root / f"{old}.qml").exists()]
    still_used = set()
    for path in old_files:
        still_used |= {m.group(1).replace('\\"', '"') for m in QSTR.finditer(path.read_text(encoding="utf-8"))}
    moved_total = 0
    for path in sorted(CATALOGS.glob("qml_*.ts")):
        catalog = path.read_text(encoding="utf-8")
        found = context_block(catalog, old)
        if not found:
            continue
        old_block, moved = found.group(0), []
        target = context_block(catalog, new)
        for source, number in wanted.items():
            message = re.search(
                r"    <message>\n(?:(?!    </message>).)*?<source>" + re.escape(escape(source)) + r"</source>.*?    </message>\n",
                old_block,
                re.S,
            )
            if not message:
                continue
            if source not in still_used:
                old_block = old_block.replace(message.group(0), "", 1)
            # Already in the new context: nothing to carry over.
            if target and f"<source>{escape(source)}</source>" in target.group(0):
                continue
            # One location, at the new file's first use.
            text = re.sub(r'(        <location filename="[^"]*" line="\d+"/>\n)+', f'        <location filename="{location}" line="{number}"/>\n', message.group(0))
            moved.append((number, text))
        if old_block == found.group(0) and not moved:
            continue
        moved_total += len(moved)
        catalog = catalog.replace(found.group(0), old_block, 1)
        if not moved:
            path.write_text(catalog, encoding="utf-8")
            continue
        target = context_block(catalog, new)
        body = "".join(text for _, text in sorted(moved))
        if target:
            catalog = catalog.replace(target.group(0), target.group(0)[: -len("</context>\n")] + body + "</context>\n", 1)
        else:
            block = f"<context>\n    <name>{new}</name>\n{body}</context>\n"
            later = [m.start() for m in re.finditer(r"<context>\n    <name>([^<]+)</name>", catalog) if m.group(1) > new]
            at = later[0] if later else catalog.rindex("</TS>")
            catalog = catalog[:at] + block + catalog[at:]
        path.write_text(catalog, encoding="utf-8")
    print(f"Moved {moved_total} messages from {old} to {new}.")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    main(sys.argv[1], sys.argv[2])
