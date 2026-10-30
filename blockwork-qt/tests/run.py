#!/usr/bin/env python3
"""Run source QML tests. Args: [blockstitch checkout] [qmltestrunner options/test filters...]."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

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
    # Match qt_diagnostics.cpp: the app uses its text-editing style and Basic
    # fallback, not macOS's non-customizable native controls.
    environment = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "QML_DISABLE_DISK_CACHE": "1",
                   "QT_QUICK_CONTROLS_STYLE": "BlockworkStyle", "QT_QUICK_CONTROLS_FALLBACK_STYLE": "Basic"}
    # Match the Mac app's Qt 6.10 incremental-GC mitigation. An explicit
    # override still permits before/after diagnostics on the same sources.
    if sys.platform == "darwin":
        version = subprocess.check_output([str(qtbin / "qtpaths"), "--qt-version"], text=True).strip()
        if version.startswith("6.10."):
            environment.setdefault("QV4_GC_TIMELIMIT", "0")
    started_at = time.time()
    result = subprocess.run([
        str(qtbin / "qmltestrunner"), "-input", str(root / "blockwork-qt/tests"), "-import", str(modules),
        *sys.argv[2:],
    ], env=environment)
    if sys.platform == "darwin" and os.environ.get("GITHUB_ACTIONS") == "true" and result.returncode < 0:
        # Preserve only this runner process's new reports, never local-user
        # diagnostics. macOS can finish writing a crash report after exit.
        reports_dir = Path.home() / "Library/Logs/DiagnosticReports"
        destination = root / "target/macos-qml-crashes"
        deadline = time.monotonic() + 8
        while True:
            reports = [report for pattern in ("qmltestrunner*.ips", "qmltestrunner*.crash")
                       for report in reports_dir.glob(pattern) if report.stat().st_mtime >= started_at]
            if reports or time.monotonic() >= deadline:
                break
            time.sleep(0.2)
        for report in reports:
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(report, destination / report.name)
            print(f"Preserved native QML crash report: {report.name}", flush=True)
    sys.exit(result.returncode)
