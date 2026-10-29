#!/usr/bin/env python3
"""Run editor, community palette and library tests using source QML without native plugins."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
blockstitch = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root / "vendor/blockstitch-qml"
qtpaths = shutil.which("qtpaths6") or shutil.which("qtpaths")
qtbin = Path(subprocess.check_output([qtpaths, "--query", "QT_HOST_BINS"], text=True).strip()) if qtpaths else Path("/usr/lib/qt6/bin")

with tempfile.TemporaryDirectory(prefix="blockwork-qml-") as temporary:
    modules = Path(temporary)
    for name, source in [("Blockstitch", blockstitch / "qml/Blockstitch"), ("Blockwork", root / "blockwork-qt/qml")]:
        destination = modules / "com/blockworked" / name
        destination.mkdir(parents=True)
        files = list(source.glob("*.qml"))
        if name == "Blockwork":
            files += list((source / "Lucide").glob("*.qml"))
        entries = [f"module com.blockworked.{name}"]
        for file in files:
            shutil.copyfile(file, destination / file.name)
            singleton = "singleton " if "pragma Singleton" in file.read_text() else ""
            entries.append(f"{singleton}{file.stem} 1.0 {file.name}")
        (destination / "qmldir").write_text("\n".join(entries) + "\n")
    shutil.copytree(root / "blockwork-qt/qml/BlockworkStyle", modules / "BlockworkStyle")
    environment = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "QML_DISABLE_DISK_CACHE": "1"}
    result = subprocess.run([
        str(qtbin / "qmltestrunner"), "-input", str(root / "blockwork-qt/tests"), "-import", str(modules),
        *sys.argv[2:],
    ], env=environment)
    sys.exit(result.returncode)
