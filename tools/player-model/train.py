#!/usr/bin/env python3
"""Fine-tunes YOLOv8n on a capture.py dataset, exports it to ONNX and
installs it as Blockwork's `mc-player-320-v1` model. See README.md."""

import argparse
import hashlib
import os
import random
import shutil
from pathlib import Path

MODEL_ID = "mc-player-320-v1"


def models_dir():
    base = os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config"
    return Path(base) / "Blockwork" / "models"


def split(data, val_share, seed):
    images = sorted((data / "images").glob("*.png"))
    if not images:
        raise SystemExit(f"no images in {data / 'images'}")
    random.Random(seed).shuffle(images)
    n_val = max(1, int(len(images) * val_share))
    (data / "val.txt").write_text("".join(f"{p.resolve()}\n" for p in images[:n_val]))
    (data / "train.txt").write_text("".join(f"{p.resolve()}\n" for p in images[n_val:]))
    yaml = data / "data.yaml"
    yaml.write_text(f"path: {data.resolve()}\ntrain: train.txt\nval: val.txt\nnames:\n  0: player\n")
    return yaml, len(images) - n_val, n_val


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data", type=Path, default=Path("dataset"))
    ap.add_argument("--base", default="yolov8n.pt", help="starting weights")
    ap.add_argument("--epochs", type=int, default=150)
    # A fixed batch: autobatch fills the GPU, which on a small dataset leaves
    # only a couple of steps per epoch.
    ap.add_argument("--batch", type=int, default=16)
    ap.add_argument("--patience", type=int, default=50, help="epochs without improvement before stopping")
    ap.add_argument("--imgsz", type=int, default=320)
    ap.add_argument("--device", default="0", help="GPU index, or cpu")
    ap.add_argument("--val-share", type=float, default=0.1)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--no-install", action="store_true", help="don't copy the model into Blockwork's models folder")
    args = ap.parse_args()

    from ultralytics import YOLO

    yaml, n_train, n_val = split(args.data, args.val_share, args.seed)
    print(f"{n_train} training and {n_val} validation images")
    model = YOLO(args.base)
    model.train(data=str(yaml), imgsz=args.imgsz, epochs=args.epochs, batch=args.batch, device=args.device,
                project=str((args.data / "runs").resolve()), name=MODEL_ID, exist_ok=True, seed=args.seed,
                patience=args.patience, plots=True)
    best = Path(model.trainer.best)
    metrics = YOLO(best).val(data=str(yaml), imgsz=args.imgsz, device=args.device, plots=False)
    print(f"validation: mAP50 {metrics.box.map50:.3f}, mAP50-95 {metrics.box.map:.3f}, "
          f"precision {metrics.box.mp:.3f}, recall {metrics.box.mr:.3f}")

    onnx = Path(YOLO(best).export(format="onnx", imgsz=args.imgsz, simplify=True))
    final = args.data / f"{MODEL_ID}.onnx"
    shutil.copy(onnx, final)
    digest = hashlib.sha256(final.read_bytes()).hexdigest()
    megabytes = max(1, round(final.stat().st_size / 1e6))
    print(f"\n{final}: {megabytes} MB, sha256 {digest}")
    if not args.no_install:
        dest = models_dir() / f"{MODEL_ID}.onnx"
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(final, dest)
        print(f"installed as {dest}")
    print("Once it's hosted, set url, sha256 and megabytes for it in blockwork-core/src/detect/models.rs:")
    print(f'    sha256: "{digest}",\n    megabytes: {megabytes},')


if __name__ == "__main__":
    main()
