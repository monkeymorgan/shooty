#!/usr/bin/env python3
"""Chroma-key matte: knock out a solid background color and write RGBA PNG.

Flat cartoon art on a pure key color mattes cleanly without a segmentation
model. Usage:
    python3 tools/chroma_matte.py in.png out.png --key 255,0,255 --tol 60
"""
import argparse
import numpy as np
from PIL import Image


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("src")
    ap.add_argument("dst")
    ap.add_argument("--key", default="255,0,255", help="R,G,B of background")
    ap.add_argument("--tol", type=float, default=70.0, help="color distance threshold")
    ap.add_argument("--feather", type=float, default=18.0, help="soft edge width")
    ap.add_argument("--crop", action="store_true", help="crop to a square around the subject")
    ap.add_argument("--pad", type=float, default=0.06, help="crop padding as fraction of side")
    args = ap.parse_args()

    key = np.array([int(x) for x in args.key.split(",")], dtype=np.float32)
    img = Image.open(args.src).convert("RGB")
    arr = np.asarray(img, dtype=np.float32)

    dist = np.sqrt(((arr - key) ** 2).sum(axis=2))
    # alpha 0 where close to key, 1 where far, soft ramp between.
    alpha = np.clip((dist - args.tol) / max(args.feather, 1e-3), 0.0, 1.0)

    rgba = np.dstack([arr, alpha * 255.0]).astype(np.uint8)
    out = Image.fromarray(rgba, "RGBA")

    if args.crop:
        ys, xs = np.where(alpha > 0.5)
        if len(xs):
            x0, x1, y0, y1 = xs.min(), xs.max(), ys.min(), ys.max()
            cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
            half = max(x1 - x0, y1 - y0) / 2 * (1 + args.pad)
            box = (
                int(round(cx - half)),
                int(round(cy - half)),
                int(round(cx + half)),
                int(round(cy + half)),
            )
            out = out.crop(box)

    out.save(args.dst)
    kept = (alpha > 0.5).mean() * 100
    print(f"{args.dst}: {kept:.1f}% opaque, {out.size}")


if __name__ == "__main__":
    main()
