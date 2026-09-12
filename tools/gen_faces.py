#!/usr/bin/env python3
"""Paint face textures for the procedural cube enemies (see ART.md).

A bad vibe is a **dark head**: a near-black cube with a miserable face on it,
and the face is the only thing with any light in it. That inversion is the
point. The first pass made them loud saturated hues — acid green, toxic purple,
hot magenta — and a street full of them read as confetti, which is the opposite
of what a wave of other people's despair should look like. Now the mass is dark
and what you actually track in a fight is a drift of glowing faces.

Because the ground is near-black and the features are bright, the same PNG is
used as both the base-colour map and the **emissive** map (see
`enemy.rs::load_enemy_assets`), so the eyes and mouth glow on their own and the
head around them stays unlit.

The expressions are negative feelings rather than monster faces - anxiety,
despair, spite - since that is what the things are.

Output: assets/models/faces/<name>.png  (256x256)
"""

from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path("assets/models/faces")
S = 256

# hue, eye colour, mouth style
FACES = {
    # the anxious one: wide staring eyes, jaw clenched. Sickly green glow.
    "anxious": {
        "skin": (11, 15, 11), "dark": (4, 6, 4),
        "eye": (150, 255, 96), "glow": (96, 214, 40),
        "eyes": "wide", "mouth": "teeth",
    },
    # the despairing one: heavy lids, flat downturned mouth. Cold violet.
    "despairing": {
        "skin": (13, 10, 17), "dark": (5, 4, 8),
        "eye": (206, 158, 255), "glow": (140, 84, 224),
        "eyes": "heavy", "mouth": "frown",
    },
    # the bitter one: narrow slits, sharp snarl. Hot red.
    "bitter": {
        "skin": (17, 9, 11), "dark": (7, 3, 4),
        "eye": (255, 138, 122), "glow": (232, 62, 58),
        "eyes": "slit", "mouth": "snarl",
    },
}


def paint(spec: dict) -> Image.Image:
    img = Image.new("RGB", (S, S), spec["skin"])
    d = ImageDraw.Draw(img)
    dark, eye, glow = spec["dark"], spec["eye"], spec["glow"]

    # A narrow dark rim only. An earlier pass added a bright 2px outline on top
    # of it for separation; at the size a head actually occupies on screen that
    # resolved to a dotted line and read as a rendering artifact, not a face.
    # The features carry the separation instead, which is why they are big.
    d.rectangle([0, 0, S - 1, S - 1], outline=dark, width=S // 40)

    # ---- eyes. Sockets are cut darker than the head, so the lit part reads as
    # something looking out of a hole rather than a sticker on a box.
    for cx in (S * 0.27, S * 0.73):
        if spec["eyes"] == "wide":
            d.rectangle([cx - S * 0.19, S * 0.20, cx + S * 0.19, S * 0.52], fill=dark)
            d.rectangle([cx - S * 0.14, S * 0.24, cx + S * 0.14, S * 0.48], fill=glow)
            d.rectangle([cx - S * 0.06, S * 0.31, cx + S * 0.06, S * 0.41], fill=eye)
        elif spec["eyes"] == "heavy":
            # Lid pulled most of the way down: only a sliver of light left.
            d.rectangle([cx - S * 0.20, S * 0.20, cx + S * 0.20, S * 0.50], fill=dark)
            d.rectangle([cx - S * 0.16, S * 0.38, cx + S * 0.16, S * 0.48], fill=glow)
            d.rectangle([cx - S * 0.10, S * 0.41, cx + S * 0.10, S * 0.45], fill=eye)
        else:  # slit
            d.rectangle([cx - S * 0.20, S * 0.24, cx + S * 0.20, S * 0.48], fill=dark)
            d.polygon(
                [
                    (cx - S * 0.18, S * 0.44),
                    (cx + S * 0.18, S * 0.33),
                    (cx + S * 0.18, S * 0.42),
                    (cx - S * 0.18, S * 0.48),
                ],
                fill=glow,
            )

    # ---- brow. Angled in for spite, flat and low for despair.
    if spec["eyes"] == "heavy":
        d.rectangle([S * 0.06, S * 0.14, S * 0.94, S * 0.21], fill=dark)
    else:
        d.polygon([(S * 0.05, S * 0.15), (S * 0.47, S * 0.28), (S * 0.47, S * 0.12)], fill=dark)
        d.polygon([(S * 0.95, S * 0.15), (S * 0.53, S * 0.28), (S * 0.53, S * 0.12)], fill=dark)

    # ---- mouth
    mx0, mx1, my = S * 0.17, S * 0.83, S * 0.60
    if spec["mouth"] == "teeth":
        d.rectangle([mx0, my, mx1, my + S * 0.24], fill=dark)
        for i in range(5):
            x = mx0 + (mx1 - mx0) * (i + 0.5) / 5
            d.rectangle([x - S * 0.045, my + S * 0.02, x + S * 0.045, my + S * 0.11], fill=glow)
            d.rectangle([x - S * 0.045, my + S * 0.13, x + S * 0.045, my + S * 0.22], fill=glow)
    elif spec["mouth"] == "frown":
        # A shallow arc turned down - drawn as steps to stay in the blocky
        # idiom. y grows downward, so the *corners* take the larger dy: the
        # first pass had this the other way round and the despairing one was
        # beaming.
        for i, (dx, dy) in enumerate(((0.00, 0.02), (0.16, 0.08), (0.31, 0.15))):
            for sgn in (-1, 1):
                x = S * 0.50 + sgn * S * dx
                d.rectangle(
                    [x - S * 0.09, my + S * dy, x + S * 0.09, my + S * dy + S * 0.085],
                    fill=glow,
                )
    else:  # snarl - bared teeth as a zigzag, lop-sided so it reads as a sneer
        d.rectangle([mx0, my, mx1, my + S * 0.22], fill=dark)
        n = 6
        for i in range(n):
            x0 = mx0 + (mx1 - mx0) * i / n
            x1 = mx0 + (mx1 - mx0) * (i + 1) / n
            top, bot = (my + S * 0.02, my + S * 0.15) if i % 2 == 0 else (
                my + S * 0.07,
                my + S * 0.20,
            )
            d.polygon([(x0, top), (x1, top), ((x0 + x1) / 2, bot)], fill=glow)

    return img


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for name, spec in FACES.items():
        dst = OUT / f"{name}.png"
        paint(spec).save(dst)
        print(f"wrote {dst}")


if __name__ == "__main__":
    main()
