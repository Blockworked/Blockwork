#!/usr/bin/env python3
"""Locate the tracked Blockstitch snapshot and fetch Lucide at its lock revision."""
from pathlib import Path
import subprocess
import sys
import tomllib

root = Path(__file__).resolve().parents[1]
packages = tomllib.loads((root / "Cargo.lock").read_text())["package"]
output = root / "target/android/deps"
print(root / "vendor/blockstitch-qml")
for name in ("lucide-qt",):
    source = next(p["source"] for p in packages if p["name"] == name)
    url, rev = source.removeprefix("git+").split("#")
    url = url.split("?")[0]
    directory = output / f"{name}-{rev}"
    if not directory.exists():
        directory.mkdir(parents=True)
        subprocess.run(["git", "init", "-q", str(directory)], check=True)
    current = subprocess.run(["git", "-C", str(directory), "rev-parse", "HEAD"], capture_output=True, text=True)
    if current.stdout.strip() != rev:
        subprocess.run(["git", "-C", str(directory), "fetch", "--depth=1", url, rev], check=True, stdout=sys.stderr)
        subprocess.run(["git", "-C", str(directory), "checkout", "--detach", "FETCH_HEAD"], check=True, stdout=sys.stderr)
    print(directory)
