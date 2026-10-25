#!/usr/bin/env python3
"""Run source QML tests. Args: [blockstitch checkout] [qmltestrunner options/test filters...]."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[2]
blockstitch = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else root.parent / "blockstitch"
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
    # The real PromptBridge is a Rust QObject that exits its process on reply.
    # A test-only bridge records replies while exercising the actual dialog UI.
    prompt_module = modules / "com/blockworked/Blockwork"
    shutil.copyfile(root / "blockwork-qt/tests/mocks/PromptBridge.qml", prompt_module / "PromptBridge.qml")
    with (prompt_module / "qmldir").open("a") as module_file:
        module_file.write("PromptBridge 1.0 PromptBridge.qml\n")
    shutil.copytree(root / "blockwork-qt/qml/BlockworkStyle", modules / "BlockworkStyle")
    environment = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "QML_DISABLE_DISK_CACHE": "1"}
    result = subprocess.run([
        str(qtbin / "qmltestrunner"), "-input", str(root / "blockwork-qt/tests"), "-import", str(modules),
        *sys.argv[2:],
    ], env=environment)
    sys.exit(result.returncode)
