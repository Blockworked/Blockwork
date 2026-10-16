#!/usr/bin/env python3
"""Draws the labels onto a few dataset images, to check them by eye.
Usage: python3 preview.py dataset [count]  (writes dataset/preview/)"""

import random
import sys
from pathlib import Path

from PIL import Image, ImageDraw


def main():
    data = Path(sys.argv[1] if len(sys.argv) > 1 else "dataset")
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 20
    out = data / "preview"
    out.mkdir(exist_ok=True)
    images = sorted((data / "images").glob("*.png"))
    for path in random.Random(0).sample(images, min(count, len(images))):
        img = Image.open(path).convert("RGB")
        draw = ImageDraw.Draw(img)
        label = data / "labels" / f"{path.stem}.txt"
        for line in label.read_text().splitlines() if label.exists() else []:
            _, cx, cy, w, h = (float(v) for v in line.split())
            x0, y0 = (cx - w / 2) * img.width, (cy - h / 2) * img.height
            draw.rectangle([x0, y0, x0 + w * img.width, y0 + h * img.height], outline=(255, 0, 255), width=2)
        img.save(out / path.name)
    print(f"wrote {min(count, len(images))} previews to {out}")


if __name__ == "__main__":
    main()
