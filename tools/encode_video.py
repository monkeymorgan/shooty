#!/usr/bin/env python3
"""Encode a folder of frameNNNNN.png into an mp4.

ffmpeg isn't installed on this machine, but `imageio-ffmpeg` bundles a binary.

    .venv/bin/python3 tools/encode_video.py screenshots/m4/frames out.mp4 [fps]
"""
import sys
import glob
import os
import numpy as np
import imageio.v2 as imageio
from PIL import Image


def main():
    frames_dir, out = sys.argv[1], sys.argv[2]
    fps = int(sys.argv[3]) if len(sys.argv) > 3 else 30
    max_w = int(sys.argv[4]) if len(sys.argv) > 4 else 960
    files = sorted(glob.glob(os.path.join(frames_dir, "frame*.png")))
    if not files:
        raise SystemExit(f"no frames in {frames_dir}")
    w = imageio.get_writer(
        out, fps=fps, codec="libx264", quality=7, macro_block_size=1,
        ffmpeg_params=["-pix_fmt", "yuv420p", "-movflags", "+faststart", "-crf", "26"],
    )
    for f in files:
        im = Image.open(f).convert("RGB")
        if im.width > max_w:
            im = im.resize((max_w, round(im.height * max_w / im.width)), Image.LANCZOS)
        w.append_data(np.asarray(im))
    w.close()
    mb = os.path.getsize(out) / 1e6
    print(f"wrote {out} ({len(files)} frames @ {fps}fps, {mb:.1f} MB)")


if __name__ == "__main__":
    main()
