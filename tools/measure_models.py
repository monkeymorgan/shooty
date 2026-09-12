#!/usr/bin/env python3
"""Measure the real bounding box of glTF/GLB models, in their own units.

`src/game/scale.rs` only works if the *native* heights it divides by are real
measurements. They were originally taken by hand, and the one time they were
guessed instead the whole commercial district came out at half height (the table
assumed the city kit stood at y = -1; it stands at y = 0). Every new kit needs
the same numbers, so here is the thing that produces them.

Bounds come from each mesh primitive's POSITION accessor min/max, pushed through
the node transforms that actually place it — a model whose nodes carry scale
would otherwise measure wrong.

    .venv/bin/python tools/measure_models.py assets/models/industrial/*.glb
    .venv/bin/python tools/measure_models.py --rust assets/models/industrial/building-*.glb

`--rust` prints rows ready to paste into a model table: path, height, and the
mean of the footprint sides (what the ground-collision radius is derived from).
"""

import json
import struct
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parent.parent


def load_gltf(path: Path):
    """Return (json, binary chunk) for a .glb, or (json, None) for a .gltf."""
    raw = path.read_bytes()
    if raw[:4] != b"glTF":
        return json.loads(raw.decode("utf-8")), None
    # GLB: 12-byte header, then length-prefixed chunks.
    n = struct.unpack_from("<I", raw, 8)[0]
    doc, blob, off = None, None, 12
    while off < n:
        clen, ctype = struct.unpack_from("<II", raw, off)
        body = raw[off + 8 : off + 8 + clen]
        if ctype == 0x4E4F534A:  # JSON
            doc = json.loads(body.decode("utf-8"))
        elif ctype == 0x004E4942:  # BIN
            blob = body
        off += 8 + clen + (-clen % 4)
    return doc, blob


def node_matrix(node) -> np.ndarray:
    if "matrix" in node:  # glTF stores column-major
        return np.array(node["matrix"], dtype=np.float64).reshape(4, 4).T
    m = np.eye(4)
    if "scale" in node:
        m = np.diag([*node["scale"], 1.0]) @ m
    if "rotation" in node:
        x, y, z, w = node["rotation"]
        r = np.array(
            [
                [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w), 0],
                [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w), 0],
                [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y), 0],
                [0, 0, 0, 1],
            ]
        )
        m = r @ m
    if "translation" in node:
        t = np.eye(4)
        t[:3, 3] = node["translation"]
        m = t @ m
    return m


def bounds(path: Path):
    """(min, max) corner of the model's world-space AABB, or None if it has no
    positioned geometry."""
    doc, _ = load_gltf(path)
    nodes = doc.get("nodes", [])
    meshes = doc.get("meshes", [])
    accessors = doc.get("accessors", [])
    lo = np.full(3, np.inf)
    hi = np.full(3, -np.inf)

    def walk(idx: int, parent: np.ndarray):
        node = nodes[idx]
        world = parent @ node_matrix(node)
        if "mesh" in node:
            for prim in meshes[node["mesh"]].get("primitives", []):
                acc = accessors[prim["attributes"]["POSITION"]]
                if "min" not in acc or "max" not in acc:
                    continue
                # Transform all eight corners: a rotated node's AABB is not the
                # rotated AABB of its corners' min/max alone.
                a, b = acc["min"], acc["max"]
                corners = np.array(
                    [[x, y, z, 1.0] for x in (a[0], b[0]) for y in (a[1], b[1]) for z in (a[2], b[2])]
                )
                pts = (world @ corners.T).T[:, :3]
                np.minimum(lo, pts.min(axis=0), out=lo)
                np.maximum(hi, pts.max(axis=0), out=hi)
        for child in node.get("children", []):
            walk(child, world)

    roots = doc["scenes"][doc.get("scene", 0)]["nodes"]
    for r in roots:
        walk(r, np.eye(4))
    return (lo, hi) if np.isfinite(lo).all() else None


def main() -> int:
    args = [a for a in sys.argv[1:] if a != "--rust"]
    as_rust = "--rust" in sys.argv[1:]
    if not args:
        print(__doc__)
        return 2

    for arg in args:
        path = Path(arg)
        got = bounds(path)
        if got is None:
            print(f"{path}: no geometry")
            continue
        lo, hi = got
        size = hi - lo
        rel = path.relative_to(REPO) if path.is_absolute() else path
        # Paths in the model tables are relative to `assets/`.
        asset = str(rel).removeprefix("assets/")
        if as_rust:
            plan = (size[0] + size[2]) / 2.0
            print(f'    ("{asset}", {size[1]:.2f}, {plan:.2f}),')
        else:
            print(
                f"{asset:52s} h={size[1]:6.2f}  w={size[0]:6.2f}  d={size[2]:6.2f}"
                f"  feet_y={lo[1]:+.2f}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
