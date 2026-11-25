"""Add a <release> for VERSION to the metainfo from CHANGELOG.md, if it has none.

Software centers show the newest <release> as the app's version, so every
release needs one. Usage: metainfo-release.py VERSION [METAINFO] [CHANGELOG]
"""

import html
import re
import sys
from pathlib import Path


def section(changelog, version):
    match = re.search(r"^## \[" + re.escape(version) + r"\] - (\S+)\n(.*?)(?=^## \[|\Z)", changelog, re.M | re.S)
    if not match:
        sys.exit(f"No CHANGELOG.md section for {version}")
    items = [re.sub(r"`([^`]*)`", r"\1", line[2:].strip()) for line in match.group(2).splitlines() if line.startswith("- ")]
    return match.group(1), items


def main():
    version = sys.argv[1].removeprefix("v")
    metainfo = Path(sys.argv[2] if len(sys.argv) > 2 else "res/com.blockworked.Blockwork.metainfo.xml")
    changelog = Path(sys.argv[3] if len(sys.argv) > 3 else "CHANGELOG.md").read_text()
    text = metainfo.read_text()
    if f'<release version="{version}"' in text:
        return
    date, items = section(changelog, version)
    lines = [f'    <release version="{version}" date="{date}">', "      <description>", "        <ul>"]
    lines += [f"          <li>{html.escape(item, quote=False)}</li>" for item in items]
    lines += ["        </ul>", "      </description>", "    </release>"]
    metainfo.write_text(text.replace("  <releases>\n", "  <releases>\n" + "\n".join(lines) + "\n", 1))


if __name__ == "__main__":
    main()
