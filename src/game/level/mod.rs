//! **Where a run happens.**
//!
//! The game used to have exactly one place in it: a thousand lines of town
//! written out longhand, with the road positions, the arena size, the
//! loudspeaker pitches and the five gloom sites all sitting in the same file as
//! the code that spawned them. That is fine for one town and impossible for
//! two — every question a second environment asks ("how big is this place?",
//! "where can a speaker go?", "is that tarmac?") had a single hard-coded
//! answer.
//!
//! So a place is now **data**. A [`Level`] declares its size, its ground, its
//! mission furniture and its palette; [`build`] lays the shared parts down
//! ([`terrain`]) and then calls the level's own `dress` function for whatever
//! makes it that place and nowhere else. The rest of the game stops reading
//! constants and asks [`CurrentLevel`] instead.
//!
//! **Adding an environment** is then: a `Palette`, a `Terrain`, three lists of
//! sites, and a `dress` function that scatters kit models with [`kit`]. See
//! [`town`] for the worked example — and note how much of it is *tables*.

use bevy::prelude::*;

pub mod kit;
pub mod kits;
pub mod net;
pub mod sky;
pub mod terrain;
pub mod town;
pub mod world;

pub use kit::{NatureMats, Solid};
pub use net::{Block, Net, RoadNet};
pub use terrain::{Pad, Perimeter, Road, Run, Surface, Terrain};

/// Ground a loudspeaker pitch keeps clear of buildings, trees and street
/// dressing. A game rule, not a level's choice: a pitch you cannot walk onto is
/// a pitch that breaks the build phase, whatever environment it is in.
pub const SITE_CLEARANCE: f32 = 6.5;

/// **Which places exist.** The one enum a new environment has to be added to.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LevelId {
    /// The tuned mission map: one town, wave loop, five gloom sources.
    #[default]
    Town,
    /// The big linked world — downtown, suburbs, industry, nature, airfield.
    /// A sandbox for now: no wave loop runs in it (see [`world`]).
    World,
}

impl LevelId {
    pub const ALL: &'static [LevelId] = &[LevelId::Town, LevelId::World];

    pub fn level(self) -> &'static Level {
        match self {
            LevelId::Town => &town::TOWN,
            LevelId::World => &world::WORLD,
        }
    }

    /// Look a level up by its `name`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|id| id.level().name.eq_ignore_ascii_case(name))
    }

    /// `SHOOTY_LEVEL=<name>` picks the environment at launch. There is no
    /// picker in the UI yet and the argv grammar belongs to the networking
    /// modes (`coop` / `host` / `join`), so this is how you boot into a level
    /// that is still being built. An unknown name says so and falls back.
    pub fn from_env() -> Self {
        match std::env::var("SHOOTY_LEVEL") {
            Ok(name) => Self::from_name(&name).unwrap_or_else(|| {
                let known: Vec<&str> = Self::ALL.iter().map(|id| id.level().name).collect();
                warn!("unknown SHOOTY_LEVEL `{name}` — known levels: {known:?}");
                Self::default()
            }),
            Err(_) => Self::default(),
        }
    }
}

/// The level this run is being played in. Read it wherever the old code read
/// `ARENA` or `env::SPEAKER_SITES`.
#[derive(Resource, Clone, Copy, Default, Debug)]
pub struct CurrentLevel(pub LevelId);

impl std::ops::Deref for CurrentLevel {
    type Target = Level;
    fn deref(&self) -> &'static Level {
        self.0.level()
    }
}

/// One environment, start to finish.
pub struct Level {
    /// Used by the `--level` switch and the logs. Lower-case, one word.
    pub name: &'static str,
    /// Half-extents of the playfield on the ground (X, Z). Everything that
    /// clamps to the edge of the world clamps to this.
    pub arena: Vec2,
    pub palette: Palette,
    pub terrain: Terrain,
    /// **The road network, generated before anything is built on it.** Given
    /// the level's own rng and arena, returns every road in it. A generator
    /// rather than a list because a district is subdivided into blocks, which
    /// is not something you can type out by hand and keep connected.
    pub roads: fn(&mut rand::rngs::StdRng, Vec2) -> Vec<Road>,
    /// Fixed seed, so a world is the same world every time you load it. A place
    /// you are meant to drive around and learn cannot be reshuffled each run.
    pub seed: u64,
    /// **Where a loudspeaker can go.** Fixed pitches turn the build phase into
    /// a route — which three of these can you reach, hold and link before the
    /// next wave? — and they are a level-design surface, so a map can be tuned
    /// rather than just generated. Adjacent pitches should link (their centres
    /// inside `build::SPEAKER_RADIUS * 1.9`) and distant ones should not, and
    /// none of them should sit on tarmac or inside a landmark's footprint.
    pub speaker_sites: &'static [Vec2],
    /// Where the stage goes — the piece of civic architecture the level builds
    /// for exactly this.
    pub stage_site: Vec2,
    /// Where the five miserable townspeople stand, in `gloom::STEMS` order.
    /// Spread to the corners, off the roads, clear of the pitches.
    pub gloom_sites: &'static [Vec2],
    /// Everything that makes this place itself: buildings, landmarks, street
    /// dressing, scatter. Called after [`terrain::build`].
    pub dress: fn(&mut Ctx<'_, '_, '_>),
}

impl Level {
    /// How far to stretch the distance fog for this level's size.
    ///
    /// The fog numbers in [`grade`](crate::game::grade) were picked against the
    /// town, whose arena half-width is 100 — so the town scales by exactly 1.0
    /// and nothing about it changes. A world three times as wide fogs out three
    /// times further away, which is the difference between seeing the next
    /// district and seeing a wall of sky.
    pub fn fog_scale(&self) -> f32 {
        (self.arena.x / 100.0).max(1.0)
    }
}

/// **What a level is made of, colour-wise.** One entry per material role the
/// shared code needs; a level that wants a snowbound version of the same town
/// changes these numbers and nothing else.
pub struct Palette {
    pub ground: Color,
    pub pavement: Color,
    pub tarmac: Color,
    pub kerb: Color,
    pub markings: Color,
    pub markings_glow: LinearRgba,
    pub crossing: Color,
    pub perimeter: Color,
    pub wood: Color,
    pub wood_pale: Color,
    pub stone: Color,
    pub metal: Color,
    pub terracotta: Color,
    pub water: Color,
    pub water_glow: LinearRgba,
    pub bulb: Color,
    pub bulb_glow: LinearRgba,
    /// Tree leaves. Kept well clear of the bright grass-green rusher cubes so a
    /// tree never reads as an enemy.
    pub foliage: Color,
    pub bark: Color,
}

/// The [`Palette`], realised as material handles. Built once per run.
pub struct Mats {
    pub ground: Handle<StandardMaterial>,
    pub pavement: Handle<StandardMaterial>,
    pub tarmac: Handle<StandardMaterial>,
    pub kerb: Handle<StandardMaterial>,
    pub markings: Handle<StandardMaterial>,
    pub crossing: Handle<StandardMaterial>,
    pub perimeter: Handle<StandardMaterial>,
    pub wood: Handle<StandardMaterial>,
    pub wood_pale: Handle<StandardMaterial>,
    pub stone: Handle<StandardMaterial>,
    pub metal: Handle<StandardMaterial>,
    pub terracotta: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
    pub bulb: Handle<StandardMaterial>,
    pub hydrant: Handle<StandardMaterial>,
    pub nature: NatureMats,
}

impl Mats {
    fn new(p: &Palette, materials: &mut Assets<StandardMaterial>) -> Self {
        let flat = |c: Color, rough: f32| StandardMaterial {
            base_color: c,
            perceptual_roughness: rough,
            ..default()
        };
        Self {
            ground: materials.add(flat(p.ground, 1.0)),
            pavement: materials.add(flat(p.pavement, 0.98)),
            tarmac: materials.add(flat(p.tarmac, 0.85)),
            kerb: materials.add(flat(p.kerb, 0.9)),
            markings: materials.add(StandardMaterial {
                base_color: p.markings,
                emissive: p.markings_glow,
                ..default()
            }),
            // Crossings glow the same faint amount in every environment — they
            // are white paint catching the light, not a mood.
            crossing: materials.add(StandardMaterial {
                base_color: p.crossing,
                emissive: LinearRgba::rgb(0.10, 0.10, 0.10),
                perceptual_roughness: 0.9,
                ..default()
            }),
            perimeter: materials.add(flat(p.perimeter, 1.0)),
            wood: materials.add(flat(p.wood, 0.85)),
            wood_pale: materials.add(flat(p.wood_pale, 0.8)),
            stone: materials.add(flat(p.stone, 0.9)),
            metal: materials.add(StandardMaterial {
                base_color: p.metal,
                perceptual_roughness: 0.5,
                metallic: 0.7,
                ..default()
            }),
            terracotta: materials.add(flat(p.terracotta, 0.9)),
            water: materials.add(StandardMaterial {
                base_color: p.water,
                emissive: p.water_glow,
                perceptual_roughness: 0.15,
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            bulb: materials.add(StandardMaterial {
                base_color: p.bulb,
                emissive: p.bulb_glow,
                ..default()
            }),
            // Fire hydrants are the same red everywhere there are fire
            // hydrants, so this one is not a level's decision.
            hydrant: materials.add(flat(Color::srgb(0.75, 0.12, 0.10), 0.6)),
            nature: NatureMats {
                foliage: materials.add(flat(p.foliage, 0.95)),
                bark: materials.add(flat(p.bark, 0.95)),
            },
        }
    }
}

/// Shared primitive meshes. Unit-diameter cylinders: scaling x/z by N gives a
/// full width of N.
pub struct Prims {
    pub cube: Handle<Mesh>,
    pub sphere_sm: Handle<Mesh>,
    pub cyl_unit: Handle<Mesh>,
    /// An eight-sided cylinder — the plaza apron, the bandstand tiers.
    pub oct: Handle<Mesh>,
}

/// Everything a level's `dress` function needs, in one place: the world to
/// spawn into, the level it is building, and the shared meshes, materials and
/// ground reservations it builds out of.
pub struct Ctx<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub assets: &'a AssetServer,
    pub meshes: &'a mut Assets<Mesh>,
    pub materials: &'a mut Assets<StandardMaterial>,
    pub level: &'static Level,
    /// Seeded from [`Level::seed`], so the same level builds the same way every
    /// time.
    pub rng: rand::rngs::StdRng,
    /// The road network. Built before the terrain is drawn, and consulted by
    /// everything that places anything.
    pub net: Net,
    /// Ground already spoken for. Seeded with the mission sites — the pitches
    /// and the gloom sources — so a level's own scatter can never bury one.
    pub solids: Vec<Solid>,
    pub prims: Prims,
    pub mats: Mats,
}

/// Build `level` into the world. Terrain first, then the level's own dressing.
pub fn build(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    level: &'static Level,
) {
    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::seed_from_u64(level.seed);
    let net = Net {
        roads: (level.roads)(&mut rng, level.arena),
    };

    let prims = Prims {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sphere_sm: meshes.add(Sphere::new(0.3)),
        cyl_unit: meshes.add(Cylinder::new(0.5, 1.0)),
        oct: meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(8).build()),
    };
    let mats = Mats::new(&level.palette, materials);

    let mut solids: Vec<Solid> = level
        .speaker_sites
        .iter()
        .map(|p| (*p, SITE_CLEARANCE))
        .collect();
    // ...and every gloom source's patch of pavement, so the five miserable
    // townspeople are always walkable-up-to rather than boxed in by a terrace.
    solids.extend(
        level
            .gloom_sites
            .iter()
            .map(|p| (*p, crate::game::gloom::SITE_CLEARANCE)),
    );

    let mut ctx = Ctx {
        commands,
        assets,
        meshes,
        materials,
        level,
        rng,
        net,
        solids,
        prims,
        mats,
    };

    terrain::build(&mut ctx);
    (level.dress)(&mut ctx);

    // Hand the finished network to the rest of the game.
    ctx.commands
        .insert_resource(RoadNet(std::mem::take(&mut ctx.net)));
}
