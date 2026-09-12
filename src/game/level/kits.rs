//! **What is in the asset kits**, and what each one should be re-materialled to.
//!
//! These tables describe the *kits*, not any one level: a Kenney office block is
//! 1.29 units tall and wants the city colormap wherever it is standing. So they
//! live here rather than in [`town`](super::town), and a new environment picks
//! the tables it needs instead of retyping them.
//!
//! **Every height here is measured, not assumed.** `tools/measure_models.py`
//! prints these rows straight from the glTF POSITION accessors with node
//! transforms applied. The one time this table was written by hand it took a
//! comment at its word — "feet at y = -1", when the city kit stands at y = 0
//! like everything else — and the whole commercial district came out at half
//! height. Re-measure, don't estimate.

use bevy::prelude::*;

use super::{Ctx, kit};
use crate::game::scale;

/// A building model: glTF path, measured native height, and the mean of its
/// native footprint sides (what the ground-collision radius is derived from).
pub type Building = (&'static str, f32, f32);

/// Commercial blocks: three-to-five-storey office and shop blocks.
pub const CITY: &[Building] = &[
    ("models/city/building-a.glb", 1.29, 0.91),
    ("models/city/building-b.glb", 1.29, 0.96),
    ("models/city/building-c.glb", 0.89, 0.99),
    ("models/city/building-d.glb", 1.29, 0.87),
    ("models/city/building-e.glb", 0.89, 1.33),
    ("models/city/building-f.glb", 1.69, 0.94),
    ("models/city/building-g.glb", 1.69, 0.95),
    ("models/city/building-h.glb", 1.29, 0.95),
];

/// The tall, narrow half of [`CITY`] — the models whose footprint stays
/// sensible when they are scaled up to downtown height.
///
/// `building-c` and `building-e` are short and wide (0.89 tall, up to 1.33
/// across). Fitted to 24 m tall they become 30 m wide, which is a fine shop but
/// sets the plot pitch for the whole district: with them in the table a
/// downtown block held three buildings instead of nine.
pub const CITY_TALL: &[Building] = &[
    ("models/city/building-a.glb", 1.29, 0.91),
    ("models/city/building-b.glb", 1.29, 0.96),
    ("models/city/building-d.glb", 1.29, 0.87),
    ("models/city/building-f.glb", 1.69, 0.94),
    ("models/city/building-g.glb", 1.69, 0.95),
    ("models/city/building-h.glb", 1.29, 0.95),
];

/// The two towers, kept apart from the rest so they can be given landmark
/// height without dragging every office block up with them.
pub const SKYSCRAPERS: &[Building] = &[
    ("models/city/building-skyscraper-a.glb", 2.88, 1.36),
    ("models/city/building-skyscraper-b.glb", 4.48, 1.36),
];

/// Kenney "City Kit (Suburban)" houses — feet at y=0.
pub const HOUSES: &[Building] = &[
    ("models/suburban/building-type-a.glb", 0.83, 1.17),
    ("models/suburban/building-type-b.glb", 1.14, 1.49),
    ("models/suburban/building-type-c.glb", 1.03, 1.16),
    ("models/suburban/building-type-e.glb", 1.14, 1.17),
    ("models/suburban/building-type-f.glb", 1.14, 1.42),
    ("models/suburban/building-type-h.glb", 0.74, 1.11),
    ("models/suburban/building-type-j.glb", 1.04, 1.15),
    ("models/suburban/building-type-k.glb", 1.15, 0.97),
    ("models/suburban/building-type-l.glb", 1.05, 1.03),
    ("models/suburban/building-type-m.glb", 0.74, 1.43),
    ("models/suburban/building-type-n.glb", 1.14, 1.58),
    ("models/suburban/building-type-q.glb", 0.92, 1.07),
    ("models/suburban/building-type-r.glb", 1.14, 1.03),
    ("models/suburban/building-type-s.glb", 1.14, 1.25),
    ("models/suburban/building-type-u.glb", 1.14, 1.26),
];

/// Kenney "City Kit (Industrial)" — sheds, warehouses and plant. Wider than
/// they are tall, which is what makes an industrial estate read differently
/// from a suburb at a glance.
pub const INDUSTRIAL: &[Building] = &[
    ("models/industrial/building-a.glb", 1.47, 1.66),
    ("models/industrial/building-b.glb", 1.47, 1.67),
    ("models/industrial/building-c.glb", 1.25, 2.00),
    ("models/industrial/building-e.glb", 1.65, 1.49),
    ("models/industrial/building-f.glb", 1.92, 1.54),
    ("models/industrial/building-g.glb", 1.28, 1.48),
    ("models/industrial/building-l.glb", 1.92, 1.98),
    ("models/industrial/building-m.glb", 1.52, 1.51),
    ("models/industrial/building-q.glb", 0.88, 1.96),
    ("models/industrial/building-r.glb", 1.39, 1.88),
    ("models/industrial/building-t.glb", 1.01, 1.56),
];

/// The low, wide industrial sheds — big enough inside to be a hangar, which is
/// what the airfield uses them for.
pub const SHEDS: &[Building] = &[
    ("models/industrial/building-h.glb", 0.73, 1.32),
    ("models/industrial/building-p.glb", 0.71, 1.34),
    ("models/industrial/building-s.glb", 0.84, 1.52),
    ("models/industrial/building-q.glb", 0.88, 1.96),
];

/// Industrial dressing: (path, native height, size in metres, collision radius
/// at that size — `None` for something you can walk over).
pub type Plant = (&'static str, f32, f32, Option<f32>);

pub const PLANT: &[Plant] = &[
    ("models/industrial/chimney-large.glb", 1.70, 22.0, Some(4.0)),
    (
        "models/industrial/chimney-medium.glb",
        1.92,
        26.0,
        Some(2.2),
    ),
    ("models/industrial/chimney-small.glb", 0.75, 12.0, Some(1.4)),
    (
        "models/industrial/detail-tank-large.glb",
        0.96,
        9.0,
        Some(6.0),
    ),
    ("models/industrial/detail-tank.glb", 0.42, 4.5, Some(3.2)),
    ("models/industrial/water-tower.glb", 2.14, 19.0, Some(3.0)),
];

/// Shipping containers — the one prop that says "yard" on its own.
pub const CONTAINERS: &[&str] = &[
    "models/industrial/shipping-container-a.glb",
    "models/industrial/shipping-container-b.glb",
    "models/industrial/shipping-container-c.glb",
];
/// Native height of every container variant (they are the same box).
pub const CONTAINER_NATIVE: f32 = 0.35;

pub const SOLAR: &[(&str, f32)] = &[
    ("models/industrial/solar-panel-landscape-group.glb", 0.26),
    ("models/industrial/solar-panel-portrait-group.glb", 0.41),
];

pub const WINDMILLS: &[(&str, f32)] = &[
    ("models/industrial/windmill.glb", 2.31),
    ("models/industrial/windmill-low.glb", 1.79),
];

/// **Aircraft** — (path, the native dimension the size refers to, size in
/// metres, the model's own lowest native `y`).
///
/// These are not Kenney kit models: they come from poly.pizza under **CC-BY**,
/// so they carry an attribution obligation the rest of the project does not —
/// see `assets/models/aircraft/LICENSE-cc-by.txt`. They also keep their own
/// materials (spawn them with [`kit::model`], not [`kit::prop`]) and none of
/// them sits at y = 0, hence the fourth field.
pub type Aircraft = (&'static str, f32, f32, f32);

/// "plane 2" by Jake Blakeley — the one meant to be flyable.
pub const PLANE: Aircraft = ("models/aircraft/plane.glb", 1.17, scale::PLANE_SPAN, -0.17);
/// "Small Airplane" by Vojtěch Balák — parked dressing on the apron.
pub const PLANE_SMALL: Aircraft = (
    "models/aircraft/plane-small.glb",
    11.07,
    scale::PLANE_SPAN,
    -1.24,
);
/// "Blimp" by Poly by Google, drifting over the world for no reason at all.
pub const BLIMP: Aircraft = (
    "models/aircraft/blimp.glb",
    1194.89,
    scale::BLIMP_LENGTH,
    -309.00,
);

/// **Quaternius farm buildings** — (path, native height, the model's lowest
/// native `y`, collision radius in metres).
///
/// CC0, but not a Kenney colormap kit: they ship OBJ only, so they were
/// converted to glTF (`trimesh`, see README), and they keep their own
/// materials — spawn them with [`kit::model`].
///
/// **They are authored in metres**, which nothing else here is: a barn measures
/// 6.01 units tall and is a 6 m barn. So they scale by [`scale::U_PER_M`] flat,
/// and the height column is the real height rather than something to divide by.
pub type FarmModel = (&'static str, f32, f32, f32);

pub const FARMSTEAD: &[FarmModel] = &[
    ("models/farm/barn.glb", 6.01, 0.00, 4.4),
    ("models/farm/bigbarn.glb", 7.89, 0.00, 4.4),
    ("models/farm/openbarn.glb", 4.74, 0.00, 3.4),
    ("models/farm/smallbarn.glb", 4.96, 0.00, 3.6),
    ("models/farm/silo.glb", 9.07, -0.05, 2.0),
    ("models/farm/silo-house.glb", 9.07, -0.05, 2.8),
    ("models/farm/chickencoop.glb", 1.85, 0.00, 1.4),
    ("models/farm/watertower.glb", 8.43, -0.07, 1.4),
    ("models/farm/windmill.glb", 11.20, 0.00, 1.6),
    ("models/farm/towerwindmill.glb", 11.41, -0.03, 2.4),
    ("models/farm/well.glb", 2.15, -0.01, 1.0),
];

/// Field fencing from the same pack — one length is ~5.9 m long.
pub const FARM_FENCE: &[FarmModel] = &[
    ("models/farm/fence.glb", 1.10, -0.01, 0.0),
    ("models/farm/fence2.glb", 1.17, -0.01, 0.0),
];
/// Length of one fence panel, in metres.
pub const FARM_FENCE_RUN: f32 = 5.89;

/// One scale for the whole car kit, set so a sedan's roof sits at
/// [`scale::CAR`] — the kit's own relative sizes (a van taller than a sports
/// car) then survive intact.
///
/// Cars are fitted by **height**, not length. Kenney's cars are stubby for
/// their height, so matching their length instead would put a parked sedan's
/// roof above the band's heads: a hero has to be the tallest thing on the
/// street or they get lost behind the traffic.
pub const CAR_SCALE: f32 = fit_const(1.30, scale::CAR);

/// `scale::fit` in a `const` context.
pub const fn fit_const(native: f32, metres: f32) -> f32 {
    metres * scale::U_PER_M / native
}

/// Kenney "Car Kit" — feet at y=0, ~1.5 wide × ~2.6 long, nose down +Z.
pub const CARS: &[&str] = &[
    "models/cars/sedan.glb",
    "models/cars/sedan-sports.glb",
    "models/cars/suv.glb",
    "models/cars/suv-luxury.glb",
    "models/cars/hatchback-sports.glb",
    "models/cars/van.glb",
    "models/cars/taxi.glb",
    "models/cars/delivery.glb",
    "models/cars/truck.glb",
    "models/cars/police.glb",
];

/// (glTF path, native height, trunk collision radius at native scale). Nature
/// models sit with feet at y=0 and bake their colours into the material, so
/// they need no colormap wiring.
pub const TREES: &[(&str, f32, f32)] = &[
    ("models/nature/tree_default.glb", 1.71, 0.20),
    ("models/nature/tree_oak.glb", 1.23, 0.22),
    ("models/nature/tree_fat.glb", 1.15, 0.24),
    ("models/nature/tree_detailed.glb", 1.33, 0.20),
    ("models/nature/tree_pineRoundC.glb", 1.25, 0.16),
    ("models/nature/tree_blocks.glb", 1.19, 0.18),
];

/// Conifers, for the wooded belts — a pine wood reads differently from a row of
/// street trees, which is most of what makes a nature district a district.
pub const CONIFERS: &[(&str, f32, f32)] = &[
    ("models/nature/tree_pineRoundC.glb", 1.25, 0.16),
    ("models/nature/tree_pineTallA.glb", 1.83, 0.14),
    ("models/nature/tree_pineTallB.glb", 1.83, 0.14),
    ("models/nature/tree_pineSmallB.glb", 1.13, 0.14),
    ("models/nature/tree_thin.glb", 1.61, 0.12),
];

/// (path, **widest native dimension**). Unlike [`TREES`], which are measured by
/// height, these are measured across — they are flat, wide models, and asking
/// for "a 1.4 m rock" by height gave a 5.5 m boulder.
pub const SHRUBS: &[(&str, f32)] = &[
    ("models/nature/plant_bushLarge.glb", 0.37),
    ("models/nature/plant_bushDetailed.glb", 0.60),
    ("models/nature/rock_largeA.glb", 1.02),
    ("models/nature/rock_tallC.glb", 0.78),
];

/// Measured native heights of the street-dressing models, so every call site
/// can ask for a size in metres (see [`scale`]) instead of a bare multiplier.
pub mod native {
    pub const LAMP: f32 = 0.67;
    /// The bench, measured on all three axes — it is the one prop fitted
    /// per-axis, because its proportions are not a bench's. See
    /// [`bench_scale`](super::bench_scale).
    pub const BENCH: bevy::math::Vec3 = bevy::math::Vec3::new(0.40, 0.47, 0.20);
    pub const BIN: f32 = 0.43;
    pub const SIGN: f32 = 0.49;
    pub const FENCE_1X4: f32 = 0.27;
    pub const PLANTER: f32 = 0.18;
}

/// The per-axis scale that turns Kenney's furniture bench into something a
/// person could actually sit on.
pub fn bench_scale() -> Vec3 {
    scale::fit3(native::BENCH, scale::BENCH)
}

/// **The kit materials a level uses.** The stock glTF materials on the Kenney
/// kits render untextured white here, so every kit mesh is re-pointed at one of
/// these shared, point-sampled `colormap.png` materials.
///
/// The tinted sets are lists rather than one handle because those kits are meant
/// to vary: a street of identically-coloured houses reads as a texture bug.
pub struct Kits {
    pub commercial: Vec<Handle<StandardMaterial>>,
    pub house: Vec<Handle<StandardMaterial>>,
    pub industrial: Vec<Handle<StandardMaterial>>,
    pub car: Handle<StandardMaterial>,
    /// Lamp posts, signs, fences — anything off the roads kit.
    pub street: Handle<StandardMaterial>,
}

impl Kits {
    pub fn load(ctx: &mut Ctx) -> Self {
        let city = kit::colormap(ctx.assets, "models/city/Textures/colormap.png");
        let commercial = tinted(
            ctx,
            &city,
            &[
                Vec3::new(0.38, 0.58, 1.10),
                Vec3::new(1.15, 0.70, 0.30),
                Vec3::new(0.32, 0.80, 1.05),
                Vec3::new(0.95, 0.93, 0.82),
                Vec3::new(0.48, 0.44, 0.92),
            ],
            0.58,
            0.1,
            0.13,
        );

        let suburban = kit::colormap(ctx.assets, "models/suburban/Textures/colormap.png");
        let house = tinted(
            ctx,
            &suburban,
            &[
                Vec3::new(1.05, 0.98, 0.86), // cream
                Vec3::new(0.80, 0.90, 1.05), // pale blue
                Vec3::new(0.86, 1.00, 0.84), // sage
                Vec3::new(1.10, 0.82, 0.70), // terracotta
                Vec3::new(0.95, 0.92, 0.95), // off-white
            ],
            0.8,
            0.0,
            0.0,
        );

        // Industry is drabber than the town on purpose — the estate should read
        // as somewhere that works rather than somewhere that lives.
        let ind_map = kit::colormap(ctx.assets, "models/industrial/Textures/colormap.png");
        let industrial = tinted(
            ctx,
            &ind_map,
            &[
                Vec3::new(0.92, 0.93, 0.95),
                Vec3::new(0.86, 0.88, 0.86),
                Vec3::new(1.00, 0.94, 0.82),
                Vec3::new(0.80, 0.84, 0.90),
            ],
            0.7,
            0.2,
            0.0,
        );

        let cars = kit::colormap(ctx.assets, "models/cars/Textures/colormap.png");
        let car = ctx.materials.add(StandardMaterial {
            base_color: Color::srgb(1.05, 1.05, 1.05),
            base_color_texture: Some(cars),
            perceptual_roughness: 0.42,
            metallic: 0.25,
            ..default()
        });

        let roads = kit::colormap(ctx.assets, "models/city-roads/Textures/colormap.png");
        let street = ctx.materials.add(StandardMaterial {
            base_color: Color::srgb(0.90, 0.90, 0.92),
            base_color_texture: Some(roads),
            perceptual_roughness: 0.55,
            metallic: 0.35,
            ..default()
        });

        Self {
            commercial,
            house,
            industrial,
            car,
            street,
        }
    }
}

/// One colormap material per tint.
fn tinted(
    ctx: &mut Ctx,
    map: &Handle<Image>,
    tints: &[Vec3],
    roughness: f32,
    metallic: f32,
    glow: f32,
) -> Vec<Handle<StandardMaterial>> {
    tints
        .iter()
        .map(|t| {
            ctx.materials.add(StandardMaterial {
                base_color: Color::srgb(t.x, t.y, t.z),
                base_color_texture: Some(map.clone()),
                emissive: LinearRgba::rgb(t.x * glow, t.y * glow, t.z * glow),
                perceptual_roughness: roughness,
                metallic,
                ..default()
            })
        })
        .collect()
}
