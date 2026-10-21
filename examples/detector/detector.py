#!/usr/bin/env python3
"""A small HTTP detector for Blockwork's detect blocks.

Blockwork POSTs a PNG of the window (Content-Type: image/png) to the
detector URL and reads back JSON boxes in image pixels:

    {"detections": [{"x": 10, "y": 20, "w": 30, "h": 40, "label": "player", "score": 0.9}]}

Two modes:

    detector.py --color ff0000 [--tolerance 40] [--label red]
        Finds blobs of one color, no model needed (needs Pillow and numpy;
        scipy splits the matches into separate blobs when it's installed).
    detector.py --yolo weights.pt [--confidence 0.25]
        Runs an Ultralytics model (pip install ultralytics).

It listens on http://127.0.0.1:8765/detect unless --port says otherwise.
See docs/window-detection-aim.md.
"""
import argparse
import io
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def color_detector(hex_color, tolerance, label, min_pixels):
    import numpy as np
    from PIL import Image

    target = np.array([int(hex_color[i:i + 2], 16) for i in (0, 2, 4)], dtype=np.int16)
    try:
        from scipy import ndimage
    except ImportError:
        ndimage = None

    def detect(png):
        rgb = np.asarray(Image.open(io.BytesIO(png)).convert("RGB"), dtype=np.int16)
        mask = (np.abs(rgb - target) <= tolerance).all(axis=2)
        if ndimage is not None:
            labels, _ = ndimage.label(mask)
            regions = [(sl, int((labels[sl] == i + 1).sum())) for i, sl in enumerate(ndimage.find_objects(labels))]
        else:
            ys, xs = np.nonzero(mask)
            regions = [((slice(ys.min(), ys.max() + 1), slice(xs.min(), xs.max() + 1)), len(xs))] if len(xs) else []
        boxes = []
        for (rows, cols), count in regions:
            if count < min_pixels:
                continue
            w, h = cols.stop - cols.start, rows.stop - rows.start
            boxes.append({"x": int(cols.start), "y": int(rows.start), "w": int(w), "h": int(h),
                          "label": label, "score": round(min(1.0, count / (w * h)), 3)})
        return boxes

    return detect


def yolo_detector(weights, confidence):
    from PIL import Image
    from ultralytics import YOLO

    model = YOLO(weights)

    def detect(png):
        result = model(Image.open(io.BytesIO(png)).convert("RGB"), conf=confidence, verbose=False)[0]
        boxes = []
        for (x1, y1, x2, y2), cls, score in zip(result.boxes.xyxy.tolist(), result.boxes.cls.tolist(), result.boxes.conf.tolist()):
            boxes.append({"x": x1, "y": y1, "w": x2 - x1, "h": y2 - y1,
                          "label": result.names[int(cls)], "score": round(score, 3)})
        return boxes

    return detect


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--color", help="RRGGBB color to find")
    mode.add_argument("--yolo", metavar="WEIGHTS", help="Ultralytics model file")
    parser.add_argument("--tolerance", type=int, default=40, help="per-channel color difference allowed (default 40)")
    parser.add_argument("--label", default=None, help="label for color matches (default: the color)")
    parser.add_argument("--min-pixels", type=int, default=4, help="ignore color blobs smaller than this")
    parser.add_argument("--confidence", type=float, default=0.25, help="lowest model score kept")
    parser.add_argument("--port", type=int, default=8765)
    args = parser.parse_args()

    if args.color:
        color = args.color.lstrip("#").lower()
        if len(color) != 6 or any(c not in "0123456789abcdef" for c in color):
            parser.error("--color takes six hex digits, like ff0000")
        detect = color_detector(color, args.tolerance, args.label or color, args.min_pixels)
    else:
        detect = yolo_detector(args.yolo, args.confidence)

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            try:
                png = self.rfile.read(int(self.headers.get("Content-Length", 0)))
                status, reply = 200, {"detections": detect(png)}
            except Exception as err:  # noqa: BLE001 - report it to Blockwork
                status, reply = 400, {"error": str(err)}
            body = json.dumps(reply).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"detector: listening on http://127.0.0.1:{args.port}/detect", flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
