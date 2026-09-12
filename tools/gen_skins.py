#!/usr/bin/env python3
"""Paint character skins for the Kenney "Blocky Characters" base mesh.

"One mesh, many skins" (see ART.md): every character is `blocky/base.glb` with a
different atlas swapped onto its materials. Rather than paint the box unwrap
blind, we recolour `Textures/texture-p.png` — the stock businessman skin, which
already has the cube UV layout and moulded sunglasses.

texture-p is painted from a small flat palette and each garment colour is
unique, so we classify every texel to its nearest source anchor and remap by
anchor, keeping the per-texel light/shade. Anchors (sampled):

    (181,101,68)  hair          (90,96,194)  blazer -> jacket
    (205,137,97)  chinos -> legs (163,88,57)  shoes  -> boots
    (255,255,255) shirt         (77,80,94)   shades (kept)
    (244,202,152) skin (kept)   (42,42,49)   background (kept)

Output: assets/models/blocky/skins/<name>.png
"""

from pathlib import Path

import numpy as np
from PIL import Image

SRC = Path("assets/models/blocky/Textures/texture-p.png")
OUT = Path("assets/models/blocky/skins")

# source anchor -> role
ANCHORS = {
    (42, 42, 49): "bg",
    (30, 30, 35): "bg",
    (77, 80, 94): "shades",
    (244, 202, 152): "skin",
    (246, 209, 165): "skin",
    (233, 181, 138): "skin",
    (181, 101, 68): "hair",
    (90, 96, 194): "jacket",
    (255, 255, 255): "shirt",
    (205, 137, 97): "legs",
    (163, 88, 57): "boots",
}
KEEP = {"bg", "shades", "skin"}

SKINS = {
    # Aging rocker, per ART.md: black hair block, cream face, black wraparound
    # shades, black leather jacket, near-black jeans. Stark — the only colour on
    # the character is the guitar-gun, a separate prop. (Pale hair here read as a
    # pirate from the top-down camera; the ground ring handles findability.)
    "hero": {
        "hair": (16, 16, 20),
        "jacket": (18, 18, 22),
        "shirt": (24, 24, 28),
        "legs": (33, 39, 58),
        "boots": (12, 12, 14),
    },
    # Drummer — the co-op second hero. Deliberately the warm/light opposite of
    # the stark-black guitarist so the two never blur together at the top-down
    # camera: burnt-orange hair, a warm grey sleeveless top over a faded-red
    # shirt, olive work trousers. Reads as "the other one" at a glance.
    "drummer": {
        "hair": (206, 92, 38),
        "jacket": (122, 112, 104),
        "shirt": (156, 46, 42),
        "legs": (94, 86, 66),
        "boots": (46, 34, 28),
    },
    # Goon — a townsperson the gloom has got to. texture-p (the businessman) has
    # a moulded beard + sunglasses + tie that recolour into a pirate, so the
    # goon instead takes the clean-faced casual `texture-c` verbatim, knocked
    # slightly grey + dim so it reads as "bad vibe", not a cheerful NPC.
    "goon": "texture-c desat=0.45 dim=0.82",
}


def build(pal: dict) -> Image.Image:
    src = np.asarray(Image.open(SRC).convert("RGB")).astype(np.int32)
    flat = src.reshape(-1, 3)

    anchors = np.array(list(ANCHORS.keys()), np.int32)
    roles = list(ANCHORS.values())
    # nearest anchor per texel
    d = ((flat[:, None, :] - anchors[None, :, :]) ** 2).sum(2)
    nearest = d.argmin(1)

    out = flat.copy()
    for ai, role in enumerate(roles):
        # A skin may explicitly override a normally-kept role (e.g. the goon
        # repaints "shades" to skin tone to lose the sunglasses).
        if role not in pal:
            continue
        sel = nearest == ai
        if not sel.any():
            continue
        lum = out[sel].mean(1, keepdims=True).astype(np.float32)
        shade = np.clip(lum / max(lum.mean(), 1.0), 0.7, 1.3)
        out[sel] = np.clip(np.array(pal[role], np.float32) * shade, 0, 255)

    return Image.fromarray(out.reshape(src.shape).astype(np.uint8), "RGB")


def build_copy(spec: str) -> Image.Image:
    """`"<texture> desat=<0..1> dim=<0..1>"` — take a Kenney atlas verbatim and
    optionally pull it toward grey / darken it."""
    parts = spec.split()
    tex = parts[0]
    opts = dict(p.split("=") for p in parts[1:])
    desat = float(opts.get("desat", 0.0))
    dim = float(opts.get("dim", 1.0))
    src = np.asarray(
        Image.open(SRC.parent / f"{tex}.png").convert("RGB")
    ).astype(np.float32)
    lum = src.mean(2, keepdims=True)
    out = np.clip(((src * (1 - desat) + lum * desat) * dim), 0, 255)
    return Image.fromarray(out.astype(np.uint8), "RGB")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, pal in SKINS.items():
        dst = OUT / f"{name}.png"
        img = build_copy(pal) if isinstance(pal, str) else build(pal)
        img.save(dst)
        print(f"wrote {dst}")


if __name__ == "__main__":
    main()
