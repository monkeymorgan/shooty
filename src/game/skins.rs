//! **Character skins** — the palette library the cast is dressed from.
//!
//! The Quaternius "Ultimate Modular Men" meshes are flat-material-coloured (no
//! texture atlas, no face UV), so a "skin" here is two things and nothing else:
//!
//! 1. a **per-material recolour** — a list of `(glTF material name, colour)`
//!    pairs the spawner applies by cloning each matched `StandardMaterial`
//!    once, keyed on [`bevy::gltf::GltfMaterialName`]; and
//! 2. the **face plane** texture parented to the mesh's `Head` bone
//!    (see `tools/gen_face_planes.py`).
//!
//! Because all eleven vendored characters share one 62-bone skeleton, any skin
//! can be worn by any body — the `model` field is just the one it was designed
//! against, and the asset lab lets you try it on the others.
//!
//! The genre skins are art direction, not gameplay: the band is what the player
//! could be, the crowd is the townsfolk a wave is drawn from. `guitarist`,
//! `drummer` and `gloom` are the three the game ships today, and they live here
//! rather than in `player.rs`/`enemy.rs` so the lab and the game read the same
//! numbers.

use bevy::prelude::*;

use super::scale::{self, fit};

/// Which side of the gig a skin belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cast {
    /// On stage — the playable musicians.
    Band,
    /// In front of it — ordinary townsfolk, one genre's worth of fans each.
    Crowd,
    /// Drained of colour by the gloom: a crowd member turned enemy.
    Gloom,
}

pub struct Skin {
    /// Stable id — used in filenames and by [`skin`].
    pub id: &'static str,
    /// What to call it on screen.
    pub name: &'static str,
    /// The musical idea it is dressed from, for the lab read-out.
    pub genre: &'static str,
    pub cast: Cast,
    /// The body it was designed against, under `assets/`.
    pub model: &'static str,
    /// Face-plane texture under `assets/`, or `None` for a covered face.
    pub face: Option<&'static str>,
    /// Measured height of `model` in its own units, straight off the mesh.
    pub native_h: f32,
    /// How tall this character actually is, in **metres**. Ordinary adults, so
    /// these all sit in a hand's breadth of [`scale::PERSON`] — the variation
    /// is characterisation, not power.
    pub height: f32,
    /// glTF material name -> replacement base colour.
    pub tints: &'static [(&'static str, Color)],
}

impl Skin {
    /// The uniform scale to spawn `model` at so the character stands
    /// [`Skin::height`] metres tall.
    pub fn model_scale(&self) -> f32 {
        fit(self.native_h, self.height)
    }

    /// The colour this skin repaints `mat` with, if it repaints it at all.
    pub fn tint(&self, mat: &str) -> Option<Color> {
        self.tints
            .iter()
            .find(|(name, _)| *name == mat)
            .map(|(_, c)| *c)
    }
}

/// Look a skin up by [`Skin::id`]. Panics on an unknown id — every caller
/// passes a literal, so a miss is a typo, not a runtime condition.
pub fn skin(id: &str) -> &'static Skin {
    SKINS
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("no skin `{id}`"))
}

/// Every skin in the library, band first, then crowd, gloom last.
pub const SKINS: &[Skin] = &[
    // -- band ---------------------------------------------------------------
    Skin {
        id: "guitarist",
        name: "Guitarist",
        genre: "aging punk — the player",
        cast: Cast::Band,
        model: "models/quaternius/punk.glb",
        face: Some("models/quaternius/faces/hero.png"),
        native_h: 1.97,
        height: 1.8,
        // `punk` is already most of the way there; just knock the bright shirt
        // down to black and warm the skin a touch.
        tints: &[
            ("White", Color::srgb(0.09, 0.09, 0.11)),
            ("Skin", Color::srgb(0.82, 0.66, 0.55)),
        ],
    },
    Skin {
        id: "drummer",
        name: "Drummer",
        genre: "aging punk — the sidekick",
        cast: Cast::Band,
        model: "models/quaternius/casual.glb",
        face: Some("models/quaternius/faces/drummer.png"),
        native_h: 1.86,
        height: 1.78,
        // Burnt orange over faded red and olive: the warm, light opposite of
        // the stark guitarist, so the two never blur at the top-down camera.
        tints: &[
            ("Hair", Color::srgb(0.82, 0.34, 0.12)),
            ("White", Color::srgb(0.60, 0.26, 0.20)),
            ("LightBrown", Color::srgb(0.46, 0.20, 0.16)),
            ("LightBlue", Color::srgb(0.30, 0.32, 0.17)),
            ("Red_Dark", Color::srgb(0.32, 0.12, 0.10)),
        ],
    },
    Skin {
        id: "emo",
        name: "Emo",
        genre: "mid-2000s emo / screamo",
        cast: Cast::Band,
        model: "models/quaternius/hoodie.glb",
        face: Some("models/quaternius/faces/liner.png"),
        native_h: 1.87,
        height: 1.75,
        // Black on black, and one shot of hot magenta so the silhouette still
        // reads against a night ground.
        tints: &[
            ("Skin", Color::srgb(0.88, 0.82, 0.84)),
            ("Purple", Color::srgb(0.10, 0.09, 0.12)),
            ("White", Color::srgb(0.72, 0.10, 0.36)),
            ("LightBlue", Color::srgb(0.16, 0.15, 0.20)),
            // the one bright note, on the head where the top-down camera
            // actually sees it — all-black shoulders vanish on a night ground
            ("Hair", Color::srgb(0.66, 0.14, 0.34)),
            ("Eyebrows", Color::srgb(0.06, 0.05, 0.07)),
        ],
    },
    Skin {
        id: "jpop",
        name: "J-Pop",
        genre: "Japanese idol pop",
        cast: Cast::Band,
        model: "models/quaternius/beach.glb",
        face: Some("models/quaternius/faces/bright.png"),
        native_h: 1.88,
        height: 1.63,
        // The brightest thing on the field: candy hair, white top, mint and
        // sky. Deliberately the highest-value skin in the cast.
        tints: &[
            ("Skin", Color::srgb(0.96, 0.84, 0.78)),
            ("White", Color::srgb(0.98, 0.97, 1.00)),
            ("Red_Dark", Color::srgb(0.35, 0.72, 0.86)),
            ("LightBrown", Color::srgb(0.62, 0.86, 0.80)),
            ("Hair", Color::srgb(0.94, 0.55, 0.74)),
            ("Eyebrows", Color::srgb(0.80, 0.45, 0.62)),
            ("Earrings", Color::srgb(0.98, 0.86, 0.42)),
        ],
    },
    Skin {
        id: "country",
        name: "Country",
        genre: "outlaw country",
        cast: Cast::Band,
        model: "models/quaternius/farmer.glb",
        face: Some("models/quaternius/faces/hero.png"),
        native_h: 1.86,
        height: 1.84,
        // Denim, tan leather, a straw hat and one rust-red plaid.
        tints: &[
            ("Skin", Color::srgb(0.80, 0.62, 0.48)),
            ("LightBlue", Color::srgb(0.28, 0.36, 0.52)),
            ("Brown", Color::srgb(0.44, 0.28, 0.16)),
            ("Brown2", Color::srgb(0.30, 0.18, 0.10)),
            ("Beige", Color::srgb(0.86, 0.74, 0.48)),
            ("Red", Color::srgb(0.62, 0.16, 0.14)),
            ("Eyebrows", Color::srgb(0.34, 0.24, 0.16)),
        ],
    },
    Skin {
        id: "psych60s",
        name: "Psych '60s",
        genre: "1960s psychedelia",
        cast: Cast::Band,
        model: "models/quaternius/adventurer.glb",
        face: Some("models/quaternius/faces/bright.png"),
        native_h: 1.86,
        height: 1.79,
        // Purple velvet, burnt-orange paisley, cream ruffles, gold braid — the
        // adventurer's jacket panels take a Sgt-Pepper read surprisingly well.
        tints: &[
            ("Skin", Color::srgb(0.84, 0.68, 0.56)),
            ("Green", Color::srgb(0.52, 0.20, 0.62)),
            ("LightGreen", Color::srgb(0.92, 0.48, 0.14)),
            ("Grey", Color::srgb(0.94, 0.86, 0.62)),
            ("Black", Color::srgb(0.24, 0.16, 0.30)),
            ("Brown", Color::srgb(0.58, 0.26, 0.14)),
            ("Brown2", Color::srgb(0.38, 0.16, 0.10)),
            ("Gold", Color::srgb(0.94, 0.76, 0.24)),
            ("Hair", Color::srgb(0.30, 0.18, 0.10)),
            ("Eyebrows", Color::srgb(0.28, 0.17, 0.10)),
        ],
    },
    Skin {
        id: "metal",
        name: "Metal",
        genre: "heavy metal",
        cast: Cast::Band,
        model: "models/quaternius/king.glb",
        face: Some("models/quaternius/faces/steely.png"),
        native_h: 1.9,
        height: 1.88,
        // The king's robe goes black, his crown-metal goes stud-silver, and
        // `Hair_White` is already the platinum mane — the one mesh in the pack
        // with hair long enough to headbang.
        tints: &[
            ("Skin", Color::srgb(0.86, 0.80, 0.78)),
            ("Blue", Color::srgb(0.07, 0.07, 0.09)),
            ("Beige", Color::srgb(0.14, 0.13, 0.15)),
            ("Metal", Color::srgb(0.42, 0.44, 0.48)),
            ("Metal_Dark", Color::srgb(0.16, 0.17, 0.20)),
            ("Gold", Color::srgb(0.55, 0.44, 0.18)),
            ("Hair_White", Color::srgb(0.90, 0.89, 0.86)),
            ("DarkBrown", Color::srgb(0.12, 0.11, 0.12)),
        ],
    },
    // -- crowd --------------------------------------------------------------
    Skin {
        id: "disco",
        name: "Disco",
        genre: "1970s disco",
        cast: Cast::Crowd,
        model: "models/quaternius/suit.glb",
        face: Some("models/quaternius/faces/bright.png"),
        native_h: 1.86,
        height: 1.82,
        // Same `suit` body as the gloom goon, wearing the exact opposite: a
        // white satin three-piece over an open pink shirt. Worth keeping the
        // pair together in the lab — it is the clearest proof the recolour
        // system carries a character on its own.
        tints: &[
            ("Suit", Color::srgb(0.96, 0.94, 0.90)),
            ("White", Color::srgb(0.86, 0.30, 0.52)),
            ("Tie", Color::srgb(0.94, 0.76, 0.28)),
            ("Black", Color::srgb(0.40, 0.26, 0.14)),
            ("Grey", Color::srgb(0.90, 0.88, 0.84)),
            ("DarkBrown", Color::srgb(0.36, 0.22, 0.12)),
            ("Skin", Color::srgb(0.72, 0.52, 0.36)),
            ("Hair", Color::srgb(0.16, 0.11, 0.08)),
            ("Eyebrows", Color::srgb(0.16, 0.11, 0.08)),
        ],
    },
    Skin {
        id: "grunge",
        name: "Grunge",
        genre: "early-'90s grunge",
        cast: Cast::Crowd,
        model: "models/quaternius/punk.glb",
        face: Some("models/quaternius/faces/weary.png"),
        native_h: 1.97,
        height: 1.77,
        // The punk body again, with the mohawk repainted as lank brown hair —
        // everything washed toward olive and faded denim.
        tints: &[
            ("Skin", Color::srgb(0.78, 0.66, 0.58)),
            ("White", Color::srgb(0.44, 0.50, 0.42)),
            ("Black", Color::srgb(0.22, 0.24, 0.28)),
            ("Red_Dark", Color::srgb(0.48, 0.18, 0.16)),
            ("Red", Color::srgb(0.24, 0.18, 0.13)),
            ("LightBlue", Color::srgb(0.40, 0.44, 0.54)),
            ("Earrings", Color::srgb(0.62, 0.62, 0.64)),
            ("Eyebrows", Color::srgb(0.30, 0.22, 0.16)),
        ],
    },
    Skin {
        id: "hiphop",
        name: "Hip-Hop",
        genre: "'90s hip-hop",
        cast: Cast::Crowd,
        model: "models/quaternius/worker.glb",
        face: Some("models/quaternius/faces/hero.png"),
        native_h: 1.87,
        height: 1.86,
        // The worker's hi-vis vest becomes a red puffer and the hard hat a
        // black cap; the tool-belt browns carry the gold.
        tints: &[
            ("Skin", Color::srgb(0.52, 0.36, 0.26)),
            ("Worker_Vest", Color::srgb(0.86, 0.16, 0.20)),
            ("Worker_Yellow", Color::srgb(0.10, 0.10, 0.12)),
            ("LightBrown", Color::srgb(0.90, 0.88, 0.84)),
            ("Grey", Color::srgb(0.20, 0.21, 0.24)),
            ("Black", Color::srgb(0.08, 0.08, 0.10)),
            ("Brown", Color::srgb(0.18, 0.19, 0.24)),
            ("Brown2", Color::srgb(0.12, 0.13, 0.16)),
            ("Moustache", Color::srgb(0.10, 0.08, 0.07)),
            ("Eyebrows", Color::srgb(0.10, 0.08, 0.07)),
        ],
    },
    Skin {
        id: "techno",
        name: "Techno",
        genre: "Berlin techno",
        cast: Cast::Crowd,
        model: "models/quaternius/swat.glb",
        // No face plane: the swat mesh's own visor is the face, and a quad
        // behind it just fights the geometry.
        face: None,
        native_h: 1.85,
        height: 1.8,
        // Head-to-toe black with one mirrored-cyan visor. The darkest reading
        // in the crowd — a good stress test for the gloom grade.
        tints: &[
            ("Swat", Color::srgb(0.10, 0.10, 0.12)),
            ("Swat_Black", Color::srgb(0.05, 0.05, 0.06)),
            ("Black", Color::srgb(0.05, 0.05, 0.06)),
            ("Grey", Color::srgb(0.22, 0.22, 0.26)),
            ("DarkBrown", Color::srgb(0.14, 0.13, 0.15)),
            ("Skin", Color::srgb(0.90, 0.86, 0.86)),
            ("Visor", Color::srgb(0.20, 0.86, 0.78)),
        ],
    },
    Skin {
        id: "ska",
        name: "Ska",
        genre: "2-tone ska",
        cast: Cast::Crowd,
        model: "models/quaternius/casual.glb",
        face: Some("models/quaternius/faces/hero.png"),
        native_h: 1.86,
        height: 1.74,
        // Two-tone means literally two tones: crisp white shirt, everything
        // else black. Flat materials can't do the check, so the value split
        // does the work.
        tints: &[
            ("Skin", Color::srgb(0.74, 0.56, 0.42)),
            ("White", Color::srgb(0.97, 0.97, 0.98)),
            ("LightBrown", Color::srgb(0.09, 0.09, 0.11)),
            ("LightBlue", Color::srgb(0.09, 0.09, 0.11)),
            ("Red_Dark", Color::srgb(0.10, 0.10, 0.12)),
            ("Skin_Darker", Color::srgb(0.66, 0.48, 0.36)),
            ("Hair", Color::srgb(0.10, 0.09, 0.09)),
            ("Eyebrows", Color::srgb(0.10, 0.09, 0.09)),
        ],
    },
    // -- gloom --------------------------------------------------------------
    Skin {
        id: "gloom",
        name: "Gloom goon",
        genre: "the crowd, drained",
        cast: Cast::Gloom,
        model: "models/quaternius/suit.glb",
        face: Some("models/quaternius/faces/gloom.png"),
        native_h: 1.86,
        // A shade over the band's heads — a heavy, not a giant. It used to be
        // spawned a good deal larger than that, which read as the town being
        // overrun by something the band could not physically fight.
        height: scale::GOON,
        // See memory `shooty-people-artdir`: a townsperson drained of colour,
        // still clearly a person.
        tints: &[
            ("Skin", Color::srgb(0.53, 0.53, 0.57)),
            ("Suit", Color::srgb(0.20, 0.20, 0.24)),
            ("Tie", Color::srgb(0.28, 0.10, 0.12)),
            ("White", Color::srgb(0.34, 0.34, 0.37)),
            ("Grey", Color::srgb(0.34, 0.34, 0.37)),
            ("Hair", Color::srgb(0.12, 0.12, 0.14)),
            ("Eye", Color::srgb(0.02, 0.02, 0.03)),
        ],
    },
];

// ---------------------------------------------------------------------------
// The boxy cast
// ---------------------------------------------------------------------------

/// A character in the **boxy** line — the house style the art direction is
/// actually aiming at (see ART.md "The look"): Kenney's `blocky/base.glb`, six
/// literal boxes of 24 verts, wearing a painted 1024² atlas.
///
/// There is no tint list and no face plane here because there is nothing to
/// tint and nothing to plane: hair, face, collar, sleeve and boot are all
/// *painted into the atlas* by `tools/gen_blocky_skins.py`. One person = one
/// PNG. That is the whole reason the style is cheap enough to have ten of.
pub struct Boxy {
    pub id: &'static str,
    pub name: &'static str,
    /// The musical idea it is dressed from, for the lab read-out.
    pub genre: &'static str,
    pub cast: Cast,
    /// Painted atlas under `assets/`.
    pub atlas: &'static str,
    /// How tall this character is, in **metres** — see [`Skin::height`].
    pub height: f32,
}

impl Boxy {
    /// The uniform scale to spawn [`BOXY_BASE`] at so the character stands
    /// [`Boxy::height`] metres tall.
    pub fn model_scale(&self) -> f32 {
        fit(BOXY_NATIVE_H, self.height)
    }
}

/// Look a boxy character up by [`Boxy::id`]. Panics on an unknown id.
pub fn boxy(id: &str) -> &'static Boxy {
    BOXY.iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no boxy character `{id}`"))
}

/// Five rockers and the five genre crowds they are pushing back against.
///
/// The band are the warm, high-contrast reads — leather, denim, flannel, satin
/// — and each is also a band role, so five heroes read as one group rather than
/// five strangers with the same taste. The crowds take the loud, cool, high-key
/// palettes: a wave should read as one hue from the top-down camera before you
/// can pick out a single person in it.
pub const BOXY: &[Boxy] = &[
    // -- band ---------------------------------------------------------------
    Boxy {
        id: "punk",
        name: "Punk",
        genre: "punk — lead guitar",
        cast: Cast::Band,
        atlas: "models/blocky/skins/punk.png",
        height: 1.8,
    },
    Boxy {
        id: "metal",
        name: "Metal",
        genre: "heavy metal — bass",
        cast: Cast::Band,
        atlas: "models/blocky/skins/metal.png",
        height: 1.88,
    },
    Boxy {
        id: "grunge",
        name: "Grunge",
        genre: "grunge — drums",
        cast: Cast::Band,
        atlas: "models/blocky/skins/grunge.png",
        height: 1.83,
    },
    Boxy {
        id: "glam",
        name: "Glam",
        genre: "glam rock — vocals",
        cast: Cast::Band,
        atlas: "models/blocky/skins/glam.png",
        height: 1.86,
    },
    Boxy {
        id: "rockabilly",
        name: "Rockabilly",
        genre: "rockabilly — rhythm guitar",
        cast: Cast::Band,
        atlas: "models/blocky/skins/rockabilly.png",
        height: 1.76,
    },
    // -- crowd --------------------------------------------------------------
    Boxy {
        id: "disco",
        name: "Disco",
        genre: "1970s disco",
        cast: Cast::Crowd,
        atlas: "models/blocky/skins/disco.png",
        height: 1.82,
    },
    Boxy {
        id: "techno",
        name: "Techno",
        genre: "Berlin techno",
        cast: Cast::Crowd,
        atlas: "models/blocky/skins/techno.png",
        height: 1.8,
    },
    Boxy {
        id: "hiphop",
        name: "Hip-Hop",
        genre: "'90s hip-hop",
        cast: Cast::Crowd,
        atlas: "models/blocky/skins/hiphop.png",
        height: 1.86,
    },
    Boxy {
        id: "country",
        name: "Country",
        genre: "outlaw country",
        cast: Cast::Crowd,
        atlas: "models/blocky/skins/country.png",
        height: 1.84,
    },
    Boxy {
        id: "jpop",
        name: "J-Pop",
        genre: "Japanese idol pop",
        cast: Cast::Crowd,
        atlas: "models/blocky/skins/jpop.png",
        height: 1.63,
    },
];

/// The blocky base mesh every [`Boxy`] skin is worn on.
pub const BOXY_BASE: &str = "models/blocky/base.glb";

/// Measured height of [`BOXY_BASE`] in its own units: legs 1.0 + torso 0.9 +
/// head 0.8 (the head mesh is 8 units cubed at a node scale of 0.1). Nearly a
/// third of that height is head, which is the proportion the whole style rests
/// on — see ART.md "The look".
pub const BOXY_NATIVE_H: f32 = 2.70;

/// Ground lift for [`BOXY_BASE`]. **Zero**: the mesh already stands with its
/// feet at y = 0 (`leg-left` sits at y = 1.0 with its mesh running down to
/// -1.0). This was 1.0 on the belief the origin was at the hips, which floated
/// the entire boxy cast a third of a body-height above the ground everywhere
/// it was drawn — including in the sheets it was being judged from.
pub const BOXY_LIFT: f32 = 0.0;

// ---------------------------------------------------------------------------
// What a character sounds like
// ---------------------------------------------------------------------------

/// The shape of an oscillation — the waveform a character's instrument puts
/// out, and therefore the shape of the shots they fire (see
/// [`super::weapon::ShotLook::Wave`]).
///
/// These are the four shapes a synth's oscillator selector offers, and they
/// carry roughly the associations you would expect from one: a sine is round
/// and clean, a square is hard and buzzy, a saw is bright and aggressive, a
/// triangle is soft with an edge on it. Picking one per character is the first
/// half of tying a hero to a sound; the second half — the actual audio — can
/// hang off the same table when the stems exist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Waveform {
    Sine,
    Square,
    Saw,
    Triangle,
}

impl Waveform {
    pub const ALL: [Waveform; 4] = [
        Waveform::Sine,
        Waveform::Square,
        Waveform::Saw,
        Waveform::Triangle,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Waveform::Sine => "sine",
            Waveform::Square => "square",
            Waveform::Saw => "saw",
            Waveform::Triangle => "triangle",
        }
    }

    /// One cycle of the wave, `t` in **turns** (0..1 is a full cycle), giving
    /// -1..1. Wrapped, so callers can hand it any phase.
    pub fn sample(self, t: f32) -> f32 {
        let t = t.rem_euclid(1.0);
        match self {
            Waveform::Sine => (t * std::f32::consts::TAU).sin(),
            Waveform::Square => {
                if t < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            Waveform::Saw => 1.0 - 2.0 * t,
            Waveform::Triangle => {
                if t < 0.5 {
                    4.0 * t - 1.0
                } else {
                    3.0 - 4.0 * t
                }
            }
        }
    }
}

/// A character's *signal*: what their shots are made of.
#[derive(Clone, Copy, Debug)]
pub struct Sound {
    pub wave: Waveform,
    /// How many cycles fit in one shot's length — the pitch of the thing. Low
    /// is a long lazy roll, high is a tight buzz.
    pub cycles: f32,
    /// How far the wave swings off the flight line, in world units. Amplitude
    /// is loudness: the heavy players throw a wider wave.
    pub amp: f32,
    /// Cycles per second the waveform travels through the shot while it is in
    /// the air — what makes it read as *oscillating* rather than as a fixed
    /// squiggle.
    pub hz: f32,
    pub color: Color,
}

/// The signal a character puts out, by [`Skin::id`] / [`Boxy::id`].
///
/// Deliberately a lookup rather than a field on the two tables: a sound is a
/// property of the *character*, and the same character exists in both casts.
/// Anything not listed falls through to the house sound.
pub fn sound(id: &str) -> Sound {
    match id {
        // -- the band ---------------------------------------------------
        // Punk: a hard square, fast and tight, in hot amber.
        "punk" | "guitarist" => Sound {
            wave: Waveform::Square,
            cycles: 2.2,
            amp: 0.55,
            hz: 7.0,
            color: Color::srgb(1.0, 0.74, 0.20),
        },
        // Metal: a slow, wide saw. Fewer cycles, bigger swing — it reads as
        // a lower note without a single audio sample being involved.
        "metal" => Sound {
            wave: Waveform::Saw,
            cycles: 1.3,
            amp: 0.78,
            hz: 4.0,
            color: Color::srgb(0.95, 0.36, 0.16),
        },
        // Grunge: a triangle, mid and slightly loose.
        "grunge" => Sound {
            wave: Waveform::Triangle,
            cycles: 1.7,
            amp: 0.64,
            hz: 5.0,
            color: Color::srgb(0.86, 0.72, 0.34),
        },
        // Glam: a clean, wide sine in magenta. The prettiest shot in the game
        // and it should look it.
        "glam" => Sound {
            wave: Waveform::Sine,
            cycles: 1.6,
            amp: 0.70,
            hz: 6.0,
            color: Color::srgb(1.0, 0.42, 0.86),
        },
        // Rockabilly: quick, small, bouncy sine — slapback.
        "rockabilly" => Sound {
            wave: Waveform::Sine,
            cycles: 2.6,
            amp: 0.44,
            hz: 9.0,
            color: Color::srgb(0.98, 0.86, 0.42),
        },
        "drummer" => Sound {
            wave: Waveform::Square,
            cycles: 1.0,
            amp: 0.66,
            hz: 3.0,
            color: Color::srgb(1.0, 0.60, 0.24),
        },
        "emo" => Sound {
            wave: Waveform::Triangle,
            cycles: 2.0,
            amp: 0.58,
            hz: 5.5,
            color: Color::srgb(0.90, 0.28, 0.62),
        },
        // -- the crowd (not fired yet; here so a wave has a voice when it is)
        "disco" => Sound {
            wave: Waveform::Sine,
            cycles: 2.0,
            amp: 0.54,
            hz: 8.0,
            color: Color::srgb(1.0, 0.84, 0.42),
        },
        "techno" => Sound {
            wave: Waveform::Square,
            cycles: 3.0,
            amp: 0.40,
            hz: 11.0,
            color: Color::srgb(0.30, 0.94, 0.88),
        },
        "hiphop" => Sound {
            wave: Waveform::Saw,
            cycles: 1.1,
            amp: 0.74,
            hz: 3.5,
            color: Color::srgb(0.94, 0.24, 0.28),
        },
        "country" => Sound {
            wave: Waveform::Triangle,
            cycles: 1.5,
            amp: 0.55,
            hz: 4.5,
            color: Color::srgb(0.86, 0.56, 0.24),
        },
        "jpop" => Sound {
            wave: Waveform::Sine,
            cycles: 3.2,
            amp: 0.46,
            hz: 12.0,
            color: Color::srgb(0.72, 0.92, 1.0),
        },
        _ => Sound {
            wave: Waveform::Sine,
            cycles: 1.8,
            amp: 0.58,
            hz: 6.0,
            color: Color::srgb(1.0, 0.82, 0.28),
        },
    }
}
