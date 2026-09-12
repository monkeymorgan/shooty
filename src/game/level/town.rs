//! **The town** — the level the game shipped with, and the worked example of
//! what a [`Level`](super::Level) looks like.
//!
//! Grass, a road grid with pavements and zebra crossings, Kenney low-poly
//! buildings grouped onto the blocks (commercial towers in the middle,
//! suburban houses with gardens toward the edge), a central plaza with a
//! fountain and a bandstand, a little park with a pond, and street dressing —
//! lamp posts, benches, bins, signs and parked cars. All CC0 (see the LICENSE
//! files beside the model folders).
//!
//! Almost everything specific to this place is a table at the top of the file:
//! which models, how tall they are in metres, where the roads run, where the
//! landmarks sit. [`dress`] is the code that reads those tables — and a second
//! environment reuses the code, not the tables.

use bevy::prelude::*;
use rand::Rng;

use super::kits::{self, Kits, native};
use super::{Ctx, Level, Palette, Perimeter, Road, Run, Surface, Terrain, kit};
use crate::game::scale::{self, fit};
use crate::game::{Obstacle, RunEntity, ground};

/// Half-width of a road lane-pair.
const ROAD_HW: f32 = 5.0;
/// Half-width of a pavement strip along a kerb.
const WALK_HW: f32 = 1.9;
/// Road centre-lines: vertical roads at these `x`, horizontal roads at these
/// plane-`y` (world `-z`). A loose 4×4 block grid with the player starting on
/// the central crossroads.
const ROAD_XS: &[f32] = &[-58.0, 0.0, 58.0];
const ROAD_YS: &[f32] = &[-40.0, 0.0, 40.0];

/// The grid, as runs. Each one overshoots the arena by 40 units so the road
/// carries on into the fog rather than stopping dead at the hedge.
const STREET: Surface = Surface::Street {
    half_width: ROAD_HW,
    walk_half_width: WALK_HW,
};
const fn ns(at: f32) -> Road {
    Road {
        run: Run::NorthSouth,
        at,
        from: -112.0,
        to: 112.0,
        surface: STREET,
    }
}
const fn ew(at: f32) -> Road {
    Road {
        run: Run::EastWest,
        at,
        from: -140.0,
        to: 140.0,
        surface: STREET,
    }
}
/// The town's network is fixed — it is a tuned mission map, and moving its
/// roads would move the loudspeaker pitches out from under it.
fn roads(_rng: &mut rand::rngs::StdRng, _arena: Vec2) -> Vec<Road> {
    vec![
        ns(ROAD_XS[0]),
        ns(ROAD_XS[1]),
        ns(ROAD_XS[2]),
        ew(ROAD_YS[0]),
        ew(ROAD_YS[1]),
        ew(ROAD_YS[2]),
    ]
}

/// Distance from a road centre-line to the first buildable ground on a block.
const INSET_ROAD: f32 = ROAD_HW + 4.2;
/// Distance from the arena edge (the hedge) to buildable ground.
const INSET_EDGE: f32 = 4.0;

/// Town landmarks, kept clear of buildings and the spawn crossroads.
const PLAZA: Vec2 = Vec2::new(29.0, 20.0);
const BANDSTAND: Vec2 = Vec2::new(15.0, 25.5);
const PARK: Vec2 = Vec2::new(-30.0, -20.0);

/// Ten marked pitches around the town — a painted pad on a corner, a patch of
/// green, the far end of a car park. See [`Level::speaker_sites`](super::Level).
const SPEAKER_SITES: &[Vec2] = &[
    Vec2::new(13.0, -15.0),
    Vec2::new(36.0, 10.0),
    Vec2::new(44.0, 30.0),
    Vec2::new(16.0, 52.0),
    Vec2::new(-13.0, 32.0),
    Vec2::new(-15.0, -13.0),
    Vec2::new(-42.0, -30.0),
    Vec2::new(-44.0, 14.0),
    Vec2::new(68.0, 16.0),
    Vec2::new(-44.0, 50.0),
];

/// The five gloom sources, in `gloom::STEMS` order.
///
/// NB: kept well clear of [`PARK`], whose ring of eleven trees is placed
/// directly rather than through the `solids` reservation list, so it is the one
/// piece of town dressing a reserved site does not repel. The third pick here
/// was first tried at (-46,-14) and the character came out invisible behind
/// three of them.
const GLOOM_SITES: &[Vec2] = &[
    Vec2::new(-50.0, -26.0),
    Vec2::new(46.0, -26.0),
    Vec2::new(-38.0, 28.0),
    Vec2::new(30.0, 56.0),
    Vec2::new(-14.0, -56.0),
];

pub static TOWN: Level = Level {
    name: "town",
    arena: Vec2::new(100.0, 72.0),
    palette: Palette {
        ground: Color::srgb(0.40, 0.49, 0.29),
        pavement: Color::srgb(0.44, 0.45, 0.43),
        tarmac: Color::srgb(0.16, 0.16, 0.18),
        kerb: Color::srgb(0.62, 0.63, 0.60),
        markings: Color::srgb(0.90, 0.82, 0.35),
        markings_glow: LinearRgba::rgb(0.25, 0.20, 0.05),
        crossing: Color::srgb(0.88, 0.88, 0.86),
        perimeter: Color::srgb(0.24, 0.40, 0.22),
        wood: Color::srgb(0.52, 0.36, 0.22),
        wood_pale: Color::srgb(0.74, 0.58, 0.38),
        stone: Color::srgb(0.66, 0.66, 0.63),
        metal: Color::srgb(0.28, 0.30, 0.33),
        terracotta: Color::srgb(0.62, 0.32, 0.22),
        water: Color::srgba(0.20, 0.52, 0.72, 0.72),
        water_glow: LinearRgba::rgb(0.05, 0.22, 0.34),
        bulb: Color::srgb(1.0, 0.95, 0.82),
        bulb_glow: LinearRgba::rgb(9.0, 6.6, 3.2),
        foliage: Color::srgb(0.13, 0.32, 0.17),
        bark: Color::srgb(0.36, 0.25, 0.16),
    },
    roads,
    seed: 0x7000,
    terrain: Terrain {
        pads: &[],
        crossings: true,
        perimeter: Some(Perimeter {
            height: 2.6,
            thickness: 2.0,
        }),
    },
    speaker_sites: SPEAKER_SITES,
    stage_site: BANDSTAND,
    gloom_sites: GLOOM_SITES,
    dress,
};

/// Everything above the tarmac: buildings on the blocks, street furniture and
/// parked cars down the kerbs, the three landmarks, and a scatter of trees over
/// what is left.
fn dress(ctx: &mut Ctx) {
    let kits = Kits::load(ctx);
    let arena = ctx.level.arena;

    // Seed the landmark footprints, so the building scatter leaves them alone.
    ctx.solids
        .extend([(PLAZA, 15.0), (BANDSTAND, 6.5), (PARK, 15.0)]);

    blocks(ctx, &kits);
    street_dressing(ctx, &kits);
    parked_cars(ctx, &kits);
    plaza(ctx);
    bandstand(ctx);
    park(ctx);

    // ---- Street trees along both kerbs of every road (behind the pavement).
    for &x in ROAD_XS {
        for side in [-1.0f32, 1.0] {
            let tx = x + side * (ROAD_HW + WALK_HW * 2.0 + 1.4);
            let mut ty = -arena.y + 10.0;
            while ty < arena.y - 10.0 {
                if !ROAD_YS.iter().any(|&ry| (ty - ry).abs() < ROAD_HW + 6.0)
                    && Vec2::new(tx, ty).distance(PLAZA) > 15.0
                {
                    let model = kits::TREES[ctx.rng.gen_range(0..3)];
                    let metres = ctx.rng.gen_range(scale::TREE.0..scale::TREE.1);
                    kit::nature(
                        ctx.commands,
                        ctx.assets,
                        &ctx.mats.nature,
                        model,
                        Vec2::new(tx, ty),
                        metres,
                        &mut ctx.rng,
                        true,
                    );
                }
                ty += ctx.rng.gen_range(13.0..18.0);
            }
        }
    }

    scatter(ctx);
}

/// Buildings, grouped onto the 16 grid blocks: towers in the four central
/// blocks, houses everywhere else.
fn blocks(ctx: &mut Ctx, kits: &Kits) {
    let arena = ctx.level.arena;
    let bx = [-arena.x, ROAD_XS[0], ROAD_XS[1], ROAD_XS[2], arena.x];
    let by = [-arena.y, ROAD_YS[0], ROAD_YS[1], ROAD_YS[2], arena.y];
    for i in 0..4 {
        for j in 0..4 {
            let min = Vec2::new(
                bx[i] + if i == 0 { INSET_EDGE } else { INSET_ROAD },
                by[j] + if j == 0 { INSET_EDGE } else { INSET_ROAD },
            );
            let max = Vec2::new(
                bx[i + 1] - if i == 3 { INSET_EDGE } else { INSET_ROAD },
                by[j + 1] - if j == 3 { INSET_EDGE } else { INSET_ROAD },
            );
            let centre = (min + max) * 0.5;
            // The two central blocks that aren't the plaza / park get towers.
            let central = (i == 1 || i == 2) && (j == 1 || j == 2);
            if central && centre.distance(PLAZA) < 24.0 {
                continue; // plaza block
            }
            if central && centre.distance(PARK) < 24.0 {
                continue; // park block
            }

            if central {
                place_commercial(ctx, kits, min, max);
            } else {
                place_suburban(ctx, kits, min, max);
            }
        }
    }
}

/// Commercial block: 2–3 tinted Kenney city towers on a small internal grid,
/// one of them occasionally a skyscraper landmark.
fn place_commercial(ctx: &mut Ctx, kits: &Kits, min: Vec2, max: Vec2) {
    let span = max - min;
    let cols = if span.x > span.y { 2 } else { 1 };
    let rows = if span.x > span.y { 1 } else { 2 };
    for cx in 0..cols {
        for cy in 0..rows {
            let cell = Vec2::new(
                min.x + span.x * (cx as f32 + 0.5) / cols as f32,
                min.y + span.y * (cy as f32 + 0.5) / rows as f32,
            );
            let p = cell + Vec2::new(ctx.rng.gen_range(-2.5..2.5), ctx.rng.gen_range(-2.5..2.5));
            if p.length() < 13.0 || ctx.net.on_road(p) {
                continue;
            }
            // One tower in six is a landmark; the rest are ordinary blocks.
            let tall = ctx.rng.gen_bool(0.16);
            let table = if tall { kits::SKYSCRAPERS } else { kits::CITY };
            let (lo, hi) = if tall {
                scale::SKYSCRAPER
            } else {
                scale::TOWER
            };
            let (model, native_h, native_plan) = table[ctx.rng.gen_range(0..table.len())];
            let s = fit(native_h, ctx.rng.gen_range(lo..hi));
            let radius = s * native_plan * 0.5;
            if ctx
                .solids
                .iter()
                .any(|(q, r)| p.distance(*q) < r + radius + 3.0)
            {
                continue;
            }
            ctx.solids.push((p, radius));
            let yaw = ctx.rng.gen_range(0..4) as f32 * std::f32::consts::FRAC_PI_2;
            let mat = kits.commercial[ctx.rng.gen_range(0..kits.commercial.len())].clone();
            kit::building(ctx.commands, ctx.assets, model, p, s, 0.0, yaw, radius, mat);
        }
    }
}

/// Suburban block: a row of small Kenney houses set back off the street, each
/// with a low fence and a garden tree.
fn place_suburban(ctx: &mut Ctx, kits: &Kits, min: Vec2, max: Vec2) {
    let span = max - min;
    let along_x = span.x >= span.y;
    let run = if along_x { span.x } else { span.y };
    // A house is a real house now — around 15 u across the plot — so a block
    // takes one or two of them, not three.
    let n = ((run / 24.0).floor() as i32).clamp(1, 2);
    for k in 0..n {
        let f = (k as f32 + 0.5) / n as f32;
        let base = if along_x {
            Vec2::new(min.x + span.x * f, (min.y + max.y) * 0.5)
        } else {
            Vec2::new((min.x + max.x) * 0.5, min.y + span.y * f)
        };
        let p = base + Vec2::new(ctx.rng.gen_range(-1.5..1.5), ctx.rng.gen_range(-1.5..1.5));
        if p.length() < 14.0 || ctx.net.on_road(p) {
            continue;
        }
        let (model, native_h, native_plan) = kits::HOUSES[ctx.rng.gen_range(0..kits::HOUSES.len())];
        let s = fit(native_h, ctx.rng.gen_range(scale::HOUSE.0..scale::HOUSE.1));
        let radius = s * native_plan * 0.5;
        // Keep the plot inside its block now that a house is wide enough to
        // hang over the kerb.
        let p = p.clamp(
            min + Vec2::splat(radius),
            (max - Vec2::splat(radius)).max(min),
        );
        if ctx.net.on_road(p) {
            continue;
        }
        if ctx
            .solids
            .iter()
            .any(|(q, r)| p.distance(*q) < r + radius + 3.0)
        {
            continue;
        }
        ctx.solids.push((p, radius));
        let yaw = ctx.rng.gen_range(0..4) as f32 * std::f32::consts::FRAC_PI_2;
        let mat = kits.house[ctx.rng.gen_range(0..kits.house.len())].clone();
        kit::building(ctx.commands, ctx.assets, model, p, s, 0.0, yaw, radius, mat);

        // a garden tree beside the house
        if ctx.rng.gen_bool(0.55) {
            let off = Vec2::new(ctx.rng.gen_range(-1.0..1.0), ctx.rng.gen_range(-1.0..1.0))
                .normalize_or_zero()
                * (radius * 1.5 + 2.6);
            let model = kits::TREES[ctx.rng.gen_range(0..kits::TREES.len())];
            let metres = ctx.rng.gen_range(scale::TREE.0..scale::TREE.1);
            kit::nature(
                ctx.commands,
                ctx.assets,
                &ctx.mats.nature,
                model,
                p + off,
                metres,
                &mut ctx.rng,
                true,
            );
        }
        // a low fence line toward the street
        if ctx.rng.gen_bool(0.6) {
            let (fp, fyaw) = if along_x {
                (
                    p + Vec2::new(0.0, (radius + 2.8) * if p.y > base.y { 1.0 } else { -1.0 }),
                    0.0,
                )
            } else {
                (
                    p + Vec2::new((radius + 2.8) * if p.x > base.x { 1.0 } else { -1.0 }, 0.0),
                    std::f32::consts::FRAC_PI_2,
                )
            };
            kit::prop(
                ctx.commands,
                ctx.assets,
                "models/suburban/fence-1x4.glb",
                fp,
                0.0,
                fyaw,
                fit(native::FENCE_1X4, scale::FENCE),
                kits.street.clone(),
                None,
            );
        }
    }
}

/// One piece of street furniture on a pavement, facing the road.
fn furniture_at(ctx: &mut Ctx, kits: &Kits, p: Vec2, face: f32) {
    match ctx.rng.gen_range(0..10) {
        0..=3 => {
            // lamp post + emissive bulb, arm reaching toward the road
            let lamp = fit(native::LAMP, scale::LAMP_POST);
            kit::prop(
                ctx.commands,
                ctx.assets,
                "models/city-roads/light-curved.glb",
                p,
                0.0,
                face,
                lamp,
                kits.street.clone(),
                None,
            );
            // Bulb rides the lamp head: both offsets are the model's own
            // proportions times the fitted scale.
            let arm = Vec2::new(-face.sin(), -face.cos()) * (0.151 * lamp);
            let (mesh, mat) = (ctx.prims.sphere_sm.clone(), ctx.mats.bulb.clone());
            ctx.commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_translation(ground(p + arm, 0.618 * lamp)),
                RunEntity,
            ));
        }
        4..=5 => {
            kit::prop_scaled(
                ctx.commands,
                ctx.assets,
                "models/furniture/bench.glb",
                p,
                0.0,
                face + std::f32::consts::PI,
                kits::bench_scale(),
                ctx.mats.wood_pale.clone(),
                None,
            );
        }
        6 => {
            kit::prop(
                ctx.commands,
                ctx.assets,
                "models/furniture/trashcan.glb",
                p,
                0.0,
                face,
                fit(native::BIN, scale::BIN),
                ctx.mats.metal.clone(),
                None,
            );
        }
        7 => {
            // fire hydrant from primitives
            let (cyl, sphere) = (ctx.prims.cyl_unit.clone(), ctx.prims.sphere_sm.clone());
            let mat = ctx.mats.hydrant.clone();
            ctx.commands
                .spawn((
                    Transform::from_translation(ground(p, 0.0)),
                    Visibility::default(),
                    RunEntity,
                ))
                .with_children(|c| {
                    c.spawn((
                        Mesh3d(cyl),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_xyz(0.0, 0.35, 0.0).with_scale(Vec3::new(0.34, 0.7, 0.34)),
                    ));
                    c.spawn((
                        Mesh3d(sphere),
                        MeshMaterial3d(mat),
                        Transform::from_xyz(0.0, 0.72, 0.0).with_scale(Vec3::splat(0.6)),
                    ));
                });
        }
        _ => {
            let sign = [
                "models/city-roads/road-sign-warning.glb",
                "models/city-roads/road-sign-street.glb",
                "models/city-roads/road-sign-stop.glb",
            ][ctx.rng.gen_range(0..3)];
            kit::prop(
                ctx.commands,
                ctx.assets,
                sign,
                p,
                0.0,
                face,
                fit(native::SIGN, scale::ROAD_SIGN),
                kits.street.clone(),
                None,
            );
        }
    }
}

/// Street furniture down every pavement, spaced out and skipping the
/// intersections.
fn street_dressing(ctx: &mut Ctx, kits: &Kits) {
    let arena = ctx.level.arena;
    for &x in ROAD_XS {
        for side in [-1.0f32, 1.0] {
            let px = x + side * (ROAD_HW + WALK_HW + 0.3);
            let mut t = -arena.y + 14.0;
            while t < arena.y - 14.0 {
                if !ROAD_YS.iter().any(|&ry| (t - ry).abs() < ROAD_HW + 7.0) {
                    let face = if side > 0.0 {
                        -std::f32::consts::FRAC_PI_2
                    } else {
                        std::f32::consts::FRAC_PI_2
                    };
                    furniture_at(ctx, kits, Vec2::new(px, t), face);
                }
                t += ctx.rng.gen_range(15.0..21.0);
            }
        }
    }
    for &y in ROAD_YS {
        for side in [-1.0f32, 1.0] {
            let py = y + side * (ROAD_HW + WALK_HW + 0.3);
            let mut t = -arena.x + 16.0;
            while t < arena.x - 16.0 {
                if !ROAD_XS.iter().any(|&rx| (t - rx).abs() < ROAD_HW + 9.0) {
                    let face = if side > 0.0 {
                        std::f32::consts::PI
                    } else {
                        0.0
                    };
                    furniture_at(ctx, kits, Vec2::new(t, py), face);
                }
                t += ctx.rng.gen_range(16.0..22.0);
            }
        }
    }
}

/// Parked cars along the kerbs — capped, because every one of them is a
/// drivable entity.
fn parked_cars(ctx: &mut Ctx, kits: &Kits) {
    let arena = ctx.level.arena;
    let mut cars = 0;
    for &x in ROAD_XS {
        for side in [-1.0f32, 1.0] {
            let lane_x = x + side * (ROAD_HW - 1.4);
            let mut t = -arena.y + 20.0 + ctx.rng.gen_range(0.0..10.0);
            while t < arena.y - 20.0 && cars < 7 {
                if !ROAD_YS.iter().any(|&ry| (t - ry).abs() < ROAD_HW + 8.0)
                    && ctx.rng.gen_bool(0.4)
                {
                    let model = kits::CARS[ctx.rng.gen_range(0..kits::CARS.len())];
                    let yaw = ctx.rng.gen_range(-0.06..0.06);
                    let car = kit::prop(
                        ctx.commands,
                        ctx.assets,
                        model,
                        Vec2::new(lane_x, t),
                        0.0,
                        yaw,
                        kits::CAR_SCALE,
                        kits.car.clone(),
                        Some(1.9),
                    );
                    ctx.commands
                        .entity(car)
                        .insert(crate::game::drive::Car::parked());
                    cars += 1;
                }
                t += ctx.rng.gen_range(15.0..24.0);
            }
        }
    }
    for &y in ROAD_YS {
        for side in [-1.0f32, 1.0] {
            let lane_y = y + side * (ROAD_HW - 1.4);
            let mut t = -arena.x + 24.0 + ctx.rng.gen_range(0.0..12.0);
            while t < arena.x - 24.0 && cars < 15 {
                if !ROAD_XS.iter().any(|&rx| (t - rx).abs() < ROAD_HW + 9.0)
                    && ctx.rng.gen_bool(0.38)
                {
                    let model = kits::CARS[ctx.rng.gen_range(0..kits::CARS.len())];
                    let yaw = std::f32::consts::FRAC_PI_2 + ctx.rng.gen_range(-0.06..0.06);
                    let car = kit::prop(
                        ctx.commands,
                        ctx.assets,
                        model,
                        Vec2::new(t, lane_y),
                        0.0,
                        yaw,
                        kits::CAR_SCALE,
                        kits.car.clone(),
                        Some(1.9),
                    );
                    ctx.commands
                        .entity(car)
                        .insert(crate::game::drive::Car::parked());
                    cars += 1;
                }
                t += ctx.rng.gen_range(17.0..27.0);
            }
        }
    }
}

/// Central plaza: paved apron, planter ring, fountain, benches.
fn plaza(ctx: &mut Ctx) {
    let (oct, cyl, sphere) = (
        ctx.prims.oct.clone(),
        ctx.prims.cyl_unit.clone(),
        ctx.prims.sphere_sm.clone(),
    );
    let (pavement, stone, water) = (
        ctx.mats.pavement.clone(),
        ctx.mats.stone.clone(),
        ctx.mats.water.clone(),
    );
    ctx.commands.spawn((
        Mesh3d(oct),
        MeshMaterial3d(pavement),
        Transform::from_translation(ground(PLAZA, 0.06)).with_scale(Vec3::new(23.0, 0.16, 23.0)),
        RunEntity,
    ));
    for k in 0..8 {
        let a = k as f32 / 8.0 * std::f32::consts::TAU;
        let pp = PLAZA + Vec2::new(a.cos(), a.sin()) * 10.5;
        kit::prop(
            ctx.commands,
            ctx.assets,
            "models/suburban/planter.glb",
            pp,
            0.06,
            a,
            fit(native::PLANTER, 0.55),
            ctx.mats.terracotta.clone(),
            Some(1.1),
        );
        let (bush, bh) = kits::SHRUBS[ctx.rng.gen_range(0..2)];
        kit::nature(
            ctx.commands,
            ctx.assets,
            &ctx.mats.nature,
            (bush, bh, 0.0),
            pp,
            scale::SHRUB.1,
            &mut ctx.rng,
            false,
        );
    }
    // fountain
    ctx.commands
        .spawn((
            Obstacle { radius: 3.1 },
            Transform::from_translation(ground(PLAZA, 0.0)),
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(cyl.clone()),
                MeshMaterial3d(stone.clone()),
                Transform::from_xyz(0.0, 0.35, 0.0).with_scale(Vec3::new(6.0, 0.7, 6.0)),
            ));
            c.spawn((
                Mesh3d(cyl.clone()),
                MeshMaterial3d(water.clone()),
                Transform::from_xyz(0.0, 0.6, 0.0).with_scale(Vec3::new(5.2, 0.12, 5.2)),
            ));
            c.spawn((
                Mesh3d(cyl),
                MeshMaterial3d(stone),
                Transform::from_xyz(0.0, 1.1, 0.0).with_scale(Vec3::new(0.9, 2.2, 0.9)),
            ));
            c.spawn((
                Mesh3d(sphere),
                MeshMaterial3d(water),
                Transform::from_xyz(0.0, 2.4, 0.0).with_scale(Vec3::splat(2.2)),
            ));
        });
    for k in 0..4 {
        let a = k as f32 / 4.0 * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
        kit::prop_scaled(
            ctx.commands,
            ctx.assets,
            "models/furniture/bench.glb",
            PLAZA + Vec2::new(a.cos(), a.sin()) * 6.4,
            0.06,
            a + std::f32::consts::FRAC_PI_2,
            kits::bench_scale(),
            ctx.mats.wood_pale.clone(),
            None,
        );
    }
}

/// Bandstand: a low octagonal timber stage — two step tiers, a plank deck, six
/// short posts each capped with a coloured stage light. No roof, so it still
/// reads as a stage from the top-down camera.
fn bandstand(ctx: &mut Ctx) {
    let lights = [
        (Color::srgb(1.0, 0.3, 0.7), LinearRgba::rgb(7.0, 1.4, 4.2)),
        (Color::srgb(0.3, 0.9, 1.0), LinearRgba::rgb(1.2, 5.4, 7.0)),
        (Color::srgb(1.0, 0.8, 0.3), LinearRgba::rgb(7.0, 4.6, 1.4)),
    ]
    .map(|(base_color, emissive)| {
        ctx.materials.add(StandardMaterial {
            base_color,
            emissive,
            ..default()
        })
    });
    let (oct, cyl, sphere) = (
        ctx.prims.oct.clone(),
        ctx.prims.cyl_unit.clone(),
        ctx.prims.sphere_sm.clone(),
    );
    let (stone, wood, wood_pale) = (
        ctx.mats.stone.clone(),
        ctx.mats.wood.clone(),
        ctx.mats.wood_pale.clone(),
    );
    ctx.commands
        .spawn((
            Obstacle { radius: 3.6 },
            Transform::from_translation(ground(BANDSTAND, 0.0)),
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(oct.clone()),
                MeshMaterial3d(stone),
                Transform::from_xyz(0.0, 0.1, 0.0).with_scale(Vec3::new(9.4, 0.2, 9.4)),
            ));
            c.spawn((
                Mesh3d(oct.clone()),
                MeshMaterial3d(wood.clone()),
                Transform::from_xyz(0.0, 0.28, 0.0).with_scale(Vec3::new(8.0, 0.24, 8.0)),
            ));
            c.spawn((
                Mesh3d(oct),
                MeshMaterial3d(wood_pale),
                Transform::from_xyz(0.0, 0.52, 0.0).with_scale(Vec3::new(6.8, 0.3, 6.8)),
            ));
            for k in 0..6 {
                let a = k as f32 / 6.0 * std::f32::consts::TAU;
                let r = 2.7;
                c.spawn((
                    Mesh3d(cyl.clone()),
                    MeshMaterial3d(wood.clone()),
                    Transform::from_xyz(a.cos() * r, 1.5, a.sin() * r)
                        .with_scale(Vec3::new(0.28, 2.6, 0.28)),
                ));
                c.spawn((
                    Mesh3d(sphere.clone()),
                    MeshMaterial3d(lights[k as usize % 3].clone()),
                    Transform::from_xyz(a.cos() * r, 2.9, a.sin() * r)
                        .with_scale(Vec3::splat(0.85)),
                ));
            }
        });
}

/// Park: a pond, a denser tree ring, a couple of benches.
fn park(ctx: &mut Ctx) {
    let cyl = ctx.prims.cyl_unit.clone();
    let (water, stone) = (ctx.mats.water.clone(), ctx.mats.stone.clone());
    ctx.commands.spawn((
        Mesh3d(cyl.clone()),
        MeshMaterial3d(water),
        Transform::from_translation(ground(PARK, 0.05)).with_scale(Vec3::new(13.0, 0.12, 10.0)),
        RunEntity,
    ));
    // low stone lip + a collision reservation that keeps the swarm off the water
    ctx.commands.spawn((
        Mesh3d(cyl),
        MeshMaterial3d(stone),
        Transform::from_translation(ground(PARK, 0.02)).with_scale(Vec3::new(13.8, 0.2, 10.8)),
        RunEntity,
    ));
    ctx.commands.spawn((
        Obstacle { radius: 5.6 },
        Transform::from_translation(ground(PARK, 0.0)),
        RunEntity,
    ));
    for k in 0..11 {
        let a = k as f32 / 11.0 * std::f32::consts::TAU;
        let rr = Vec2::new(a.cos() * 11.0, a.sin() * 9.0);
        let model = kits::TREES[ctx.rng.gen_range(0..kits::TREES.len())];
        let metres = ctx.rng.gen_range(scale::TREE.0..scale::TREE.1);
        kit::nature(
            ctx.commands,
            ctx.assets,
            &ctx.mats.nature,
            model,
            PARK + rr,
            metres,
            &mut ctx.rng,
            true,
        );
    }
    for k in 0..2 {
        kit::prop_scaled(
            ctx.commands,
            ctx.assets,
            "models/furniture/bench.glb",
            PARK + Vec2::new(if k == 0 { -13.0 } else { 13.0 }, 0.0),
            0.0,
            if k == 0 {
                std::f32::consts::FRAC_PI_2
            } else {
                -std::f32::consts::FRAC_PI_2
            },
            kits::bench_scale(),
            ctx.mats.wood_pale.clone(),
            None,
        );
    }
    ctx.solids.push((PARK, 6.5));
}

/// A light dusting of trees and props left in the open ground the block layout
/// leaves over.
fn scatter(ctx: &mut Ctx) {
    let arena = ctx.level.arena;
    let mut scattered = 0;
    let mut tries = 0;
    while scattered < 26 && tries < 900 {
        tries += 1;
        let p = Vec2::new(
            ctx.rng.gen_range(-(arena.x - 4.0)..arena.x - 4.0),
            ctx.rng.gen_range(-(arena.y - 4.0)..arena.y - 4.0),
        );
        if p.length() < 12.0
            || ctx.net.on_road(p)
            || p.distance(PLAZA) < 15.0
            || p.distance(BANDSTAND) < 9.0
            || ctx.solids.iter().any(|(q, r)| p.distance(*q) < r + 2.5)
        {
            continue;
        }
        scattered += 1;
        if ctx.rng.gen_bool(0.5) {
            let (path, h, tr) = kits::TREES[ctx.rng.gen_range(0..kits::TREES.len())];
            let metres = ctx.rng.gen_range(scale::TREE.0..scale::TREE.1);
            kit::nature(
                ctx.commands,
                ctx.assets,
                &ctx.mats.nature,
                (path, h, tr),
                p,
                metres,
                &mut ctx.rng,
                true,
            );
            ctx.solids.push((p, tr * fit(h, metres) + 0.3));
        } else {
            let (path, h) = kits::SHRUBS[ctx.rng.gen_range(0..kits::SHRUBS.len())];
            let metres = ctx.rng.gen_range(scale::SHRUB.0..scale::SHRUB.1);
            kit::nature(
                ctx.commands,
                ctx.assets,
                &ctx.mats.nature,
                (path, h, 0.0),
                p,
                metres,
                &mut ctx.rng,
                false,
            );
        }
    }
}
