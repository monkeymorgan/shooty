#!/usr/bin/env python3
"""Face / people-skin art-direction exploration (does NOT touch the game).

Target look: Synty "Simple People" — chunky cartoon, flat painterly textures,
segmented limbs, hair & hats built from 1-4 stacked blocks. The townsfolk that
become the "gloom" enemies are civilian skins with a soured face.

Produces labelled contact sheets in `screenshots/people/`:
  faces_families.png   - face style family x enemy treatment
  hair_blocks.png      - 1-4 block hair / hat shapes
  people_variety.png   - identity variety on one body
  people_corruption.png- townsperson -> gloom -> infected -> wraith
  body_limbs.png       - blocky (1-seg) vs Synty (3-seg) limb proportions

Run:  .venv/bin/python3 tools/gen_people.py
"""

from __future__ import annotations

import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

OUT = Path("screenshots/people")
CELL = 256  # work res for one painted cube face

SKIN_TONES = {
    "porcelain": (247, 214, 181), "light": (233, 189, 148),
    "tan": (206, 154, 108), "brown": (156, 104, 66), "deep": (104, 66, 46),
}
HAIR = {
    "black": (28, 26, 30), "brown": (74, 48, 32), "blonde": (216, 174, 94),
    "auburn": (150, 66, 40), "grey": (156, 154, 156), "white": (224, 222, 220),
}


def shade(rgb, f):
    return tuple(int(max(0, min(255, c * f))) for c in rgb)


def mix(a, b, t):
    return tuple(int(round(a[i] * (1 - t) + b[i] * t)) for i in range(3))


# ---------------------------------------------------------------------------
# FACE PAINTER  ->  front image, plus flat colours for the other head faces
# ---------------------------------------------------------------------------

def paint_face(*, family="synty", treatment="healthy", skin="light",
               brow="soft", eyes="rect", mouth="neutral",
               glasses=None, stubble=False):
    base = np.array(SKIN_TONES[skin], float)
    glow = None
    if treatment == "gloom":
        base = np.array(mix(tuple(base.astype(int)), (148, 148, 156), 0.5), float) * 0.92
    elif treatment == "infected":
        base = np.array(mix(tuple(base.astype(int)), (150, 196, 96), 0.5), float)
        glow = (198, 255, 120)
    elif treatment == "wraith":
        base = np.array((44, 44, 56), float)
        glow = (95, 232, 255)

    sk = tuple(base.astype(int))
    sk_d = shade(sk, 0.8)
    line = (18, 16, 20) if family == "voxel" else shade(sk, 0.46)

    S = CELL
    im = Image.new("RGB", (S, S), sk)
    # gentle face modelling for the painterly families
    if family in ("synty", "cel"):
        g = np.linspace(1.05, 0.9, S)[:, None]
        im = Image.fromarray((np.asarray(im, float) * g[..., None]).clip(0, 255).astype(np.uint8))
    d = ImageDraw.Draw(im)
    p = S / 16.0

    def R(x0, y0, x1, y1, fill):
        d.rectangle([x0 * p, y0 * p, x1 * p, y1 * p], fill=fill)

    if treatment == "healthy" and family != "voxel":
        for cx in (3.4, 12.6):
            d.ellipse([(cx - 1.3) * p, 9.4 * p, (cx + 1.3) * p, 11.0 * p],
                      fill=mix(sk, (232, 150, 138), 0.30))

    eye_y = 6.4
    if brow != "none":
        bc = shade((60, 46, 38) if treatment == "healthy" else (40, 38, 44), 1.0)
        for left, x0 in ((False, 2.6), (True, 8.9)):
            if brow == "angry":
                pts = [(x0, eye_y - 0.1), (x0 + 4.5, eye_y - 1.3),
                       (x0 + 4.5, eye_y - 0.3), (x0, eye_y + 0.8)]
                if left:
                    pts = [(16 - a, b) for a, b in pts]
                d.polygon([(a * p, b * p) for a, b in pts], fill=bc)
            elif brow == "sad":
                pts = [(x0, eye_y - 1.0), (x0 + 4.5, eye_y - 0.1),
                       (x0 + 4.5, eye_y + 0.8), (x0, eye_y + 0.2)]
                if left:
                    pts = [(16 - a, b) for a, b in pts]
                d.polygon([(a * p, b * p) for a, b in pts], fill=bc)
            else:
                xx = x0 if not left else 16 - x0 - 4.5
                R(xx, eye_y - 1.1, xx + 4.5, eye_y - 0.3, bc)

    for cx in (4.6, 11.4):
        a, b = cx - 1.7, cx + 1.7
        if eyes == "rect":
            R(a, eye_y, b, eye_y + 2.4, line)
            R(a + 0.5, eye_y + 0.4, b - 0.5, eye_y + 1.5, glow or (255, 255, 255))
        elif eyes == "dot":
            R(cx - 1.0, eye_y + 0.5, cx + 1.0, eye_y + 2.5, glow or line)
        elif eyes == "round":
            d.ellipse([a * p, eye_y * p, b * p, (eye_y + 3.2) * p],
                      fill=glow or (255, 255, 255), outline=line, width=int(p * 0.34))
            d.ellipse([(cx - 0.8) * p, (eye_y + 1.0) * p, (cx + 0.8) * p, (eye_y + 2.6) * p], fill=line)
        elif eyes == "tired":
            R(a, eye_y + 0.6, b, eye_y + 1.9, line)
            d.line([(a * p, (eye_y + 2.4) * p), (b * p, (eye_y + 2.4) * p)], fill=sk_d, width=int(p * 0.4))
        elif eyes == "hollow":
            d.ellipse([a * p, (eye_y - 0.4) * p, b * p, (eye_y + 3.4) * p], fill=(10, 8, 14))
            if glow:
                d.ellipse([(cx - 0.9) * p, (eye_y + 0.8) * p, (cx + 0.9) * p, (eye_y + 2.5) * p], fill=glow)

    if family != "voxel":
        R(7.4, 8.3, 8.9, 10.3, sk_d)
        R(7.4, 9.9, 8.9, 10.3, shade(sk, 0.66))

    my = 11.7
    lip = mix(sk, (176, 92, 88), 0.45) if treatment == "healthy" else (58, 56, 64)
    if mouth == "neutral":
        R(6.0, my, 10.0, my + 0.7, lip)
    elif mouth == "smile":
        d.arc([5.0 * p, (my - 2.4) * p, 11.0 * p, (my + 1.6) * p], 20, 160, fill=lip, width=int(p * 0.7))
    elif mouth == "frown":
        d.arc([5.0 * p, (my + 0.6) * p, 11.0 * p, (my + 4.2) * p], 200, 340, fill=lip, width=int(p * 0.7))
    elif mouth == "grit":
        R(5.6, my - 0.5, 10.4, my + 1.6, (26, 22, 24))
        for i in range(5):
            R(5.6 + i * 0.96 + 0.12, my - 0.3, 5.6 + i * 0.96 + 0.84, my + 0.9, (234, 230, 218))
    elif mouth == "open":
        d.ellipse([6.4 * p, (my - 0.6) * p, 9.6 * p, (my + 2.8) * p], fill=(38, 18, 22))

    if stubble and treatment != "wraith":
        arr = np.asarray(im).copy()
        rng = np.random.default_rng(5)
        m = np.zeros((S, S), bool)
        m[int(9.4 * p):int(13.6 * p), int(2.6 * p):int(13.4 * p)] = True
        sel = m & (rng.random((S, S)) < 0.24)
        arr[sel] = (arr[sel] * 0.7).astype(np.uint8)
        im = Image.fromarray(arr)
        d = ImageDraw.Draw(im)

    if glasses == "shades":
        R(2.0, 5.6, 14.0, 8.7, (20, 20, 24))
        R(7.3, 6.4, 8.7, 7.1, (20, 20, 24))
    elif glasses == "specs":
        for cx in (4.6, 11.4):
            d.rectangle([(cx - 2.2) * p, 5.4 * p, (cx + 2.2) * p, 8.5 * p], outline=(38, 36, 42), width=int(p * 0.36))
        d.line([(6.8 * p, 6.4 * p), (9.2 * p, 6.4 * p)], fill=(38, 36, 42), width=int(p * 0.36))

    return im, sk, sk_d


# ---------------------------------------------------------------------------
# ISO CUBE RENDERER  (front + right + top visible, flat directional shading)
# ---------------------------------------------------------------------------

UX = np.array([0.92, -0.26])   # per unit world-X  (right, tilts up)
UY = np.array([0.0, 1.06])     # per unit world-Y  (down)
UZ = np.array([0.50, -0.42])   # per unit world-Z  (depth, recedes up-right)


def _prj(o, s, x, y, z):
    return o + s * (x * UX + y * UY + z * UZ)


def _face(canvas, tex, tl, tr, bl):
    w, h = canvas.size
    tl, tr, bl = map(lambda q: np.array(q, float), (tl, tr, bl))
    ex, ey = (tr - tl) / tex.width, (bl - tl) / tex.height
    inv = np.linalg.inv(np.array([[ex[0], ey[0]], [ex[1], ey[1]]]))
    c = -inv @ tl
    warped = tex.transform((w, h), Image.AFFINE,
                           (inv[0, 0], inv[0, 1], c[0], inv[1, 0], inv[1, 1], c[1]),
                           resample=Image.BILINEAR)
    br = tr + (bl - tl)
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).polygon([tuple(tl), tuple(tr), tuple(br), tuple(bl)], fill=255)
    canvas.paste(warped, (0, 0), mask)


def _tex(fill, size=64):
    return Image.new("RGB", (size, size), tuple(int(c) for c in fill))


TOP_F, SIDE_F, FRONT_F = 1.16, 0.74, 1.0


def draw_box(canvas, o, s, p0, size, faces, edge=(26, 24, 30), ew=2):
    """faces = (front_img_or_rgb, side_rgb, top_rgb). p0 = min corner (x,y,z)."""
    x0, y0, z0 = p0
    x1, y1, z1 = x0 + size[0], y0 + size[1], z0 + size[2]
    fr, sd, tp = faces
    fr = fr if isinstance(fr, Image.Image) else _tex(fr)
    _face(canvas, Image.fromarray((np.asarray(_tex(tp), float) * TOP_F).clip(0, 255).astype(np.uint8)),
          _prj(o, s, x0, y0, z1), _prj(o, s, x1, y0, z1), _prj(o, s, x0, y0, z0))
    _face(canvas, Image.fromarray((np.asarray(_tex(sd), float) * SIDE_F).clip(0, 255).astype(np.uint8)),
          _prj(o, s, x1, y0, z0), _prj(o, s, x1, y0, z1), _prj(o, s, x1, y1, z0))
    _face(canvas, Image.fromarray((np.asarray(fr, float) * FRONT_F).clip(0, 255).astype(np.uint8)),
          _prj(o, s, x0, y0, z0), _prj(o, s, x1, y0, z0), _prj(o, s, x0, y1, z0))
    d = ImageDraw.Draw(canvas)
    for a, b in [((x0, y0, z0), (x1, y0, z0)), ((x0, y0, z0), (x0, y1, z0)),
                 ((x1, y0, z0), (x1, y1, z0)), ((x0, y1, z0), (x1, y1, z0)),
                 ((x0, y0, z0), (x0, y0, z1)), ((x1, y0, z0), (x1, y0, z1)),
                 ((x0, y0, z1), (x1, y0, z1)), ((x1, y0, z1), (x1, y1, z1)),
                 ((x1, y1, z0), (x1, y1, z1))]:
        d.line([tuple(_prj(o, s, *a)), tuple(_prj(o, s, *b))], fill=edge, width=ew)


# ---- hair / hat shapes as 1-4 blocks (on a head that spans y 0..1) ---------

def hair_blocks(style, col):
    c, cd, ct = col, shade(col, 0.75), shade(col, 1.12)
    F = (c, cd, ct)
    if style == "none":
        return []
    if style == "crew":
        return [((-0.02, -0.16, -0.02), (1.04, 0.22, 1.04), F)]
    if style == "flattop":
        return [((-0.06, -0.5, -0.06), (1.12, 0.5, 1.12), F)]
    if style == "sidepart":
        return [((-0.06, -0.34, -0.04), (1.12, 0.34, 1.1), F),
                ((-0.06, -0.16, -0.04), (0.5, 0.2, 1.1), F)]
    if style == "quiff":  # aging-rocker pompadour: slab + forward overhang
        return [((-0.04, -0.42, -0.04), (1.08, 0.42, 1.08), F),
                ((-0.06, -0.66, -0.12), (1.12, 0.3, 0.5), F),
                ((-0.06, -0.5, 0.62), (1.12, 0.4, 0.5), (cd, shade(cd, 0.8), c))]
    if style == "mohawk":
        return [((0.02, -0.12, -0.02), (0.96, 0.16, 1.0), (cd,) * 3),
                ((0.34, -0.8, 0.0), (0.32, 0.8, 1.0), F)]
    if style == "bun":
        return [((-0.02, -0.2, -0.02), (1.04, 0.24, 1.04), F),
                ((0.28, -0.34, 0.66), (0.44, 0.44, 0.4), F)]
    if style == "long":
        return [((-0.06, -0.3, -0.06), (1.12, 0.34, 1.12), F),
                ((-0.06, 0.0, 0.6), (1.12, 0.9, 0.4), F),
                ((-0.06, 0.0, -0.06), (0.14, 0.8, 1.12), F),
                ((0.92, 0.0, -0.06), (0.14, 0.8, 1.12), F)]
    if style == "cap":
        return [((-0.04, -0.2, -0.04), (1.08, 0.24, 1.08), F),
                ((-0.06, -0.06, -0.66), (1.12, 0.12, 0.66), (cd, cd, c))]
    if style == "beanie":
        return [((-0.08, -0.26, -0.08), (1.16, 0.4, 1.16), F),
                ((-0.08, 0.06, -0.08), (1.16, 0.16, 1.16), (ct, ct, ct))]
    if style == "tophat":
        return [((-0.14, -0.06, -0.14), (1.28, 0.1, 1.28), (cd,) * 3),
                ((0.06, -0.9, 0.06), (0.88, 0.86, 0.88), F)]
    if style == "hood":
        h = (72, 76, 84)
        return [((-0.14, -0.32, -0.16), (1.28, 0.5, 1.3), (h, shade(h, .8), shade(h, 1.1))),
                ((-0.14, 0.0, 0.55), (1.28, 1.0, 0.5), (h, shade(h, .8), shade(h, 1.1))),
                ((-0.14, 0.0, -0.16), (0.22, 0.95, 1.0), (h, shade(h, .8), shade(h, 1.1))),
                ((0.92, 0.0, -0.16), (0.22, 0.95, 1.0), (h, shade(h, .8), shade(h, 1.1)))]
    return []


# ---- one character bust: head + face + hair blocks + shoulders ------------

def bust(*, face_kw, hair="sidepart", hair_col="brown", outfit=(70, 90, 130),
         limbs="synty", size=380, bg=(238, 238, 240)):
    canvas = Image.new("RGB", (size, size), bg)
    s = size * 0.30
    o = np.array([size * 0.46, size * 0.30])

    front, sk, sk_d = paint_face(**face_kw)
    hc = HAIR.get(hair_col, hair_col) if isinstance(hair_col, str) else hair_col

    # shoulders / torso peeking in under the head
    oc = outfit
    draw_box(canvas, o, s, (-0.35, 1.02, -0.2), (1.7, 0.9, 1.2),
             (oc, shade(oc, 0.78), shade(oc, 1.12)))
    # neck
    draw_box(canvas, o, s, (0.34, 0.9, 0.28), (0.32, 0.2, 0.4), (sk, sk_d, shade(sk, 1.1)))
    # head
    draw_box(canvas, o, s, (0, 0, 0), (1, 1, 1), (front, sk_d, sk))
    # hair / hat blocks
    for p0, sz, F in hair_blocks(hair, hc):
        draw_box(canvas, o, s, p0, sz, F, ew=2)
    return canvas


def full_body(limbs="synty", size=460, outfit=(66, 92, 140), skin="light",
              hair="quiff", hair_col="black", bg=(238, 238, 240), label_seg=True):
    canvas = Image.new("RGB", (size, size), bg)
    s = size * 0.145
    o = np.array([size * 0.42, size * 0.12])
    sk = SKIN_TONES[skin]
    skd = shade(sk, 0.8)
    oc = outfit
    ocd, oct = shade(oc, 0.78), shade(oc, 1.12)
    trou = shade(oc, 0.6)
    shoe = (40, 38, 44)

    def box(p0, sz, col):
        c = col if isinstance(col, tuple) and len(col) == 3 and isinstance(col[0], tuple) else \
            (col, shade(col, 0.78), shade(col, 1.12))
        draw_box(canvas, o, s, p0, sz, c, ew=2)

    # legs
    if limbs == "synty":
        for lx in (0.15, 1.05):
            box((lx, 3.5, 0.15), (0.8, 1.5, 0.8), trou)      # thigh
            box((lx + 0.03, 4.9, 0.12), (0.74, 1.5, 0.82), shade(trou, 0.9))  # shin
            box((lx - 0.02, 6.3, 0.02), (0.9, 0.5, 1.15), shoe)  # foot
    else:
        for lx in (0.15, 1.05):
            box((lx, 3.5, 0.15), (0.8, 3.0, 0.8), trou)
            box((lx - 0.02, 6.3, 0.02), (0.9, 0.5, 1.15), shoe)
    # hips
    box((0.05, 3.2, 0.1), (1.9, 0.5, 0.95), ocd)
    # torso
    box((0.05, 1.3, 0.1), (1.9, 2.0, 0.95), oc)
    # arms
    if limbs == "synty":
        for ax, side in ((-0.75, 1), (2.0, -1)):
            box((ax, 1.35, 0.2), (0.7, 1.3, 0.7), oct)        # upper arm
            box((ax + 0.02, 2.55, 0.18), (0.66, 1.2, 0.72), sk)  # forearm
            box((ax - 0.02, 3.6, 0.14), (0.72, 0.55, 0.8), skd)  # hand
    else:
        for ax in (-0.75, 2.0):
            box((ax, 1.35, 0.2), (0.72, 2.4, 0.72), oct)
            box((ax - 0.02, 3.6, 0.14), (0.72, 0.5, 0.8), skd)
    # neck + head
    box((0.75, 0.9, 0.35), (0.5, 0.45, 0.5), sk)
    front, _, _ = paint_face(family="synty", skin=skin, brow="soft", eyes="rect", mouth="neutral")
    draw_box(canvas, o, s, (0.3, -0.9, -0.1), (1.4, 1.8, 1.4), (front, skd, sk), ew=2)
    for p0, sz, F in hair_blocks(hair, HAIR.get(hair_col, hair_col)):
        # scale hair-block coords (defined for a unit head) to the 1.4-wide head
        sp = (0.3 + p0[0] * 1.4, -0.9 + p0[1] * 1.8, -0.1 + p0[2] * 1.4)
        ss = (sz[0] * 1.4, sz[1] * 1.8, sz[2] * 1.4)
        draw_box(canvas, o, s, sp, ss, F, ew=2)

    d = ImageDraw.Draw(canvas)
    d.text((14, size - 26), f"{limbs}  ({'3-segment' if limbs == 'synty' else '1-segment'} limbs)",
           fill=(20, 20, 24))
    return canvas


# ---------------------------------------------------------------------------
# sheets
# ---------------------------------------------------------------------------

def label(img, text, sub=None):
    pad = Image.new("RGB", (img.width, img.height + 40), (250, 250, 251))
    pad.paste(img, (0, 0))
    d = ImageDraw.Draw(pad)
    d.text((10, img.height + 5), text, fill=(18, 18, 22))
    if sub:
        d.text((10, img.height + 22), sub, fill=(122, 122, 130))
    return pad


def grid(tiles, cols, path, title):
    tw, th = tiles[0].size
    rows = math.ceil(len(tiles) / cols)
    sh = Image.new("RGB", (cols * tw + 20, rows * th + 64), (250, 250, 251))
    ImageDraw.Draw(sh).text((14, 18), title, fill=(14, 14, 18))
    for i, t in enumerate(tiles):
        r, c = divmod(i, cols)
        sh.paste(t, (10 + c * tw, 50 + r * th))
    sh.save(path)
    print("wrote", path)


def main():
    OUT.mkdir(parents=True, exist_ok=True)

    # 1. face family x treatment
    tiles = []
    for fam in ("synty", "voxel", "cel"):
        for tr in ("healthy", "gloom", "infected", "wraith"):
            fk = dict(family=fam, treatment=tr, skin="light",
                      brow="soft" if tr == "healthy" else "angry",
                      eyes=("hollow" if tr == "wraith"
                            else {"synty": "rect", "voxel": "dot", "cel": "round"}[fam]),
                      mouth=("neutral" if tr == "healthy"
                             else "frown" if tr in ("gloom", "wraith") else "grit"))
            tiles.append(label(bust(face_kw=fk, hair="sidepart", hair_col="brown"),
                               f"{fam} / {tr}"))
    grid(tiles, 4, OUT / "faces_families.png",
         "shooty  -  face style family (rows: synty / voxel / cel)  x  enemy treatment (cols)")

    # 2. hair / hat blocks
    styles = ["crew", "flattop", "sidepart", "quiff", "mohawk", "bun",
              "long", "cap", "beanie", "tophat", "hood", "none"]
    cols = {"crew": "black", "flattop": "blonde", "sidepart": "brown", "quiff": "black",
            "mohawk": "auburn", "bun": "brown", "long": "black", "cap": (180, 54, 48),
            "beanie": (70, 110, 150), "tophat": (24, 22, 26), "hood": "black", "none": "black"}
    fk = dict(family="synty", skin="light", brow="soft", eyes="rect", mouth="neutral")
    tiles = [label(bust(face_kw=fk, hair=st, hair_col=cols[st]), st,
                   f"{len(hair_blocks(st, (0,0,0)))} block(s)") for st in styles]
    grid(tiles, 4, OUT / "hair_blocks.png",
         "shooty  -  hair & hats as 1-4 stacked blocks (silhouette does the work)")

    # 3. identity variety on one body
    ppl = [
        ("office",  dict(skin="tan", hair="crew", hair_col="grey", glasses="specs"), (150, 150, 158)),
        ("courier", dict(skin="brown", hair="cap", hair_col=(180, 54, 48)), (180, 54, 48)),
        ("hoodie",  dict(skin="light", hair="hood", hair_col="black", eyes="rect"), (72, 76, 84)),
        ("retiree", dict(skin="porcelain", hair="sidepart", hair_col="grey", stubble=True, mouth="frown"), (110, 96, 80)),
        ("skater",  dict(skin="light", hair="beanie", hair_col=(70, 110, 150), mouth="smile"), (60, 140, 120)),
        ("clubber", dict(skin="deep", hair="flattop", hair_col="black", glasses="shades"), (30, 30, 36)),
        ("student", dict(skin="light", hair="bun", hair_col="auburn", eyes="round", mouth="smile"), (200, 120, 60)),
        ("suit",    dict(skin="porcelain", hair="sidepart", hair_col="blonde", glasses="specs"), (44, 52, 78)),
        ("builder", dict(skin="deep", hair="cap", hair_col=(230, 180, 60), stubble=True), (230, 180, 60)),
        ("rocker",  dict(skin="light", hair="quiff", hair_col="black", glasses="shades"), (24, 24, 28)),
        ("mohawk",  dict(skin="tan", hair="mohawk", hair_col="auburn", mouth="grit"), (120, 40, 44)),
        ("gran",    dict(skin="porcelain", hair="bun", hair_col="white", eyes="round", mouth="smile"), (150, 90, 130)),
    ]
    tiles = [label(bust(face_kw=dict(family="synty", **{k: v for k, v in fkw.items() if k not in ("hair", "hair_col")}),
                        hair=fkw["hair"], hair_col=fkw["hair_col"], outfit=oc), name)
             for name, fkw, oc in ppl]
    grid(tiles, 4, OUT / "people_variety.png",
         "shooty  -  townsfolk variety: ONE body + mesh, differ by face / hair-blocks / outfit colour")

    # 4. corruption progression
    tiles = []
    for who, hstyle, hcol, oc in [("commuter", "sidepart", "brown", (60, 90, 130)),
                                  ("builder", "cap", (230, 180, 60), (230, 180, 60)),
                                  ("rocker", "quiff", "black", (24, 24, 28))]:
        for tr in ("healthy", "gloom", "infected", "wraith"):
            fk = dict(family="synty", treatment=tr, skin="light",
                      brow="soft" if tr == "healthy" else "angry",
                      eyes="hollow" if tr == "wraith" else "rect",
                      mouth="neutral" if tr == "healthy" else "frown")
            tiles.append(label(bust(face_kw=fk, hair=hstyle, hair_col=hcol, outfit=oc),
                               f"{who} / {tr}"))
    grid(tiles, 4, OUT / "people_corruption.png",
         "shooty  -  same townsperson: healthy -> gloom -> infected -> wraith  (skin swap only)")

    # 5. limb proportion comparison
    a = full_body(limbs="synty", hair="quiff", hair_col="black", outfit=(150, 40, 44))
    b = full_body(limbs="blocky", hair="quiff", hair_col="black", outfit=(150, 40, 44))
    cmp = Image.new("RGB", (a.width + b.width + 30, a.height + 50), (250, 250, 251))
    ImageDraw.Draw(cmp).text((14, 16), "shooty  -  limb articulation: Synty 3-segment (your pick)  vs  Kenney base.glb 1-segment",
                             fill=(14, 14, 18))
    cmp.paste(a, (10, 44))
    cmp.paste(b, (a.width + 20, 44))
    cmp.save(OUT / "body_limbs.png")
    print("wrote", OUT / "body_limbs.png")


if __name__ == "__main__":
    main()
