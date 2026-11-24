#!/usr/bin/env python3
"""Prepare a release: `just release 0.6.4`.

Sets the version, dates the changelog's Unreleased section, adds the release to
the metainfo, syncs the Flatpak manifest and its Cargo sources, commits
everything and tags the commit. Nothing is pushed.
"""

import datetime
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REPO_URL = "https://github.com/Blockworked/Blockwork"
MANIFEST = ROOT / "packaging/flatpak/com.blockworked.Blockwork.yml"


def run(*args, capture=False):
    result = subprocess.run(args, cwd=ROOT, check=True, text=True, capture_output=capture)
    return result.stdout.strip() if capture else None


def fail(message):
    sys.exit(f"release: {message}")


def set_cargo_version(version):
    path = ROOT / "Cargo.toml"
    text = path.read_text()
    match = re.search(r'^\[workspace\.package\]\n(?:.*\n)*?version = "([^"]+)"', text, re.M)
    if not match:
        fail("no version under [workspace.package] in Cargo.toml")
    previous = match.group(1)
    start, end = match.span(1)
    path.write_text(text[:start] + version + text[end:])
    # Refresh the workspace's own entries in Cargo.lock without touching dependencies.
    run("cargo", "update", "--workspace", "--offline", "--quiet")
    return previous


def date_changelog(version, previous):
    path = ROOT / "CHANGELOG.md"
    text = path.read_text()
    match = re.search(r"^## \[Unreleased\]\n(.*?)(?=^## \[)", text, re.M | re.S)
    if not match:
        fail("CHANGELOG.md has no ## [Unreleased] section")
    if not re.search(r"^- ", match.group(1), re.M):
        fail("CHANGELOG.md's Unreleased section is empty; write the release notes first")
    today = datetime.date.today().isoformat()
    text = text.replace("## [Unreleased]\n", f"## [Unreleased]\n\n## [{version}] - {today}\n", 1)
    link = f"[{version}]: {REPO_URL}/compare/{previous}...{version}\n"
    first_link = re.search(r"^\[[^\]]+\]: ", text, re.M)
    if first_link and link not in text:
        text = text[: first_link.start()] + link + text[first_link.start():]
    path.write_text(text)


def sync_flatpak_blockstitch():
    rev = re.search(r'blockstitch-qml = \{[^}]*rev = "([0-9a-f]+)"', (ROOT / "Cargo.toml").read_text())
    if not rev:
        fail("no blockstitch-qml rev in Cargo.toml")
    text = MANIFEST.read_text()
    text, count = re.subn(
        r"(url: https://github.com/Blockworked/blockstitch.git\n\s+commit: )[0-9a-f]+",
        lambda m: m.group(1) + rev.group(1),
        text,
    )
    if count != 1:
        fail("couldn't find the blockstitch source in the Flatpak manifest")
    MANIFEST.write_text(text)


def main():
    if len(sys.argv) != 2:
        fail("usage: just release <version>, for example just release 0.6.4")
    version = sys.argv[1].removeprefix("v")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[0-9A-Za-z.]+)?", version):
        fail(f"{version!r} isn't a version like 0.6.4")
    if run("git", "tag", "--list", version, capture=True):
        fail(f"tag {version} already exists")

    previous = set_cargo_version(version)
    date_changelog(version, previous)
    run(sys.executable, "packaging/linux/metainfo-release.py", version)
    sync_flatpak_blockstitch()
    run("just", "flatpak-sources")

    run("git", "add", "-A")
    run("git", "commit", "-m", f"Release {version}")
    run("git", "tag", "-a", version, "-m", version)
    print(f"Tagged {version}. Push with: git push origin HEAD {version}")


if __name__ == "__main__":
    main()
