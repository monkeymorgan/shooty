#!/usr/bin/env python3
"""Paint the boxy cast onto the Kenney "Blocky Characters" atlas.

The house style is **boxy / near-voxel** (see ART.md "The look"): a character is
six literal boxes, and everything that makes them a *person* — hair, face,
collar, sleeve, boot — is painted flat onto the atlas. `blocky/base.glb` is
24 verts a part, so a "character" here costs one 1024² PNG and nothing else.

Earlier skins recoloured a stock Kenney atlas by colour-distance
(`gen_skins.py`). That works for one businessman-turned-rocker but it can only
ever wear the clothes the source was painted in — no mohawk, no stetson, no
twin tails. This paints the atlas outright instead, from a per-character spec,
so the ten-strong cast is authored rather than found.

Atlas layout (measured off the stock textures — box unwraps, y down):

    head    128² faces: left(0,128) front(128,128) right(256,128) back(384,128)
                        top(128,0)  bottom(128,256)
    torso   band y784-928: side(0) front(96,w128) side(224) back(320,w128)
                        top(96,688,128x96)  bottom(96,928)
    arm     island x480 / x768: top+bottom (64² each) at y534, band y598-768
    leg     island x480 / x768: top+bottom (64² each) at y800, band y864-1024

Output: assets/models/blocky/skins/<id>.png
"""

from dataclasses import dataclass, field
from pathlib import Path

from PIL import Image, ImageDraw

OUT = Path("assets/models/blocky/skins")
SIZE = 1024
BG = (30, 30, 35)

# -- atlas rectangles -------------------------------------------------------

HEAD = 128
HEAD_BAND_Y = 128
# Band order left to right. `front` carries the face.
HEAD_FACES = {"left": 0, "front": 128, "right": 256, "back": 384}
HEAD_TOP = (128, 0, HEAD, HEAD)
HEAD_BOTTOM = (128, 256, HEAD, HEAD)

TORSO_BAND = (784, 144)  # y, height
TORSO_FACES = {"side_l": (0, 96), "front": (96, 128), "side_r": (224, 96), "back": (320, 128)}
TORSO_TOP = (96, 688, 128, 96)
TORSO_BOTTOM = (96, 928, 128, 96)

LIMB = 64
ARM_ISLANDS = (480, 768)
ARM_CAPS_Y, ARM_BAND = 534, (598, 170)
LEG_ISLANDS = (480, 768)
LEG_CAPS_Y, LEG_BAND = 800, (864, 160)


# -- character specs --------------------------------------------------------


@dataclass
class Char:
    """One boxy person. Colours are 8-bit sRGB triples."""

    id: str
    name: str
    genre: str
    cast: str  # "band" | "crowd"
    skin: tuple
    hair: tuple
    style: str  # crop | long | mohawk | afro | quiff | twin | cap | hat | hood
    top: tuple  # jacket / shirt — the torso base
    legs: tuple
    boots: tuple
    sleeve: tuple = None  # arm colour; defaults to `top`
    sleeve_len: float = 0.62  # fraction of the arm the sleeve covers
    boot_len: float = 0.26
    shirt: tuple = None  # painted down the middle of the torso front
    accent: tuple = None  # collar / brim / chain / stripe
    shades: tuple = None  # a band across the eyes instead of eyes
    beard: tuple = None
    brow: tuple = None  # defaults to a shade of `hair`
    note: str = ""
    face_h: float = 1.0  # scales how far down the face features sit


def dim(c, f):
    return tuple(max(0, min(255, int(v * f))) for v in c)


# Five rockers. Rock is the *player's* side, so these are the warm, high-contrast
# reads: leather, denim, flannel, satin. Each is also a band role, so the five
# read as one group rather than five strangers who like the same music.
BAND = [
    Char(
        "punk", "Punk", "punk — lead guitar", "band",
        skin=(232, 196, 168), hair=(22, 20, 24), style="mohawk",
        top=(26, 24, 28), shirt=(178, 30, 36), legs=(38, 40, 52), boots=(18, 17, 20),
        sleeve_len=0.42, accent=(196, 40, 44), brow=(30, 28, 32),
        note="the player: black leather over a red tee, sleeves torn off",
    ),
    Char(
        "metal", "Metal", "heavy metal — bass", "band",
        skin=(226, 208, 200), hair=(20, 18, 22), style="long",
        top=(16, 16, 19), shirt=(58, 58, 66), legs=(22, 22, 28), boots=(14, 14, 17),
        sleeve_len=0.30, accent=(150, 150, 158),
        note="cut-off denim vest over black, studs, hair to the waist",
    ),
    Char(
        "grunge", "Grunge", "grunge — drums", "band",
        skin=(224, 186, 158), hair=(150, 118, 70), style="long",
        top=(122, 44, 42), shirt=(158, 158, 162), legs=(96, 116, 152), boots=(86, 58, 40),
        sleeve_len=0.78, accent=(64, 26, 26), beard=(120, 92, 56),
        note="red flannel open over a grey tee, faded jeans, dirty-blond hair",
    ),
    Char(
        "glam", "Glam", "glam rock — vocals", "band",
        skin=(236, 202, 176), hair=(168, 74, 40), style="afro",
        top=(126, 58, 176), shirt=(244, 240, 232), legs=(168, 106, 208), boots=(226, 190, 84),
        sleeve_len=0.86, accent=(238, 200, 92), shades=(214, 146, 52),
        note="purple satin, open cream shirt, flares, gold platforms",
    ),
    Char(
        "rockabilly", "Rockabilly", "rockabilly — rhythm guitar", "band",
        skin=(230, 190, 160), hair=(26, 22, 22), style="quiff",
        top=(238, 234, 226), shirt=(176, 42, 46), legs=(58, 78, 122), boots=(226, 222, 214),
        sleeve_len=0.44, accent=(176, 42, 46),
        note="bowling shirt, cuffed indigo jeans, a quiff you could lose a comb in",
    ),
]

# Five crowds, one per wave — the genres rock is pushing back against. These get
# the loud, cool, high-key palettes so a wave reads as one hue from the top-down
# camera before you can make out a single person.
CROWD = [
    Char(
        "disco", "Disco", "1970s disco", "crowd",
        skin=(184, 132, 92), hair=(28, 20, 16), style="afro",
        top=(244, 242, 236), shirt=(220, 78, 132), legs=(238, 236, 230), boots=(238, 202, 92),
        sleeve_len=0.9, accent=(238, 202, 92),
        note="white three-piece over an open pink shirt, gold everything",
    ),
    Char(
        "techno", "Techno", "Berlin techno", "crowd",
        skin=(226, 216, 216), hair=(30, 30, 34), style="cap",
        top=(26, 26, 30), shirt=(18, 18, 22), legs=(32, 32, 38), boots=(16, 16, 19),
        sleeve_len=0.34, accent=(52, 220, 200), shades=(52, 220, 200),
        note="head-to-toe black and one mirrored-cyan visor",
    ),
    Char(
        "hiphop", "Hip-Hop", "'90s hip-hop", "crowd",
        skin=(132, 90, 62), hair=(24, 20, 18), style="cap",
        top=(214, 44, 48), shirt=(238, 236, 232), legs=(52, 62, 92), boots=(240, 238, 234),
        sleeve_len=0.94, accent=(238, 198, 78),
        note="red puffer, gold rope chain, baggy denim, white sneakers",
    ),
    Char(
        "country", "Country", "outlaw country", "crowd",
        skin=(206, 160, 122), hair=(96, 66, 40), style="hat",
        top=(158, 62, 56), shirt=(238, 226, 200), legs=(72, 92, 130), boots=(112, 72, 44),
        # A straw the colour of the face disappears into it — the stetson has to
        # be a good two stops darker than the skin under it to read as a hat.
        sleeve_len=0.82, accent=(166, 126, 62), beard=(104, 74, 46),
        note="straw stetson, rust plaid, denim, tooled boots",
    ),
    Char(
        "jpop", "J-Pop", "Japanese idol pop", "crowd",
        skin=(244, 214, 200), hair=(240, 140, 186), style="twin",
        top=(250, 248, 252), shirt=(120, 196, 226), legs=(246, 168, 200), boots=(250, 248, 250),
        sleeve_len=0.52, accent=(120, 196, 226), face_h=1.06,
        note="the brightest read on the field: candy hair, mint and sky",
    ),
]

CAST = BAND + CROWD


# -- painting ---------------------------------------------------------------


def shade(d: ImageDraw.ImageDraw, box, colour, top_f=1.06, bot_f=0.88):
    """Fill a box face with a gentle top-to-bottom gradient.

    Flat fills read as cardboard once six of them meet at a corner; the stock
    Kenney atlases carry the same soft vertical falloff, so we match it.
    """
    x, y, w, h = box
    for i in range(h):
        t = i / max(1, h - 1)
        d.rectangle([x, y + i, x + w - 1, y + i], fill=dim(colour, top_f + (bot_f - top_f) * t))


def band_faces(x0, y0, h, w, n=4):
    return [(x0 + i * w, y0, w, h) for i in range(n)]


def paint_head(d, c: Char):
    faces = {k: (x, HEAD_BAND_Y, HEAD, HEAD) for k, x in HEAD_FACES.items()}
    for box in list(faces.values()) + [HEAD_TOP, HEAD_BOTTOM]:
        shade(d, box, c.skin)

    hair = c.hair
    top = HEAD_TOP

    def cap(depth, which=("left", "front", "right", "back")):
        """Hair down `depth` px from the top of the named side faces."""
        for k in which:
            x, y, w, _ = faces[k]
            shade(d, (x, y, w, depth), hair, 1.04, 0.94)

    if c.style == "crop":
        shade(d, top, hair)
        cap(48)
    elif c.style == "long":
        shade(d, top, hair)
        cap(40, ("front",))
        cap(HEAD, ("back",))
        cap(108, ("left", "right"))
    elif c.style == "mohawk":
        # Shaved sides, so the crest has to carry it: a wide strip front-to-back
        # over the crown, down over the brow and the nape. The bare sides get a
        # stubble tone rather than bare scalp.
        x, y, w, h = top
        shade(d, (x + w // 2 - 30, y, 60, h), hair)
        for k in ("front", "back"):
            fx, fy, fw, _ = faces[k]
            shade(d, (fx + fw // 2 - 30, fy, 60, 40), hair, 1.04, 0.94)
        cap(20)
    elif c.style == "afro":
        # No geometry to bulge, so the mass has to be painted: hair takes most
        # of every side face and leaves the face as a window.
        shade(d, top, hair)
        cap(HEAD, ("back",))
        cap(96, ("left", "right"))
        fx, fy, fw, _ = faces["front"]
        shade(d, (fx, fy, fw, 40), hair, 1.04, 0.94)
        shade(d, (fx, fy, 14, HEAD), hair, 1.0, 0.9)
        shade(d, (fx + fw - 14, fy, 14, HEAD), hair, 1.0, 0.9)
    elif c.style == "quiff":
        shade(d, top, hair)
        cap(54, ("left", "right", "back"))
        fx, fy, fw, _ = faces["front"]
        shade(d, (fx, fy, fw, 36), hair, 1.04, 0.94)
        # the pompadour itself, rolled up over the brow
        shade(d, (fx + 24, fy, fw - 48, 58), hair, 1.1, 1.0)
    elif c.style == "twin":
        shade(d, top, hair)
        cap(44, ("front",))
        cap(HEAD, ("back",))
        # two tails down the full height of each side face
        for k in ("left", "right"):
            fx, fy, fw, fh = faces[k]
            shade(d, (fx, fy, fw, 40), hair, 1.04, 0.94)
            shade(d, (fx + 18, fy, 30, fh), hair, 1.02, 0.9)
    elif c.style in ("cap", "hat"):
        hat = c.accent if c.style == "hat" else hair
        crown = 58 if c.style == "hat" else 46
        shade(d, top, hat)
        for k in ("left", "front", "right", "back"):
            fx, fy, fw, _ = faces[k]
            shade(d, (fx, fy, fw, crown), hat, 1.04, 0.94)
        # brim: a darker lip under the crown — across the front for a cap, all
        # the way round for the stetson.
        brim = dim(hat, 0.72)
        keys = ("front",) if c.style == "cap" else ("left", "front", "right", "back")
        for k in keys:
            fx, fy, _fw, _ = faces[k]
            shade(d, (fx, fy + crown, HEAD, 14), brim, 1.0, 0.86)
        if c.style == "cap":
            # a sliver of hair at the nape so a capped head still has hair
            cap(64, ("back",))

    paint_face(d, c, faces["front"])


def paint_face(d, c: Char, box):
    """Eyes, brows and mouth on the head's front face.

    Minimal by design — the reference cast (`assets/refs/characters.jpg`) is two
    dark bars and a line. Anything finer disappears at the top-down camera.
    """
    x, y, w, h = box
    ey = y + int(h * 0.42 * c.face_h)
    ink = (36, 30, 30)
    # Hair-coloured brows vanish on a blonde and look dyed on a pink — the
    # reference cast paints every brow as the same near-black bar.
    brow = c.brow or ink

    if c.shades:
        d.rectangle([x + 14, ey - 6, x + w - 15, ey + 14], fill=c.shades)
        d.rectangle([x + 14, ey - 6, x + w - 15, ey - 3], fill=dim(c.shades, 0.7))
    else:
        for ox in (26, w - 26 - 14):
            d.rectangle([x + ox, ey, x + ox + 13, ey + 15], fill=ink)
        for ox in (24, w - 24 - 18):
            d.rectangle([x + ox, ey - 13, x + ox + 17, ey - 8], fill=brow)

    if c.beard:
        d.rectangle([x + 18, y + int(h * 0.62), x + w - 19, y + h - 1], fill=c.beard)
        d.rectangle([x + 34, y + int(h * 0.60), x + w - 35, y + int(h * 0.68)], fill=c.beard)

    my = y + int(h * 0.72)
    d.rectangle([x + w // 2 - 13, my, x + w // 2 + 12, my + 5], fill=ink)


def paint_torso(d, c: Char):
    y, h = TORSO_BAND
    for x, w in TORSO_FACES.values():
        shade(d, (x, y, w, h), c.top)
    shade(d, TORSO_TOP, dim(c.top, 1.05))
    shade(d, TORSO_BOTTOM, dim(c.top, 0.8))

    fx, fw = TORSO_FACES["front"]
    if c.shirt:
        # An open jacket. The shirt is a panel down the middle of the front
        # face, *widening* from the throat — a constant-width strip reads as a
        # tie, and a strip with the top corners cut off reads as an arrow.
        wide, narrow = int(fw * 0.42), int(fw * 0.16)
        for i in range(h):
            sw = narrow + int((wide - narrow) * min(1.0, i / 46))
            sx = fx + (fw - sw) // 2
            t = i / max(1, h - 1)
            d.rectangle([sx, y + i, sx + sw - 1, y + i], fill=dim(c.shirt, 1.06 - 0.18 * t))
        # The jacket's own edges, a shade darker than its face, so the opening
        # reads as two lapels rather than a hole cut in the chest.
        for i in range(h):
            sw = narrow + int((wide - narrow) * min(1.0, i / 46))
            sx = fx + (fw - sw) // 2
            for ex in (sx - 4, sx + sw):
                d.rectangle([ex, y + i, ex + 3, y + i], fill=dim(c.top, 0.82))

    if c.accent:
        # A collar band across the shoulders — the one bright note the top-down
        # camera actually catches, since it sits on the up-facing edge.
        d.rectangle([fx, y, fx + fw - 1, y + 9], fill=dim(c.accent, 0.92))
        for x, w in (TORSO_FACES["side_l"], TORSO_FACES["side_r"], TORSO_FACES["back"]):
            d.rectangle([x, y, x + w - 1, y + 9], fill=dim(c.accent, 0.8))


def paint_limbs(d, c: Char):
    sleeve = c.sleeve or c.top
    for x0 in ARM_ISLANDS:
        y, h = ARM_BAND
        cut = int(h * c.sleeve_len)
        for box in band_faces(x0, y, h, LIMB):
            bx, by, bw, _ = box
            shade(d, (bx, by, bw, cut), sleeve)
            shade(d, (bx, by + cut, bw, h - cut), c.skin)
        shade(d, (x0, ARM_CAPS_Y, LIMB, LIMB), dim(sleeve, 1.05))
        shade(d, (x0 + LIMB, ARM_CAPS_Y, LIMB, LIMB), dim(c.skin, 0.85))

    for x0 in LEG_ISLANDS:
        y, h = LEG_BAND
        cut = int(h * (1 - c.boot_len))
        for box in band_faces(x0, y, h, LIMB):
            bx, by, bw, _ = box
            shade(d, (bx, by, bw, cut), c.legs)
            shade(d, (bx, by + cut, bw, h - cut), c.boots)
        shade(d, (x0, LEG_CAPS_Y, LIMB, LIMB), dim(c.legs, 1.05))
        shade(d, (x0 + LIMB, LEG_CAPS_Y, LIMB, LIMB), dim(c.boots, 0.8))


def build(c: Char) -> Image.Image:
    im = Image.new("RGB", (SIZE, SIZE), BG)
    d = ImageDraw.Draw(im)
    paint_head(d, c)
    paint_torso(d, c)
    paint_limbs(d, c)
    return im


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for c in CAST:
        path = OUT / f"{c.id}.png"
        build(c).save(path)
        print(f"{path}  {c.name} ({c.cast}) — {c.note}")


if __name__ == "__main__":
    main()
