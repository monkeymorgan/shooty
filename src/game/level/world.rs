//! **The world** — one map with districts in it, rather than one town.
//!
//! Downtown in the middle, suburbs west and north-east, an industrial estate in
//! the south-east, an airfield along the north edge, and woods and farmland
//! filling the south-west. Roads link all of it, which is the point: this is
//! meant to be somewhere you drive across, not an arena you fight in.
//!
//! **The reclaim loop is district by district.** Each of the five gloom
//! sources sits in a different district ([`GLOOM_SITES`], in `gloom::STEMS`
//! order); a source only starts brooding once a hero comes within
//! `gloom::WAKE_RADIUS` of it, and `enemy::run_wave` vents from whichever
//! brooding source is nearest the party — so the mob turns up in the district
//! you walked into, and you clear the map a piece at a time. `speaker_sites` is
//! still empty here, so building is not usable in the world yet — a huge
//! district map wants a build-at-feet fallback, not ten hand-placed pitches
//! (README, M2).
//!
//! ```bash
//! SHOOTY_LEVEL=world cargo run
//! ```
//!
//! **Laying out a district**: pick a [`Block`], ask [`sites`] for somewhere to
//! put things in it, and spawn. `sites` handles the parts that are the same
//! everywhere — stay off the tarmac, stay off everything already placed, give
//! up rather than spin — so a district is a table and a loop.

use bevy::prelude::*;
use rand::Rng;

use super::kits::{self, Kits, native};
use super::net::{self, Block};
use super::{Ctx, Level, Pad, Palette, Perimeter, Road, Run, Surface, Terrain, kit};
use crate::game::scale::{self, fit};
use crate::game::{RunEntity, ground};

/// Half-extents. 600 × 448 units is 469 × 350 m — nine times the town's area,
/// and enough for a 170 m runway to sit along one edge without eating the map.
const ARENA: Vec2 = Vec2::new(300.0, 224.0);

const ROAD_HW: f32 = 5.0;
const WALK_HW: f32 = 1.9;
const STREET: Surface = Surface::Street {
    half_width: ROAD_HW,
    walk_half_width: WALK_HW,
};
/// A runway is much wider than a street — it has to read as one from the
/// height the camera sits at.
const RUNWAY: Surface = Surface::Runway { half_width: 14.0 };
const TAXIWAY: Surface = Surface::Runway { half_width: 7.0 };

const fn ns(at: f32, from: f32, to: f32, surface: Surface) -> Road {
    Road {
        run: Run::NorthSouth,
        at,
        from,
        to,
        surface,
    }
}
const fn ew(at: f32, from: f32, to: f32, surface: Surface) -> Road {
    Road {
        run: Run::EastWest,
        at,
        from,
        to,
        surface,
    }
}

/// **The skeleton.** Arterials at deliberately uneven spacing, and several of
/// them stopping on another arterial rather than running the full width — a
/// T-junction reads far less like graph paper than a crossroads does, and the
/// uneven spacing is what stops the districts coming out the same size.
///
/// These positions also *define* the districts: every district below is one
/// cell of this skeleton, which is what guarantees its local streets connect —
/// they are subdivisions of a block whose edges are already roads.
const ARTERIAL_XS: &[(f32, f32, f32)] = &[
    (-206.0, -264.0, 264.0),
    (-104.0, -264.0, 116.0), // stops on the north cross street
    (-8.0, -264.0, 264.0),
    (92.0, -152.0, 264.0), // starts on the south cross street
    (196.0, -224.0, 116.0),
];
const ARTERIAL_YS: &[(f32, f32, f32)] = &[
    (-152.0, -340.0, 340.0),
    (-66.0, -340.0, 340.0),
    (30.0, -206.0, 340.0), // starts on the western arterial
    (116.0, -340.0, 300.0),
];

/// A district: the block it occupies, and how finely its streets carve it up.
/// `min_block` is the smallest block its subdivision will leave — small for a
/// downtown of tight city blocks, large for an industrial estate of big yards,
/// and `None` for somewhere with no streets at all.
struct District {
    name: &'static str,
    block: Block,
    min_block: Option<Vec2>,
}

/// Districts, sized to what has to fit in them.
///
/// The first cut made downtown 196 × 96, and the fill diagnostics said exactly
/// why that failed: eight blocks between them offered **five** buildable cells,
/// because a Kenney tower fitted to 20 m is around 20 m across and a block has
/// to hold the paving inset on both sides plus a plot. A city centre needs the
/// room to be a city centre, so downtown is the widest district here and the
/// suburbs moved north around it.
const DISTRICTS: &[District] = &[
    District {
        name: "downtown",
        block: Block::new(-206.0, -66.0, 92.0, 30.0),
        min_block: Some(Vec2::new(84.0, 80.0)),
    },
    District {
        name: "suburb-west",
        block: Block::new(-300.0, 30.0, -8.0, 116.0),
        min_block: Some(Vec2::new(60.0, 56.0)),
    },
    District {
        name: "suburb-north",
        block: Block::new(92.0, 30.0, 300.0, 116.0),
        min_block: Some(Vec2::new(60.0, 56.0)),
    },
    District {
        // The arterials cross the whole map, so every cell they make needs an
        // owner — the ones left unassigned came out as roads through empty
        // grass, which is most of what made the world look abandoned.
        name: "eastside",
        block: Block::new(92.0, -66.0, 300.0, 30.0),
        min_block: Some(Vec2::new(60.0, 56.0)),
    },
    District {
        name: "north-fields",
        block: Block::new(-8.0, 116.0, 300.0, 224.0),
        min_block: Some(Vec2::new(150.0, 104.0)),
    },
    District {
        name: "industry",
        block: Block::new(92.0, -224.0, 300.0, -66.0),
        min_block: Some(Vec2::new(90.0, 84.0)),
    },
    District {
        name: "farm",
        block: Block::new(-206.0, -224.0, 92.0, -152.0),
        min_block: Some(Vec2::new(150.0, 90.0)),
    },
    District {
        name: "woods",
        block: Block::new(-300.0, -224.0, -206.0, 30.0),
        min_block: None,
    },
    District {
        name: "airfield",
        block: Block::new(-300.0, 116.0, -8.0, 224.0),
        min_block: None,
    },
];

fn district(name: &str) -> Block {
    DISTRICTS
        .iter()
        .find(|d| d.name == name)
        .expect("district exists")
        .block
}

/// Lay the network: arterials first, then each district's local streets as a
/// subdivision of its own block.
fn roads(rng: &mut rand::rngs::StdRng, _arena: Vec2) -> Vec<Road> {
    let mut out = Vec::new();
    for &(at, from, to) in ARTERIAL_XS {
        out.push(ns(at, from, to, STREET));
    }
    for &(at, from, to) in ARTERIAL_YS {
        out.push(ew(at, from, to, STREET));
    }
    for d in DISTRICTS {
        if let Some(min_block) = d.min_block {
            net::subdivide(d.block, min_block, STREET, rng, &mut out);
        }
    }
    // The airfield, which is paving rather than streets.
    out.push(ew(178.0, -280.0, -60.0, RUNWAY));
    out.push(ns(-100.0, 140.0, 178.0, TAXIWAY));
    out
}

/// The apron the aircraft would park on, beside the taxiway.
const PADS: &[Pad] = &[Pad {
    centre: Vec2::new(-72.0, 152.0),
    size: Vec2::new(64.0, 40.0),
}];

/// Where the stage would go if this map ever carried the mission — the middle
/// of downtown. Nothing plots it while `speaker_sites` is empty, but `Level`
/// wants a point and the centre of town is the honest answer. Cured gloom folk
/// also march here.
const DOWNTOWN_PLAZA: Vec2 = Vec2::new(29.0, 20.0);

/// The five gloom sources, one per themed district, in `gloom::STEMS` order
/// (hiphop, techno, disco, country, jpop). Each sits inside its district's
/// block so a hero has to travel into that district to wake it.
const GLOOM_SITES: &[Vec2] = &[
    Vec2::new(-36.0, -12.0),  // hiphop  — downtown
    Vec2::new(176.0, -140.0), // techno  — industry
    Vec2::new(-150.0, 66.0),  // disco   — suburb-west
    Vec2::new(-52.0, -186.0), // country — farm
    Vec2::new(-250.0, -70.0), // jpop    — woods
];

pub static WORLD: Level = Level {
    name: "world",
    arena: ARENA,
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
    seed: 0x5C0FFEE,
    terrain: Terrain {
        pads: PADS,
        crossings: true,
        perimeter: Some(Perimeter {
            height: 2.6,
            thickness: 2.0,
        }),
    },
    // No fixed build pitches yet (see the module docs); the wave loop runs off
    // the per-district gloom sources below.
    speaker_sites: &[],
    stage_site: DOWNTOWN_PLAZA,
    gloom_sites: GLOOM_SITES,
    dress,
};

fn dress(ctx: &mut Ctx) {
    let kits = Kits::load(ctx);

    downtown(ctx, &kits);
    suburb(ctx, &kits, district("suburb-west"), 90);
    suburb(ctx, &kits, district("suburb-north"), 70);
    suburb(ctx, &kits, district("eastside"), 70);
    industry(ctx, &kits);
    airfield(ctx, &kits);
    woods(ctx);
    farm(ctx, &kits);
    countryside(ctx);
    super::sky::spawn_blimp(ctx);
    street_dressing(ctx, &kits);
    parked_cars(ctx, &kits);

    info!(
        "world built: {} things reserved on the ground",
        ctx.solids.len()
    );
}

/// **Somewhere to put things.** Rejection-samples `want` points inside `zone`
/// that are off the tarmac and at least `radius` clear of everything already
/// placed, reserving each one as it goes.
///
/// Returns however many it found — a district that asks for more than fits gets
/// fewer, rather than the caller getting a hang. Everything placed through this
/// ends up in `ctx.solids`, which is also what keeps the roads walkable: a
/// building can never land where the tarmac is.
fn sites(ctx: &mut Ctx, zone: Block, want: usize, radius: f32) -> Vec<Vec2> {
    let mut out = Vec::with_capacity(want);
    let mut tries = 0;
    let budget = want * 60;
    while out.len() < want && tries < budget {
        tries += 1;
        let p = Vec2::new(
            ctx.rng.gen_range(zone.min.x..zone.max.x),
            ctx.rng.gen_range(zone.min.y..zone.max.y),
        );
        // Clear of the **paving**, not just the tarmac, and by this thing's own
        // radius. Testing `on_road` instead is what put buildings on the
        // pavement: it asks whether the centre point is on the road, which says
        // nothing about where the edges land.
        if !ctx.net.fits(p, radius + 1.0) {
            continue;
        }
        // Keep clear of the aprons too — they are there to be parked on.
        if ctx
            .level
            .terrain
            .pads
            .iter()
            .any(|pad| (p - pad.centre).abs().cmple(pad.size * 0.5 + radius).all())
        {
            continue;
        }
        if ctx
            .solids
            .iter()
            .any(|(q, r)| p.distance(*q) < r + radius + 2.0)
        {
            continue;
        }
        ctx.solids.push((p, radius));
        out.push(p);
    }
    out
}

/// **Fill the blocks a district's streets carved out of it.**
///
/// This is the placement half of "lay the network first". The streets come from
/// [`net::subdivide`]; [`net::blocks_of`] replays them to recover the blocks
/// between them; and every building goes inside a block, inset from the paving
/// by the paving's own half-width plus its own radius. It therefore *cannot*
/// stand on a pavement — the geometry forbids it rather than a check catching
/// it afterwards.
///
/// Two earlier attempts got this wrong. Scattering inside the district ignored
/// the paving entirely. Lining the streets ("frontage") fixed the setback but
/// left block interiors empty and, with big downtown footprints, had neighbours
/// rejecting each other faster than they placed: 57 of 150 tries collided.
#[allow(clippy::too_many_arguments)]
fn fill_blocks(
    ctx: &mut Ctx,
    district: Block,
    table: &[kits::Building],
    mats: &[Handle<StandardMaterial>],
    metres: (f32, f32),
    verge: f32,
    gap: f32,
) -> usize {
    // NB the inset below is paid on all four sides of every block, so *fewer,
    // bigger* blocks fit more buildings than many small ones — shrinking
    // `min_block` to get more streets cut the suburbs from 40 houses to 18.
    let roads: Vec<Road> = ctx
        .net
        .roads
        .iter()
        .filter(|r| r.surface.is_street())
        .map(|r| Road {
            run: r.run,
            at: r.at,
            from: r.from,
            to: r.to,
            surface: r.surface,
        })
        .collect();
    let blocks = net::blocks_of(district, &roads);

    // The *typical* footprint sets the grid pitch — not the worst case. Some
    // kit models are short and wide (`building-e` is 0.89 tall by 1.33 across),
    // so fitting them to the tallest height in the range makes a 32 m monster;
    // insetting every block by that left no usable ground at all and placed
    // nothing. Outliers are caught per-building by `fits` below instead.
    let mid = (metres.0 + metres.1) * 0.5;
    let typical = table
        .iter()
        .map(|(_, h, plan)| fit(*h, mid) * plan * 0.5)
        .sum::<f32>()
        / table.len() as f32;
    let pitch = typical * 2.0 + gap;

    let mut placed = 0;
    let (mut cells, mut skipped, mut nofit, mut clashed) = (0, 0, 0, 0);
    debug!(
        "  fill: {} blocks, typical r={typical:.1}, pitch={pitch:.1}",
        blocks.len()
    );
    for block in blocks {
        // Inset by the paving on every side, plus room for the building itself.
        let inset = 8.8 + typical + verge;
        let usable = Vec2::new(block.size().x - inset * 2.0, block.size().y - inset * 2.0);
        if usable.x < 0.0 || usable.y < 0.0 {
            continue;
        }
        let cols = (usable.x / pitch).floor() as i32 + 1;
        let rows = (usable.y / pitch).floor() as i32 + 1;
        let origin = block.centre() - Vec2::new(usable.x, usable.y) * 0.5;
        let step = Vec2::new(
            if cols > 1 {
                usable.x / (cols - 1) as f32
            } else {
                0.0
            },
            if rows > 1 {
                usable.y / (rows - 1) as f32
            } else {
                0.0
            },
        );
        for cx in 0..cols {
            for cy in 0..rows {
                // Interior plots of a deep block are hidden anyway, and a solid
                // slab of buildings reads worse than one with yards in it.
                let edge = cx == 0 || cy == 0 || cx == cols - 1 || cy == rows - 1;
                cells += 1;
                if !edge && ctx.rng.gen_bool(0.15) {
                    skipped += 1;
                    continue;
                }
                let jitter = Vec2::new(ctx.rng.gen_range(-1.5..1.5), ctx.rng.gen_range(-1.5..1.5));
                let p = origin + Vec2::new(cx as f32 * step.x, cy as f32 * step.y) + jitter;

                let (model, native_h, native_plan) = table[ctx.rng.gen_range(0..table.len())];
                let s = fit(native_h, ctx.rng.gen_range(metres.0..metres.1));
                let radius = s * native_plan * 0.5;
                if !ctx.net.fits(p, radius + 0.5) {
                    nofit += 1;
                    continue;
                }
                if ctx
                    .solids
                    .iter()
                    .any(|(q, r)| p.distance(*q) < r + radius + 0.5)
                {
                    clashed += 1;
                    continue;
                }
                ctx.solids.push((p, radius));
                // Turn to face the nearest block edge — which is a street.
                let off = p - block.centre();
                let half = block.size() * 0.5;
                let yaw = if (off.x.abs() / half.x) > (off.y.abs() / half.y) {
                    if off.x > 0.0 {
                        std::f32::consts::FRAC_PI_2
                    } else {
                        -std::f32::consts::FRAC_PI_2
                    }
                } else if off.y > 0.0 {
                    0.0
                } else {
                    std::f32::consts::PI
                };
                let mat = mats[ctx.rng.gen_range(0..mats.len())].clone();
                kit::building(ctx.commands, ctx.assets, model, p, s, 0.0, yaw, radius, mat);
                placed += 1;
            }
        }
    }
    debug!(
        "  fill: cells {cells}, skipped {skipped}, no-fit {nofit}, clashed {clashed}, placed {placed}"
    );
    placed
}

/// Spawn one building from `table`, sized somewhere in `metres`, at `p`.
fn building_at(
    ctx: &mut Ctx,
    table: &[kits::Building],
    mats: &[Handle<StandardMaterial>],
    p: Vec2,
    metres: (f32, f32),
) {
    let (model, native_h, native_plan) = table[ctx.rng.gen_range(0..table.len())];
    let s = fit(native_h, ctx.rng.gen_range(metres.0..metres.1));
    let radius = s * native_plan * 0.5;
    let yaw = ctx.rng.gen_range(0..4) as f32 * std::f32::consts::FRAC_PI_2;
    let mat = mats[ctx.rng.gen_range(0..mats.len())].clone();
    kit::building(ctx.commands, ctx.assets, model, p, s, 0.0, yaw, radius, mat);
}

/// Downtown towers, taller than the town's. The town's `scale::TOWER` is sized
/// for somewhere you fight at street level; downtown here has to read as the
/// centre of a 470 m world from the far side of it, so it goes up.
const DOWNTOWN_TOWER: (f32, f32) = (13.0, 24.0);
const DOWNTOWN_LANDMARK: (f32, f32) = (30.0, 48.0);

/// Downtown: office blocks shoulder to shoulder along the grid, a few landmark
/// towers among them, and infill on the ground behind the frontage.
fn downtown(ctx: &mut Ctx, kits: &Kits) {
    let n = fill_blocks(
        ctx,
        district("downtown"),
        kits::CITY_TALL,
        &kits.commercial,
        DOWNTOWN_TOWER,
        0.5,
        1.5,
    );

    // Landmarks: a handful of much taller towers. They still go on the
    // frontage — a skyscraper standing in the middle of a block with its back
    // to every street is exactly the "dropped in a field" look — but `sites`
    // now keeps them off the paving, so the ones that land in a block interior
    // are at least legitimately in a block interior.
    let mut tall = 0;
    for p in sites(ctx, district("downtown"), 7, 13.0) {
        building_at(
            ctx,
            kits::SKYSCRAPERS,
            &kits.commercial,
            p,
            DOWNTOWN_LANDMARK,
        );
        tall += 1;
    }
    info!("downtown: {n} blocks-filled, {tall} landmarks");
}

/// A suburb: houses down both sides of every street through it, then a scatter
/// of back-plot houses and gardens behind them.
fn suburb(ctx: &mut Ctx, kits: &Kits, zone: Block, count: usize) {
    let n = fill_blocks(ctx, zone, kits::HOUSES, &kits.house, scale::HOUSE, 0.5, 2.0);
    info!("suburb: {n} houses");
    let _ = count;

    for p in sites(ctx, zone, 0, 9.0) {
        building_at(ctx, kits::HOUSES, &kits.house, p, scale::HOUSE);

        if ctx.rng.gen_bool(0.6) {
            let off = Vec2::new(ctx.rng.gen_range(-1.0..1.0), ctx.rng.gen_range(-1.0..1.0))
                .normalize_or_zero()
                * 8.5;
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
        if ctx.rng.gen_bool(0.5) {
            let yaw = ctx.rng.gen_range(0..4) as f32 * std::f32::consts::FRAC_PI_2;
            kit::prop(
                ctx.commands,
                ctx.assets,
                "models/suburban/fence-1x4.glb",
                p + Vec2::new(0.0, -7.5),
                0.0,
                yaw,
                fit(native::FENCE_1X4, scale::FENCE),
                kits.street.clone(),
                None,
            );
        }
    }
}

/// The estate: sheds and plant, a container yard, a solar array, and turbines
/// out along the edge.
fn industry(ctx: &mut Ctx, kits: &Kits) {
    let n = fill_blocks(
        ctx,
        district("industry"),
        kits::INDUSTRIAL,
        &kits.industrial,
        scale::WAREHOUSE,
        1.0,
        4.0,
    );
    info!("industry: {n} sheds");
    for p in sites(ctx, district("industry"), 46, 11.0) {
        building_at(ctx, kits::INDUSTRIAL, &kits.industrial, p, scale::WAREHOUSE);
    }

    // Plant — chimneys, tanks, a water tower. Tall, so it gives the estate a
    // skyline you can navigate by from the other side of the world.
    for p in sites(ctx, district("industry"), 26, 9.0) {
        let (model, native_h, metres, solid) = kits::PLANT[ctx.rng.gen_range(0..kits::PLANT.len())];
        kit::prop(
            ctx.commands,
            ctx.assets,
            model,
            p,
            0.0,
            ctx.rng.gen_range(0.0..std::f32::consts::TAU),
            fit(native_h, metres),
            kits.industrial[0].clone(),
            solid,
        );
    }

    // A container yard: stacks of two and three, in rows, because a scatter of
    // containers reads as litter rather than as cargo.
    let yard = Block::new(150.0, -200.0, 250.0, -150.0);
    for p in sites(ctx, yard, 22, 7.0) {
        let stack = ctx.rng.gen_range(1..=3);
        let yaw = if ctx.rng.gen_bool(0.5) {
            0.0
        } else {
            std::f32::consts::FRAC_PI_2
        };
        for k in 0..stack {
            let model = kits::CONTAINERS[ctx.rng.gen_range(0..kits::CONTAINERS.len())];
            let s = fit(kits::CONTAINER_NATIVE, scale::CONTAINER);
            kit::prop(
                ctx.commands,
                ctx.assets,
                model,
                p,
                k as f32 * scale::CONTAINER * scale::U_PER_M,
                yaw,
                s,
                kits.industrial[ctx.rng.gen_range(0..kits.industrial.len())].clone(),
                if k == 0 { Some(3.0) } else { None },
            );
        }
    }

    // Solar array and turbines on the far edge.
    for p in sites(ctx, Block::new(200.0, -120.0, 285.0, -60.0), 18, 6.0) {
        let (model, native_h) = kits::SOLAR[ctx.rng.gen_range(0..kits::SOLAR.len())];
        kit::prop(
            ctx.commands,
            ctx.assets,
            model,
            p,
            0.0,
            0.0,
            fit(native_h, scale::SOLAR),
            kits.industrial[0].clone(),
            None,
        );
    }
    for p in sites(ctx, Block::new(215.0, -40.0, 285.0, 20.0), 5, 20.0) {
        let (model, native_h) = kits::WINDMILLS[ctx.rng.gen_range(0..kits::WINDMILLS.len())];
        let metres = ctx.rng.gen_range(scale::WINDMILL.0..scale::WINDMILL.1);
        kit::prop(
            ctx.commands,
            ctx.assets,
            model,
            p,
            0.0,
            ctx.rng.gen_range(-0.4..0.4),
            fit(native_h, metres),
            kits.industrial[0].clone(),
            Some(2.5),
        );
    }
}

/// The airfield: hangars along the apron, and nothing at all on the runway.
fn airfield(ctx: &mut Ctx, kits: &Kits) {
    // Reserve the runway generously before anything is placed near it — a shed
    // on the centre line would be the one mistake you cannot miss from the air.
    for i in 0..22 {
        let x = -280.0 + i as f32 * 10.0;
        ctx.solids.push((Vec2::new(x, 178.0), 18.0));
    }

    let hangars = Block::new(-200.0, 136.0, -70.0, 158.0);
    for p in sites(ctx, hangars, 6, 17.0) {
        let (model, native_h, native_plan) = kits::SHEDS[ctx.rng.gen_range(0..kits::SHEDS.len())];
        let s = fit(native_h, scale::HANGAR);
        let radius = s * native_plan * 0.5;
        kit::building(
            ctx.commands,
            ctx.assets,
            model,
            p,
            s,
            0.0,
            0.0,
            radius,
            kits.industrial[ctx.rng.gen_range(0..kits.industrial.len())].clone(),
        );
    }

    // Aircraft on the apron. These keep their own materials (see
    // `kit::model`) — they are CC-BY models, not Kenney colormap kit.
    let apron = PADS[0].centre;
    for (i, craft) in [kits::PLANE, kits::PLANE_SMALL, kits::PLANE]
        .into_iter()
        .enumerate()
    {
        let (path, native, metres, lift) = craft;
        let at = apron + Vec2::new(-22.0 + i as f32 * 22.0, ctx.rng.gen_range(-6.0..6.0));
        let e = kit::model(
            ctx.commands,
            ctx.assets,
            path,
            at,
            0.0,
            // Nose out toward the taxiway, give or take.
            std::f32::consts::PI + ctx.rng.gen_range(-0.25..0.25),
            fit(native, metres),
            lift,
            Some(4.5),
        );
        // The middle one is the one that flies. The others are dressing, so the
        // apron does not look like a single-aircraft airfield.
        if i == 0 {
            crate::game::fly::make_flyable(ctx.commands, e);
        }
        ctx.solids.push((at, 9.0));
    }

    // A windsock stand-in and some ground clutter along the strip: barriers and
    // cones off the roads kit, which is what an apron edge is made of anyway.
    let edge = Block::new(-270.0, 162.0, -80.0, 168.0);
    for p in sites(ctx, edge, 20, 5.0) {
        let model = if ctx.rng.gen_bool(0.5) {
            "models/city-roads/construction-barrier.glb"
        } else {
            "models/city-roads/construction-cone.glb"
        };
        kit::prop(
            ctx.commands,
            ctx.assets,
            model,
            p,
            0.0,
            ctx.rng.gen_range(0.0..std::f32::consts::TAU),
            fit(0.5, 1.1),
            kits.street.clone(),
            None,
        );
    }
}

/// The wooded belt: conifers, dense, with rocks and logs on the floor.
fn woods(ctx: &mut Ctx) {
    let mut n = 0;
    // A wood is trees close enough that their canopies touch, so the packing
    // radius here is much tighter than anything with a footprint you walk
    // around. At radius 4 the zone saturated at 67 trees and read as scrub.
    for p in sites(ctx, district("woods"), 230, 2.4) {
        n += 1;
        let model = kits::CONIFERS[ctx.rng.gen_range(0..kits::CONIFERS.len())];
        let metres = ctx.rng.gen_range(scale::CONIFER.0..scale::CONIFER.1);
        kit::nature(
            ctx.commands,
            ctx.assets,
            &ctx.mats.nature,
            model,
            p,
            metres,
            &mut ctx.rng,
            true,
        );
    }
    info!("woods: {n} conifers");
    for p in sites(ctx, district("woods"), 90, 3.0) {
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

/// **Farmland**: farmsteads, fenced fields, and crop rows between them.
///
/// The buildings are Quaternius' CC0 Farm Buildings pack, converted from OBJ
/// (see `kits::FARMSTEAD`). They are authored in metres, so they scale by
/// `U_PER_M` flat rather than being fitted to a target height like the kits.
fn farm(ctx: &mut Ctx, _kits: &Kits) {
    const CROPS: &[&str] = &[
        "models/nature/crops_wheatStageB.glb",
        "models/nature/crops_cornStageC.glb",
        "models/nature/crops_cornStageD.glb",
        "models/nature/crops_leafsStageB.glb",
    ];
    let scale = scale::U_PER_M;

    for zone in [district("farm"), district("north-fields")] {
        let roads: Vec<Road> = ctx
            .net
            .roads
            .iter()
            .filter(|r| r.surface.is_street())
            .map(|r| Road {
                run: r.run,
                at: r.at,
                from: r.from,
                to: r.to,
                surface: r.surface,
            })
            .collect();

        for block in net::blocks_of(zone, &roads) {
            let size = block.size();
            if size.x < 60.0 || size.y < 50.0 {
                continue;
            }
            // A farmstead in one corner of the block, the rest under crop.
            let stead = block.min + Vec2::new(size.x * 0.16, size.y * 0.22);
            let mut placed_stead = 0;
            for k in 0..5 {
                let (path, h, lift, r) =
                    kits::FARMSTEAD[ctx.rng.gen_range(0..kits::FARMSTEAD.len())];
                let at = stead
                    + Vec2::new(
                        ctx.rng.gen_range(-16.0..16.0),
                        ctx.rng.gen_range(-14.0..14.0),
                    );
                let radius = r * scale::U_PER_M;
                if !ctx.net.fits(at, radius + 1.0)
                    || ctx
                        .solids
                        .iter()
                        .any(|(q, rr)| at.distance(*q) < rr + radius + 1.5)
                {
                    continue;
                }
                ctx.solids.push((at, radius));
                kit::model(
                    ctx.commands,
                    ctx.assets,
                    path,
                    at,
                    0.0,
                    ctx.rng.gen_range(0..4) as f32 * std::f32::consts::FRAC_PI_2,
                    scale,
                    lift,
                    Some(radius),
                );
                let _ = (h, k);
                placed_stead += 1;
            }

            // The field: crop rows over the far side of the block, fenced.
            let field = Block::new(
                block.min.x + size.x * 0.42,
                block.min.y + 16.0,
                block.max.x - 16.0,
                block.max.y - 16.0,
            );
            let fsize = field.size();
            if fsize.x < 24.0 || fsize.y < 24.0 {
                continue;
            }
            let crop = CROPS[ctx.rng.gen_range(0..CROPS.len())];
            let spacing = 4.6;
            let (cols, rows) = ((fsize.x / spacing) as i32, (fsize.y / spacing) as i32);
            for cx in 0..cols {
                for cy in 0..rows {
                    let p = field.min + Vec2::new(cx as f32 * spacing, cy as f32 * spacing);
                    if !ctx.net.fits(p, 2.0) {
                        continue;
                    }
                    kit::nature(
                        ctx.commands,
                        ctx.assets,
                        &ctx.mats.nature,
                        (crop, 0.5, 0.0),
                        p,
                        scale::CROP_ROW,
                        &mut ctx.rng,
                        false,
                    );
                }
            }

            // Post fence round the field, from the farm pack rather than the
            // nature kit — same pack as the barns, so it matches them.
            let (fpath, _fh, flift, _) = kits::FARM_FENCE[ctx.rng.gen_range(0..2)];
            let run = kits::FARM_FENCE_RUN * scale::U_PER_M;
            let edge = |ctx: &mut Ctx, from: Vec2, to: Vec2, yaw: f32| {
                let len = from.distance(to);
                let n = (len / run).floor() as i32;
                let dir = (to - from).normalize_or_zero();
                for i in 0..n {
                    let p = from + dir * (i as f32 + 0.5) * run;
                    if !ctx.net.fits(p, 1.5) {
                        continue;
                    }
                    kit::model(
                        ctx.commands,
                        ctx.assets,
                        fpath,
                        p,
                        0.0,
                        yaw,
                        scale,
                        flift,
                        None,
                    );
                }
            };
            let (a, b) = (field.min, field.max);
            edge(ctx, a, Vec2::new(b.x, a.y), 0.0);
            edge(ctx, Vec2::new(a.x, b.y), b, 0.0);
            edge(ctx, a, Vec2::new(a.x, b.y), std::f32::consts::FRAC_PI_2);
            edge(ctx, Vec2::new(b.x, a.y), b, std::f32::consts::FRAC_PI_2);

            ctx.solids
                .push((field.centre(), fsize.max_element() * 0.45));
            debug!("farm block: {placed_stead} farmstead pieces, {cols}x{rows} crop");
        }
    }
}

/// Hedgerow trees and scrub over whatever ground the districts left over, so
/// the space between them reads as countryside rather than as a lawn.
fn countryside(ctx: &mut Ctx) {
    let whole = Block::new(-ARENA.x + 6.0, -ARENA.y + 6.0, ARENA.x - 6.0, ARENA.y - 6.0);
    for p in sites(ctx, whole, 260, 7.0) {
        if ctx.rng.gen_bool(0.62) {
            let model = kits::TREES[ctx.rng.gen_range(0..kits::TREES.len())];
            let metres = ctx.rng.gen_range(scale::TREE.0..scale::TREE.1);
            kit::nature(
                ctx.commands,
                ctx.assets,
                &ctx.mats.nature,
                model,
                p,
                metres,
                &mut ctx.rng,
                true,
            );
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

/// Lamp posts and signs down the streets, spaced wide — the world is nine times
/// the town's area, so the town's spacing would be nine times the entities.
fn street_dressing(ctx: &mut Ctx, kits: &Kits) {
    let roads: Vec<(Run, f32, f32, f32)> = ctx
        .net
        .roads
        .iter()
        .filter(|r| matches!(r.surface, Surface::Street { .. }))
        .map(|r| (r.run, r.at, r.from, r.to))
        .collect();

    for (run, at, from, to) in roads {
        let (lo, hi) = match run {
            Run::NorthSouth => (from.max(-ARENA.y + 10.0), to.min(ARENA.y - 10.0)),
            Run::EastWest => (from.max(-ARENA.x + 10.0), to.min(ARENA.x - 10.0)),
        };
        for side in [-1.0f32, 1.0] {
            let offset = side * (ROAD_HW + WALK_HW + 0.3);
            let mut t = lo + ctx.rng.gen_range(0.0..20.0);
            while t < hi {
                let p = match run {
                    Run::NorthSouth => Vec2::new(at + offset, t),
                    Run::EastWest => Vec2::new(t, at + offset),
                };
                let face = match (run, side > 0.0) {
                    (Run::NorthSouth, true) => -std::f32::consts::FRAC_PI_2,
                    (Run::NorthSouth, false) => std::f32::consts::FRAC_PI_2,
                    (Run::EastWest, true) => std::f32::consts::PI,
                    (Run::EastWest, false) => 0.0,
                };
                if ctx.rng.gen_bool(0.72) {
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
                    let arm = Vec2::new(-face.sin(), -face.cos()) * (0.151 * lamp);
                    let (mesh, mat) = (ctx.prims.sphere_sm.clone(), ctx.mats.bulb.clone());
                    ctx.commands.spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(mat),
                        Transform::from_translation(ground(p + arm, 0.618 * lamp)),
                        RunEntity,
                    ));
                } else {
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
                t += ctx.rng.gen_range(34.0..52.0);
            }
        }
    }
}

/// Cars along the kerbs, spread over the whole network.
///
/// The first version walked the streets in order and stopped at the cap, which
/// parked every car on the first two roads it happened to visit and left the
/// rest of the world empty. This one collects every legal kerbside slot first,
/// shuffles, and takes the cap off the top — so the same number of cars covers
/// the whole map.
fn parked_cars(ctx: &mut Ctx, kits: &Kits) {
    let streets: Vec<(Run, f32, f32, f32)> = ctx
        .net
        .roads
        .iter()
        .filter(|r| matches!(r.surface, Surface::Street { .. }))
        .map(|r| (r.run, r.at, r.from, r.to))
        .collect();

    let mut slots: Vec<(Vec2, f32)> = Vec::new();
    for (run, at, from, to) in streets {
        let (lo, hi) = match run {
            Run::NorthSouth => (from.max(-ARENA.y + 20.0), to.min(ARENA.y - 20.0)),
            Run::EastWest => (from.max(-ARENA.x + 20.0), to.min(ARENA.x - 20.0)),
        };
        for side in [-1.0f32, 1.0] {
            let lane = side * (ROAD_HW - 1.4);
            let mut t = lo + 12.0;
            while t < hi {
                // Nose with the traffic: down-map on one side, up on the other.
                let (p, yaw) = match run {
                    Run::NorthSouth => (
                        Vec2::new(at + lane, t),
                        if side > 0.0 {
                            0.0
                        } else {
                            std::f32::consts::PI
                        },
                    ),
                    Run::EastWest => (
                        Vec2::new(t, at + lane),
                        if side > 0.0 {
                            std::f32::consts::FRAC_PI_2
                        } else {
                            -std::f32::consts::FRAC_PI_2
                        },
                    ),
                };
                // Not on a junction: a car parked across an intersection blocks
                // the one route through it.
                let clear = ctx
                    .net
                    .roads
                    .iter()
                    .filter(|r| matches!(r.surface, Surface::Street { .. }))
                    .all(|r| {
                        let crosses = match (run, r.run) {
                            (Run::NorthSouth, Run::EastWest) => (t - r.at).abs() < 16.0,
                            (Run::EastWest, Run::NorthSouth) => (t - r.at).abs() < 16.0,
                            _ => false,
                        };
                        !crosses
                    });
                if clear {
                    slots.push((p, yaw));
                }
                t += 26.0;
            }
        }
    }

    // Shuffle, then take the cap — every car is a live drivable entity, so the
    // number is a budget, and it should buy coverage rather than a car park.
    for i in (1..slots.len()).rev() {
        let j = ctx.rng.gen_range(0..=i);
        slots.swap(i, j);
    }
    const CARS_WANTED: usize = 64;
    for (p, yaw) in slots.into_iter().take(CARS_WANTED) {
        let model = kits::CARS[ctx.rng.gen_range(0..kits::CARS.len())];
        let jitter = ctx.rng.gen_range(-0.04..0.04);
        let car = kit::prop(
            ctx.commands,
            ctx.assets,
            model,
            p,
            0.0,
            yaw + jitter,
            kits::CAR_SCALE,
            kits.car.clone(),
            Some(1.9),
        );
        ctx.commands
            .entity(car)
            .insert(crate::game::drive::Car::parked());
    }
}

/// Which district a point is in, for anything that wants to know — the minimap,
/// eventually the gating the user asked for. Nothing consumes this yet.
pub fn district_at(p: Vec2) -> Option<&'static str> {
    DISTRICTS
        .iter()
        .find(|d| d.block.contains(p))
        .map(|d| d.name)
}
