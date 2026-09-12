//! **Music gear** — the instruments the cast carries, and the poses that put a
//! piece of gear on a body.
//!
//! Two separable things live here on purpose:
//!
//! 1. **The gear itself** ([`Prop`]) — chunky flat-colour primitives built to
//!    the same box vocabulary as the characters (see ART.md "The look"). Every
//!    prop is modelled **neck-forward down +Z with its grip at the origin**, so
//!    the same mount maths works for all of them.
//! 2. **How it is worn** ([`Carry`]) — slung across the chest, aimed from the
//!    right hand, or stowed flat on the back. A carry pose is a *bone name plus
//!    a local transform*, and it differs per rig: the Quaternius skeleton and
//!    the Kenney blocky rig have different bones, different scales and
//!    different bone orientations.
//!
//! Keeping the poses as data here — rather than as literals at each call site —
//! is what lets the asset lab dress a character exactly the way the game does,
//! and lets the lab's nudge mode print a number you can paste straight back in.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

/// Which skeleton a character is built on. The two rigs need different mount
/// bones and wildly different scales, so nearly every pose number is keyed on
/// this.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rig {
    /// Quaternius "Ultimate Modular Men" — 62 bones, `Chest` / `Wrist.R`.
    Skeletal,
    /// Kenney "Blocky Characters" — 6 node-animated boxes, `torso` /
    /// `arm-right`.
    Boxy,
}

/// How a piece of gear is being worn this frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Carry {
    /// Across the chest on its strap, body at the hip and neck up over the
    /// opposite shoulder. The **resting** pose — what a guitar looks like when
    /// you are walking, not playing.
    #[default]
    Slung,
    /// Up in the right hand, neck running out along the forearm. The **firing**
    /// pose; `player::aim_guns` then trains the neck on the cursor.
    Aim,
    /// Stowed flat on the back — how the gear a character *isn't* playing
    /// rides along.
    Back,
}

impl Carry {
    pub const ALL: [Carry; 3] = [Carry::Slung, Carry::Aim, Carry::Back];

    pub fn name(self) -> &'static str {
        match self {
            Carry::Slung => "slung (chest)",
            Carry::Aim => "aim (right hand)",
            Carry::Back => "back",
        }
    }

    /// The bone this pose hangs off, by `Name`, for the given rig.
    ///
    /// `Slung` and `Back` both ride the chest/torso — the difference between
    /// them is only where they sit on it, so they share a mount.
    pub fn bone(self, rig: Rig, left: bool) -> &'static str {
        match (self, rig) {
            (Carry::Aim, Rig::Skeletal) => {
                if left {
                    "Wrist.L"
                } else {
                    "Wrist.R"
                }
            }
            (Carry::Aim, Rig::Boxy) => {
                if left {
                    "arm-left"
                } else {
                    "arm-right"
                }
            }
            (_, Rig::Skeletal) => "Chest",
            (_, Rig::Boxy) => "torso",
        }
    }
}

/// A piece of music gear.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Prop {
    /// The rocker's red flying V — also the guitar-*gun*, so it is the one
    /// prop whose local space is load-bearing: neck down +Z, muzzle at the
    /// headstock.
    FlyingV,
    /// The bass: the same silhouette, longer and darker, so the two read apart
    /// at a glance on stage.
    Bass,
    /// A snare on a sling — shallow shell, chrome rims, six lugs.
    Snare,
    /// A small slab synth: dark body, white keybed along the front edge, a lit
    /// screen and three knobs.
    Synth,
    /// A mic on a boom stand, for the singer. Rides the back like a rifle.
    MicStand,
}

impl Prop {
    pub const ALL: [Prop; 5] = [
        Prop::FlyingV,
        Prop::Bass,
        Prop::Snare,
        Prop::Synth,
        Prop::MicStand,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Prop::FlyingV => "flying V",
            Prop::Bass => "bass",
            Prop::Snare => "snare",
            Prop::Synth => "synth",
            Prop::MicStand => "mic stand",
        }
    }

    /// Length along +Z in local units — how the carry poses know how far back
    /// to push a prop so it doesn't clip through the chest.
    pub fn reach(self) -> f32 {
        match self {
            Prop::FlyingV => 2.5,
            Prop::Bass => 3.1,
            Prop::Snare => 0.9,
            Prop::Synth => 1.1,
            Prop::MicStand => 2.6,
        }
    }
}

// ---------------------------------------------------------------------------
// Carry poses
// ---------------------------------------------------------------------------

/// How a prop wants to be oriented when it is worn flat against a body.
///
/// Every prop is modelled neck-forward down +Z, but they do not all *hang* the
/// same way: a guitar lies diagonally across the plane of the back, a snare or
/// a synth lies flat against it face-out, a mic stand rides it like a rifle.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    /// Long-necked, hangs on a strap at a diagonal.
    Strung,
    /// Wide and shallow, lies flat with its face pointing away from the body.
    Flat,
}

impl Prop {
    fn family(self) -> Family {
        match self {
            Prop::FlyingV | Prop::Bass | Prop::MicStand => Family::Strung,
            Prop::Snare | Prop::Synth => Family::Flat,
        }
    }
}

/// Where the chest sits in the mount bone's own space, per rig.
///
/// The Quaternius `Chest` bone is already at chest height, so its offset is
/// nearly zero. The blocky `torso` node's origin is at the **hip** — the torso
/// box runs from y 0.3 to y 1.2 above it — so anything worn on the chest has
/// to be lifted most of a body height. Getting this wrong is what put the
/// first pass of the guitar down by the character's knees.
fn chest_offset(rig: Rig) -> Vec3 {
    match rig {
        Rig::Skeletal => Vec3::new(0.0, -0.02, 0.0),
        Rig::Boxy => Vec3::new(0.0, 0.74, 0.0),
    }
}

/// Half-depth of the torso in the mount bone's space — the body surface a worn
/// prop has to sit against.
fn torso_depth(rig: Rig) -> f32 {
    match rig {
        Rig::Skeletal => 0.15,
        Rig::Boxy => 0.34,
    }
}

impl Prop {
    /// Half the prop's thickness through its face, in local units.
    ///
    /// Worn gear sits at the body surface *plus this*, so it rests flush
    /// against the chest or back instead of half-sunk into it.
    fn half_thickness(self) -> f32 {
        match self {
            Prop::FlyingV | Prop::Bass => 0.19,
            Prop::Snare => 0.17,
            Prop::Synth => 0.10,
            Prop::MicStand => 0.06,
        }
    }
}

/// The local transform that mounts `prop` on the bone [`Carry::bone`] names.
///
/// Both mount bones turned out to share the model's own axes — +Z forward, +Y
/// up — which is what lets one formula serve both rigs, with only the chest
/// offset, the torso depth and the scale keyed on the rig. Verified by mounting
/// a prop at each bone's origin unrotated and looking at where it landed; see
/// ART.md.
///
/// Every prop is modelled lying **face-up**: the neck runs down +Z and the face
/// normal is +Y. So a prop worn on the body needs its face normal turned to
/// point away from the body (±Z) — an X-rotation — before it is rolled to the
/// angle it hangs at. Doing the roll alone leaves the instrument stood on edge
/// against the chest, which is what the first pass did.
pub fn carry_transform(carry: Carry, prop: Prop, rig: Rig, left: bool) -> Transform {
    let s = if left { -1.0 } else { 1.0 };
    let scale = Vec3::splat(prop_scale(prop, rig));
    let chest = chest_offset(rig);
    // Sit the prop's face on the body surface rather than through it.
    let standoff = torso_depth(rig) + prop.half_thickness() * scale.x;

    // Lay the prop against the front of the body: face normal +Y -> +Z (out of
    // the chest), which leaves the neck pointing down -Y. `roll` then swings
    // the neck round to the angle it hangs at, in the plane of the chest.
    let on_front = |roll: f32| Quat::from_rotation_z(roll) * Quat::from_rotation_x(FRAC_PI_2);
    // The same against the back: face normal +Y -> -Z, neck to +Y.
    let on_back = |roll: f32| Quat::from_rotation_z(roll) * Quat::from_rotation_x(-FRAC_PI_2);

    match carry {
        // -- in the hand ----------------------------------------------------
        // Two quarter-turns put the neck down the arm: X sends the neck from
        // +Z to -Y (the direction a limb runs in both rigs' bone space), and Y
        // rolls the instrument upright about its own neck. On the Quaternius
        // rig `player::aim_guns` then overrides the rotation so the neck tracks
        // the cursor; this is the pose the blocky rig and the lab use.
        Carry::Aim => {
            let rot = Quat::from_rotation_x(FRAC_PI_2) * Quat::from_rotation_y(FRAC_PI_2);
            match rig {
                Rig::Skeletal => Transform::from_xyz(s * 0.02, -0.03, 0.05)
                    .with_rotation(rot)
                    .with_scale(scale),
                // The blocky `arm-right` box hangs from its node origin at the
                // shoulder down to y = -1, on the -X side of it.
                Rig::Boxy => Transform::from_xyz(s * -0.2, -0.84, 0.1)
                    .with_rotation(rot)
                    .with_scale(scale),
            }
        }

        // -- slung across the chest ------------------------------------------
        // Body at the playing hip, neck up over the opposite shoulder: the 45°
        // a guitar actually hangs at, and the pose the game rests in.
        Carry::Slung => {
            let (rot, slide) = match prop.family() {
                // Neck up-and-across to the off shoulder.
                Family::Strung => (on_front(s * -2.36), 0.34),
                // A snare or a synth hangs square, face out, at hip height.
                Family::Flat => (on_front(0.0), 0.0),
            };
            let along = rot * Vec3::Z * -slide * prop.reach() * scale.x;
            Transform::from_translation(chest + Vec3::Z * standoff + along + Vec3::X * s * 0.04)
                .with_rotation(rot)
                .with_scale(scale)
        }

        // -- stowed on the back ----------------------------------------------
        // Mirrored behind the shoulder blades, neck up over the playing-side
        // shoulder so it reads from the top-down camera.
        Carry::Back => {
            let (rot, slide) = match prop.family() {
                Family::Strung => (on_back(s * -0.78), 0.34),
                Family::Flat => (on_back(0.0), 0.0),
            };
            let along = rot * Vec3::Z * -slide * prop.reach() * scale.x;
            Transform::from_translation(chest - Vec3::Z * standoff + along)
                .with_rotation(rot)
                .with_scale(scale)
        }
    }
}

/// How big a prop rides on each rig. The blocky characters are chunkier and
/// their bone space is the model's own, so gear has to be bigger on them than
/// on the Quaternius skeleton to read at the same size.
fn prop_scale(prop: Prop, rig: Rig) -> f32 {
    let rig_scale = match rig {
        Rig::Skeletal => 0.20,
        Rig::Boxy => 0.30,
    };
    let prop_scale = match prop {
        Prop::FlyingV | Prop::Bass => 1.0,
        Prop::Snare | Prop::Synth => 1.15,
        Prop::MicStand => 0.9,
    };
    rig_scale * prop_scale
}

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Shared flat-colour materials, built once per spawn.
struct Palette {
    red: Handle<StandardMaterial>,
    black: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    chrome: Handle<StandardMaterial>,
    cream: Handle<StandardMaterial>,
    navy: Handle<StandardMaterial>,
    screen: Handle<StandardMaterial>,
}

fn palette(materials: &mut Assets<StandardMaterial>) -> Palette {
    let flat = |c: Color, rough: f32| StandardMaterial {
        base_color: c,
        perceptual_roughness: rough,
        ..default()
    };
    Palette {
        red: materials.add(StandardMaterial {
            base_color: Color::srgb(0.86, 0.09, 0.12),
            emissive: LinearRgba::rgb(0.45, 0.03, 0.04),
            perceptual_roughness: 0.3,
            ..default()
        }),
        black: materials.add(flat(Color::srgb(0.05, 0.05, 0.06), 0.45)),
        glow: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.4),
            emissive: LinearRgba::rgb(4.0, 2.2, 0.6),
            unlit: true,
            ..default()
        }),
        chrome: materials.add(flat(Color::srgb(0.78, 0.80, 0.84), 0.22)),
        cream: materials.add(flat(Color::srgb(0.93, 0.91, 0.86), 0.55)),
        navy: materials.add(flat(Color::srgb(0.13, 0.15, 0.22), 0.5)),
        screen: materials.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.9, 0.95),
            emissive: LinearRgba::rgb(0.6, 2.4, 2.8),
            unlit: true,
            ..default()
        }),
    }
}

/// Spawn `prop` as a bare, unparented entity with its geometry as children.
///
/// The caller adds whatever the prop means to it — `player::spawn_guitar_gun`
/// adds the `GuitarGun` / `Muzzle` tags, the lab adds nothing — and then
/// parents it with a [`carry_transform`].
pub fn spawn_prop(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    prop: Prop,
) -> Entity {
    let p = palette(materials);
    let root = commands
        .spawn((Transform::default(), Visibility::default()))
        .id();
    commands.entity(root).with_children(|c| match prop {
        Prop::FlyingV => guitar(c, meshes, &p, 1.0, p.red.clone()),
        Prop::Bass => guitar(c, meshes, &p, 1.24, p.navy.clone()),
        Prop::Snare => snare(c, meshes, &p),
        Prop::Synth => synth(c, meshes, &p),
        Prop::MicStand => mic_stand(c, meshes, &p),
    });
    root
}

/// A flying V: forked body behind the origin (-Z), long neck forward (+Z) so
/// from the top-down camera it reads as a barrel. `len` stretches the neck for
/// the bass; `body` is the finish.
///
/// This is the geometry `player::spawn_guitar_gun` has always drawn — lifted
/// here so the bass, and the lab, get it for free.
fn guitar(
    c: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    p: &Palette,
    len: f32,
    body: Handle<StandardMaterial>,
) {
    let prong = meshes.add(Cuboid::new(0.2, 0.36, 0.6));
    for sign in [-1.0_f32, 1.0] {
        c.spawn((
            Mesh3d(prong.clone()),
            MeshMaterial3d(body.clone()),
            Transform::from_xyz(sign * 0.28, 0.0, -0.62)
                .with_rotation(Quat::from_rotation_y(-sign * 0.5)),
        ));
    }
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.44, 0.38, 0.6))),
        MeshMaterial3d(body),
        Transform::from_xyz(0.0, 0.0, -0.35),
    ));
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.15, 0.13, 2.0 * len))),
        MeshMaterial3d(p.black.clone()),
        Transform::from_xyz(0.0, 0.02, 0.95 * len),
    ));
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.28, 0.13, 0.34))),
        MeshMaterial3d(p.black.clone()),
        Transform::from_xyz(0.0, 0.02, 2.1 * len),
    ));
    c.spawn((
        Mesh3d(meshes.add(Sphere::new(0.14))),
        MeshMaterial3d(p.glow.clone()),
        Transform::from_xyz(0.0, 0.02, 2.28 * len),
    ));
}

/// A snare: shallow cream shell lying in the XZ plane (it hangs flat against a
/// back or a hip), chrome hoops top and bottom, six lugs round the rim.
fn snare(c: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, p: &Palette) {
    // Low `resolution` keeps it faceted — a smooth cylinder reads as the wrong
    // art direction next to six-box characters.
    let shell = meshes.add(Cylinder::new(0.62, 0.34).mesh().resolution(12).build());
    let hoop = meshes.add(Cylinder::new(0.66, 0.05).mesh().resolution(12).build());
    c.spawn((
        Mesh3d(shell),
        MeshMaterial3d(p.cream.clone()),
        Transform::default(),
    ));
    for y in [-0.17_f32, 0.17] {
        c.spawn((
            Mesh3d(hoop.clone()),
            MeshMaterial3d(p.chrome.clone()),
            Transform::from_xyz(0.0, y, 0.0),
        ));
    }
    let lug = meshes.add(Cuboid::new(0.09, 0.22, 0.09));
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0;
        c.spawn((
            Mesh3d(lug.clone()),
            MeshMaterial3d(p.chrome.clone()),
            Transform::from_xyz(a.cos() * 0.63, 0.0, a.sin() * 0.63)
                .with_rotation(Quat::from_rotation_y(-a)),
        ));
    }
}

/// A slab synth: dark body, white keybed along the front edge (+Z), a lit
/// screen and three knobs on the top.
fn synth(c: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, p: &Palette) {
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.5, 0.2, 0.62))),
        MeshMaterial3d(p.navy.clone()),
        Transform::default(),
    ));
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.42, 0.12, 0.26))),
        MeshMaterial3d(p.cream.clone()),
        Transform::from_xyz(0.0, 0.02, 0.24),
    ));
    // Black keys, as one dashed strip rather than twelve entities.
    let black_key = meshes.add(Cuboid::new(0.06, 0.06, 0.15));
    for i in 0..7 {
        c.spawn((
            Mesh3d(black_key.clone()),
            MeshMaterial3d(p.black.clone()),
            Transform::from_xyz(-0.6 + i as f32 * 0.2, 0.09, 0.19),
        ));
    }
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.34, 0.03, 0.14))),
        MeshMaterial3d(p.screen.clone()),
        Transform::from_xyz(-0.44, 0.11, -0.14),
    ));
    let knob = meshes.add(Cylinder::new(0.05, 0.07).mesh().resolution(8).build());
    for i in 0..3 {
        c.spawn((
            Mesh3d(knob.clone()),
            MeshMaterial3d(p.chrome.clone()),
            Transform::from_xyz(0.16 + i as f32 * 0.18, 0.12, -0.14),
        ));
    }
}

/// A mic on a boom stand, modelled down +Z so it stows like a rifle: round
/// base, long pole, short boom, capsule with a bright grille.
fn mic_stand(c: &mut ChildSpawnerCommands, meshes: &mut Assets<Mesh>, p: &Palette) {
    c.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.42, 0.08).mesh().resolution(10).build())),
        MeshMaterial3d(p.black.clone()),
        Transform::from_xyz(0.0, 0.0, -0.55)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.09, 0.09, 2.6))),
        MeshMaterial3d(p.black.clone()),
        Transform::from_xyz(0.0, 0.0, 0.75),
    ));
    // The boom, kicked out to one side so the silhouette isn't just a stick.
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.07, 0.07, 0.62))),
        MeshMaterial3d(p.black.clone()),
        Transform::from_xyz(0.16, 0.0, 1.92).with_rotation(Quat::from_rotation_y(-0.5)),
    ));
    c.spawn((
        Mesh3d(meshes.add(Cuboid::new(0.13, 0.13, 0.3))),
        MeshMaterial3d(p.black.clone()),
        Transform::from_xyz(0.36, 0.0, 2.2),
    ));
    c.spawn((
        Mesh3d(meshes.add(Sphere::new(0.11))),
        MeshMaterial3d(p.chrome.clone()),
        Transform::from_xyz(0.36, 0.0, 2.4),
    ));
}
