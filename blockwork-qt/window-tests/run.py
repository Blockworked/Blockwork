#!/usr/bin/env python3
"""Build and run the window tests with the real QWindowKit event filters."""
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
build = root / "target/window-tests"
qmake = os.environ.get("QMAKE") or shutil.which("qmake6") or shutil.which("qmake")
if not qmake:
    sys.exit("Set QMAKE to the Qt SDK used by Blockwork")
os.environ["PATH"] = str(Path(qmake).resolve().parent) + os.pathsep + os.environ["PATH"]
libraries = subprocess.check_output([qmake, "-query", "QT_INSTALL_LIBS"], text=True).strip()
options = [f"-DQt6_DIR={libraries}/cmake/Qt6", "-DCMAKE_BUILD_TYPE=Release",
           "-DFETCHCONTENT_SOURCE_DIR_QWINDOWKIT=" + os.environ.get("BLOCKWORK_QWINDOWKIT_SOURCE_DIR", ""),
           "-DFETCHCONTENT_FULLY_DISCONNECTED=" + ("ON" if os.environ.get("CARGO_NET_OFFLINE") == "true" else "OFF")]
subprocess.run(["cmake", "-S", str(Path(__file__).parent), "-B", str(build), *options], check=True)
subprocess.run(["cmake", "--build", str(build), "--config", "Release", "--parallel", "4"], check=True)
name = "blockwork-window-tests.exe" if sys.platform == "win32" else "blockwork-window-tests"
executable = next(path for path in [build / "Release" / name, build / name] if path.is_file())
environment = {**os.environ, "BLOCKWORK_QML_TEST_RUNNER": str(executable),
               "BLOCKWORK_QML_TESTS": str(Path(__file__).parent / "qml")}
sys.exit(subprocess.run([sys.executable, str(root / "blockwork-qt/tests/run.py"),
                        str(root.parent / "blockstitch"), *sys.argv[1:]], env=environment).returncode)
