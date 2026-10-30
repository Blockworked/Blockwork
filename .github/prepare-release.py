"""Validate release downloads and render notes for the requested version."""

import argparse
from pathlib import Path
import re


DOWNLOADS = [
    ("Windows x86_64 (installer)", "blockwork-windows-x86_64-setup.exe", True),
    ("Windows x86_64 (portable Qt bundle)", "blockwork-windows-x86_64.zip", True),
    ("Windows x86_64 (MSIX, for the Microsoft Store)", "blockwork-windows-x86_64.msix", True),
    ("Windows arm64 (MSIX, for the Microsoft Store)", "blockwork-windows-arm64.msix", False),
    ("Windows arm64 (installer)", "blockwork-windows-arm64-setup.exe", False),
    ("Windows arm64 (portable Qt bundle)", "blockwork-windows-arm64.zip", False),
    ("Linux x86_64 (AppImage)", "blockwork-linux-x86_64.AppImage", True),
    ("Linux x86_64 (Flatpak)", "blockwork-linux-x86_64.flatpak", True),
    ("Linux arm64 (AppImage)", "blockwork-linux-arm64.AppImage", True),
    ("Linux arm64 (Flatpak)", "blockwork-linux-arm64.flatpak", True),
    ("macOS arm64 (app bundle, unsigned)", "blockwork-macos-arm64.app.zip", True),
    ("macOS x86_64 (app bundle, unsigned)", "blockwork-macos-x86_64.app.zip", True),
]


def release_body(changelog, tag, artifacts):
    version = tag.removeprefix("v")
    section = []
    found = False
    for line in changelog.splitlines():
        if line.startswith("## ["):
            if found:
                break
            found = bool(re.match(r"^## \[" + re.escape(version) + r"\](?:\s|$)", line))
        if found:
            section.append(line)
    if not section:
        raise ValueError(f"No changelog section for {tag}")

    rows = []
    missing = []
    for label, filename, required in DOWNLOADS:
        path = artifacts / filename
        if path.is_file() and path.stat().st_size:
            rows.append(f"| {label} | `{filename}` |")
        elif required:
            missing.append(filename)
    if missing:
        raise ValueError("Missing or empty required release artifacts: " + ", ".join(missing))
    return "\n".join(section).rstrip() + "\n\n## Downloads\n\n| Platform | File |\n|---|---|\n" + "\n".join(rows) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--changelog", type=Path, default=Path("CHANGELOG.md"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        body = release_body(args.changelog.read_text(), args.tag, args.artifacts)
    except ValueError as error:
        parser.exit(1, f"{error}\n")
    args.output.write_text(body)


if __name__ == "__main__":
    main()
