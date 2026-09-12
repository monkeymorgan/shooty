#!/usr/bin/env python3
"""Bake game-ready face textures for the Quaternius face-plane system.

The Quaternius "Ultimate Modular Men" meshes are flat-material-coloured with a
tiny baked cartoon face and no face UV, so we can't paint onto them. Instead the
engine parents a small forward-facing quad to each character's `Head` bone and
this tool paints what goes on that quad: the locked **synty rect-eye** face
(see memory `shooty-people-artdir`), one PNG per character role.

The image is **RGBA with a transparent background** — only the features
(brows / eyes / nose shadow / mouth) are drawn, so the character's own head
skin shows through and the quad never reads as a panel stuck to the face. The
painter is lifted from `tools/gen_people.py` and trimmed to that.

Output: assets/models/quaternius/faces/<role>.png   (RGBA, 256x256)
Run:    .venv/bin/python3 tools/gen_face_planes.py
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path("assets/models/quaternius/faces")
S = 256  # texture size

# feature inks
BROW_OK = (58, 44, 36, 255)
BROW_BAD = (34, 32, 40, 255)
EYE_LINE = (46, 40, 44, 255)
EYE_WHITE = (250, 250, 252, 255)
EYE_GLOOM = (198, 202, 210, 255)
NOSE = (0, 0, 0, 40)
LIP_OK = (132, 84, 80, 255)
LIP_BAD = (48, 46, 54, 255)
BLUSH = (232, 150, 138, 60)


def paint_face(*, treatment="healthy", brow="soft", mouth="neutral",
               eyes="rect", liner=False, lids=False):
    """One face's features on a transparent field. Returns an RGBA image.

    `treatment` healthy | pale | gloom  — healthy gets blush, pale drops it,
                                        gloom also greys the eye whites.
    `brow`      soft | angry | high     — flat bar, down-angled, thin arch.
    `eyes`      rect | round           — synty rectangle, or the friendly
                                        big-round variant kept for kids,
                                        idols and anyone beaming.
    `liner`     heavy eyeliner with an outer wing (emo, glam).
    `lids`      half-lidded — the eye keeps its width but loses height.
    """
    gloom = treatment == "gloom"
    im = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    p = S / 16.0

    def R(x0, y0, x1, y1, fill):
        d.rectangle([x0 * p, y0 * p, x1 * p, y1 * p], fill=fill)

    if treatment == "healthy":
        for cx in (3.2, 12.8):
            d.ellipse([(cx - 1.3) * p, 9.4 * p, (cx + 1.3) * p, 11.0 * p], fill=BLUSH)

    eye_y = 6.6
    bc = BROW_BAD if gloom else BROW_OK
    # a heavy lash line eats into where a flat brow would sit, so lift it
    brow_y = eye_y - (0.9 if liner else 0.0)
    for left in (False, True):
        x0 = 2.4  # outer edge of the right (screen-left) brow
        if brow == "angry":
            pts = [(x0, brow_y - 0.2), (x0 + 4.5, brow_y - 1.5),
                   (x0 + 4.5, brow_y - 0.4), (x0, brow_y + 0.9)]
        elif brow == "high":
            # thin, lifted and tilted up at the inner end — reads open and
            # pleased before the mouth has said anything.
            pts = [(x0 + 0.3, brow_y - 1.5), (x0 + 4.5, brow_y - 2.0),
                   (x0 + 4.5, brow_y - 1.55), (x0 + 0.3, brow_y - 1.05)]
        else:  # soft — a flat bar
            pts = [(x0, brow_y - 1.1), (x0 + 4.5, brow_y - 1.1),
                   (x0 + 4.5, brow_y - 0.35), (x0, brow_y - 0.35)]
        if left:
            pts = [(16 - a, b) for a, b in pts]
        d.polygon([(a * p, b * p) for a, b in pts], fill=bc)

    white = EYE_GLOOM if gloom else EYE_WHITE
    for cx in (4.5, 11.5):
        outer = cx < 8  # screen-left eye — the wing kicks away from centre
        a, b = cx - 1.7, cx + 1.7
        if eyes == "round":
            # a touch wider and taller than the rect eye, with a real pupil
            d.ellipse([(cx - 2.0) * p, (eye_y - 0.3) * p,
                       (cx + 2.0) * p, (eye_y + 3.0) * p], fill=EYE_LINE)
            d.ellipse([(cx - 1.5) * p, (eye_y + 0.2) * p,
                       (cx + 1.5) * p, (eye_y + 2.5) * p], fill=white)
            d.ellipse([(cx - 0.85) * p, (eye_y + 0.75) * p,
                       (cx + 0.85) * p, (eye_y + 2.4) * p], fill=EYE_LINE)
            # catchlight — the whole reason to bother with a round eye
            d.ellipse([(cx - 0.7) * p, (eye_y + 0.95) * p,
                       (cx - 0.1) * p, (eye_y + 1.55) * p], fill=EYE_WHITE)
            continue
        top = eye_y + (0.9 if lids else 0.0)
        R(a, top, b, eye_y + 2.4, EYE_LINE)
        R(a + 0.45, top + 0.4, b - 0.45, eye_y + 1.6, white)
        if liner:
            # thicken the top lash line and flick it outwards
            R(a - 0.2, top - 0.55, b + 0.2, top + 0.15, EYE_LINE)
            tip = (a - 2.1, top - 1.9) if outer else (b + 2.1, top - 1.9)
            root = (a - 0.2, top - 0.55) if outer else (b + 0.2, top - 0.55)
            base = (a + 0.6, top + 0.7) if outer else (b - 0.6, top + 0.7)
            d.polygon([(x * p, y * p) for x, y in (root, tip, base)],
                      fill=EYE_LINE)

    # a whisper of a nose — two soft shadow ticks, nothing more
    R(7.35, 9.6, 8.65, 10.25, NOSE)
    d.line([(7.2 * p, 10.35 * p), (8.8 * p, 10.35 * p)], fill=NOSE, width=int(p * 0.5))

    my = 11.9
    lip = LIP_BAD if gloom else LIP_OK
    if mouth == "neutral":
        R(6.2, my, 9.8, my + 0.55, lip)
    elif mouth == "smile":
        d.arc([5.4 * p, (my - 2.2) * p, 10.6 * p, (my + 1.4) * p], 25, 155,
              fill=lip, width=int(p * 0.6))
    elif mouth == "frown":
        d.arc([5.4 * p, (my + 0.4) * p, 10.6 * p, (my + 4.0) * p], 205, 335,
              fill=lip, width=int(p * 0.6))
    elif mouth == "smirk":
        # a flat mouth that kicks up at the left corner
        R(6.2, my, 9.4, my + 0.55, lip)
        R(9.0, my - 0.5, 9.9, my + 0.2, lip)

    return im


# Faces are named for the EXPRESSION, not the character — `src/game/skins.rs`
# picks one per skin, and several genres happily share a face.
FACES = {
    "hero": dict(brow="soft", mouth="smirk"),
    "drummer": dict(brow="soft", mouth="smile"),
    "gloom": dict(treatment="gloom", brow="angry", mouth="frown"),
    # beaming, wide-eyed — j-pop, psych, disco
    "bright": dict(brow="high", eyes="round", mouth="smile"),
    # heavy eyeliner over a flat mouth, pale — emo
    "liner": dict(treatment="pale", brow="soft", mouth="neutral", liner=True),
    # levelled brow, no smile, pale — metal
    "steely": dict(treatment="pale", brow="angry", mouth="neutral"),
    # half-lidded and unbothered — grunge
    "weary": dict(brow="soft", mouth="neutral", lids=True),
}


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for role, kw in FACES.items():
        dst = OUT / f"{role}.png"
        paint_face(**kw).save(dst)
        print("wrote", dst)


if __name__ == "__main__":
    main()
