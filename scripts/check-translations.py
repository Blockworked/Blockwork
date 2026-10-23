"""Check Qt catalogs without requiring Qt or third-party Python packages."""

from collections import Counter
from pathlib import Path
import re
import sys
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
CATALOGS = ROOT / "blockwork-qt" / "i18n"
PLACEHOLDERS = re.compile(r"%L?(?:[1-9][0-9]*|n)")
GLOBS = re.compile(r"\([^()]*\*[^()]*\)")


def check():
    errors = []
    keys_by_file = {}
    count = 0
    for path in sorted(CATALOGS.glob("qml_*.ts")):
        root = ET.parse(path).getroot()
        keys = set()
        for context in root.findall("context"):
            name = context.findtext("name")
            for message in context.findall("message"):
                source = message.findtext("source") or ""
                translation = message.find("translation")
                if translation is not None and translation.get("type") in ("obsolete", "vanished"):
                    continue
                key = (name, source, message.findtext("comment") or "", message.get("numerus", "no"))
                prefix = f"{path.name}: {name}: {source!r}"
                if key in keys:
                    errors.append(f"{prefix}: duplicate message")
                keys.add(key)
                count += 1
                if source == "Blockwork" or source.startswith(("https://", "http://", "~/")):
                    errors.append(f"{prefix}: technical literal should not be translated")
                if translation is None:
                    errors.append(f"{prefix}: missing translation element")
                    continue
                unfinished = translation.get("type") == "unfinished"
                forms = translation.findall("numerusform")
                if path.name == "qml_en.ts":
                    if message.get("numerus") != "yes" or len(forms) != 2 or unfinished:
                        errors.append(f"{prefix}: English requires two finished plural forms")
                if unfinished:
                    continue
                for form in forms or [translation]:
                    text = "".join(form.itertext())
                    if not text:
                        errors.append(f"{prefix}: empty finished translation")
                    if Counter(PLACEHOLDERS.findall(source)) != Counter(PLACEHOLDERS.findall(text)):
                        errors.append(f"{prefix}: translation changes Qt placeholders")
                    if GLOBS.findall(source) != GLOBS.findall(text):
                        errors.append(f"{prefix}: translation changes file filter patterns")
                    if path.name == "qml_en.ts" and "(s)" in text:
                        errors.append(f"{prefix}: English plural form still contains '(s)'")
        keys_by_file[path.name] = keys
    reference = keys_by_file["qml_pirate.ts"]
    for name, keys in keys_by_file.items():
        expected = {key for key in reference if key[3] == "yes"} if name == "qml_en.ts" else reference
        if keys != expected:
            errors.append(f"{name}: catalog keys differ (missing {len(expected - keys)}, extra {len(keys - expected)})")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Checked {len(keys_by_file)} catalogs and {count} messages: keys, placeholders, file filters and English plurals are valid.")
    return 0


if __name__ == "__main__":
    sys.exit(check())
