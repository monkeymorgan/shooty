//! **The ground a level stands on**, described as data rather than written out.
//!
//! Every environment wants the same handful of things underneath it — a slab of
//! ground, some paved runs across it, painted crossings where those runs meet,
//! and something at the edge saying *stop* — and differs only in the numbers and
//! the colours. So those are the numbers and colours: a [`Terrain`] says where
//! the paving goes, a [`super::Palette`] says what it all looks like, and this
//! module is the one copy of the code that lays it out.
//!
//! **Roads are segments, not a grid.** The first version of this took two lists
//! of centre-lines and crossed every one with every other, which is exactly a
//! town and nothing else: roads that run the full width of the world and always
//! intersect. A world with districts needs roads that *stop* — an arterial that
//! serves the suburbs and ends, a runway that is a road in every respect except
//! that it has no kerbs and nothing crosses it. So a [`Road`] has a start and an
//! end, and a [`Surface`] saying what kind of paving it is.

use bevy::prelude::*;

use super::Ctx;
use crate::game::{RunEntity, ground};

/// Which way a paved run goes. Everything here is axis-aligned: the kits are
/// square, the camera is square-on, and a diagonal street would cost more than
/// it is worth.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// Constant `x`, running along plane-`y` from `from` to `to`.
    NorthSouth,
    /// Constant plane-`y`, running along `x`.
    EastWest,
}

/// What kind of paving a run is.
#[derive(Clone, Copy)]
pub enum Surface {
    /// A street: tarmac on a kerb strip, a dashed centre line, and a pavement
    /// down both sides. The thing buildings face onto.
    Street {
        half_width: f32,
        walk_half_width: f32,
    },
    /// Bare tarmac with a dashed centre line and a threshold bar at each end —
    /// a runway or a taxiway. No kerb, no pavement, nothing facing onto it.
    Runway { half_width: f32 },
}

impl Surface {
    /// Half-width of the tarmac itself, whatever kind it is.
    pub fn half_width(self) -> f32 {
        match self {
            Surface::Street { half_width, .. } | Surface::Runway { half_width } => half_width,
        }
    }

    pub fn is_street(self) -> bool {
        matches!(self, Surface::Street { .. })
    }
}

/// One paved run.
pub struct Road {
    pub run: Run,
    /// The centre-line: `x` for [`Run::NorthSouth`], plane-`y` for
    /// [`Run::EastWest`].
    pub at: f32,
    /// Where the run starts and ends, along its own axis.
    pub from: f32,
    pub to: f32,
    pub surface: Surface,
}

impl Road {
    /// Half-width of everything paved on this run — tarmac plus, on a street,
    /// the pavements down both sides.
    pub fn paved_half_width(self: &Road) -> f32 {
        match self.surface {
            Surface::Street {
                half_width,
                walk_half_width,
            } => half_width + walk_half_width * 2.0,
            Surface::Runway { half_width } => half_width,
        }
    }

    /// How far `p` is from this run's paved edge. Negative when `p` is on the
    /// paving; [`f32::MAX`] when it is off the end of the run entirely.
    pub fn clearance(&self, p: Vec2) -> f32 {
        let (across, along) = match self.run {
            Run::NorthSouth => (p.x - self.at, p.y),
            Run::EastWest => (p.y - self.at, p.x),
        };
        let ext = self.paved_half_width();
        if along < self.from - ext || along > self.to + ext {
            return f32::MAX;
        }
        across.abs() - ext
    }

    /// Is `p` on this run's tarmac (plus a margin for the kerb)?
    pub fn covers(&self, p: Vec2) -> bool {
        let (across, along) = match self.run {
            Run::NorthSouth => (p.x - self.at, p.y),
            Run::EastWest => (p.y - self.at, p.x),
        };
        across.abs() < self.surface.half_width() + 1.5 && along >= self.from && along <= self.to
    }

    /// The ground-plane point at distance `along` down the run.
    fn point(&self, along: f32) -> Vec2 {
        match self.run {
            Run::NorthSouth => Vec2::new(self.at, along),
            Run::EastWest => Vec2::new(along, self.at),
        }
    }

    /// `(x, y, z)` scale for a slab of this width running this road's length,
    /// `overshoot` units past each end.
    fn slab(&self, half_width: f32, thickness: f32, overshoot: f32) -> Vec3 {
        self.slab_between(
            half_width,
            thickness,
            self.from - overshoot,
            self.to + overshoot,
        )
    }

    /// As [`Road::slab`], but for an explicit span down the run — used by the
    /// pavements, which stop at the edge of the playfield even where the road
    /// itself carries on into the fog.
    fn slab_between(&self, half_width: f32, thickness: f32, from: f32, to: f32) -> Vec3 {
        let len = (to - from).max(0.0);
        match self.run {
            Run::NorthSouth => Vec3::new(half_width * 2.0, thickness, len),
            Run::EastWest => Vec3::new(len, thickness, half_width * 2.0),
        }
    }

    fn midpoint(&self) -> Vec2 {
        self.point((self.from + self.to) * 0.5)
    }
}

/// A flat paved rectangle that is not a road — an apron, a car park, a yard.
pub struct Pad {
    pub centre: Vec2,
    /// Full width and depth, not half-extents.
    pub size: Vec2,
}

/// The wall around the edge of the playfield — a hedge, a snow bank, a chain
/// fence. Purely visual: the hard limit is [`super::Level::arena`], which the
/// movement code clamps to.
pub struct Perimeter {
    pub height: f32,
    pub thickness: f32,
}

/// What is underneath and around a level, before any of its own dressing.
///
/// The roads are **not** here: they are generated per run into
/// [`Net`](super::net::Net), because a hand-written list could not express a
/// district subdivided into blocks. This is only the parts that are fixed.
pub struct Terrain {
    pub pads: &'static [Pad],
    /// Paint zebra crossings where two *streets* cross. Runways are left alone.
    pub crossings: bool,
    pub perimeter: Option<Perimeter>,
}

/// Lay the ground slab, the roads, the pads, the crossings and the perimeter
/// wall for `ctx.level`.
pub fn build(ctx: &mut Ctx) {
    // Copy the level reference out first: it is `'static`, so nothing below has
    // to hold a borrow of `ctx` while it also spawns through it.
    let level: &'static super::Level = ctx.level;
    let arena = level.arena;
    let cube = ctx.prims.cube.clone();

    // ---- Ground: a big slab, overshooting the walls so the camera never sees
    // an edge.
    let g = arena + Vec2::splat(40.0);
    let ground_mat = ctx.mats.ground.clone();
    ctx.commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(ground_mat),
        Transform::from_xyz(0.0, -0.5, 0.0).with_scale(Vec3::new(g.x * 2.0, 1.0, g.y * 2.0)),
        RunEntity,
    ));

    for road in std::mem::take(&mut ctx.net.roads) {
        pave(ctx, &road);
        ctx.net.roads.push(road);
    }
    for pad in level.terrain.pads {
        let mat = ctx.mats.pavement.clone();
        ctx.commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(ground(pad.centre, 0.05))
                .with_scale(Vec3::new(pad.size.x, 0.16, pad.size.y)),
            RunEntity,
        ));
    }
    if level.terrain.crossings {
        crossings(ctx);
    }

    if let Some(edge) = &level.terrain.perimeter {
        let (h, t) = (edge.height, edge.thickness);
        let wall = ctx.mats.perimeter.clone();
        for (pos, scale) in [
            (
                Vec2::new(0.0, arena.y),
                Vec3::new(arena.x * 2.0 + 2.0 * t, h, t),
            ),
            (
                Vec2::new(0.0, -arena.y),
                Vec3::new(arena.x * 2.0 + 2.0 * t, h, t),
            ),
            (
                Vec2::new(arena.x, 0.0),
                Vec3::new(t, h, arena.y * 2.0 + 2.0 * t),
            ),
            (
                Vec2::new(-arena.x, 0.0),
                Vec3::new(t, h, arena.y * 2.0 + 2.0 * t),
            ),
        ] {
            ctx.commands.spawn((
                Mesh3d(cube.clone()),
                MeshMaterial3d(wall.clone()),
                Transform::from_translation(ground(pos, h * 0.5)).with_scale(scale),
                RunEntity,
            ));
        }
    }
}

/// One paved run: kerb, tarmac, centre line, and — on a street — a pavement
/// down each side.
fn pave(ctx: &mut Ctx, road: &Road) {
    let cube = ctx.prims.cube.clone();
    let hw = road.surface.half_width();
    let centre = ground(road.midpoint(), 0.0);

    // Streets overshoot their ends slightly so a junction reads as continuous
    // paving rather than as two slabs that nearly meet. A runway does not: its
    // ends are the thing you are aiming at.
    let over = if road.surface.is_street() { 2.0 } else { 0.0 };

    if road.surface.is_street() {
        let kerb = ctx.mats.kerb.clone();
        ctx.commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(kerb),
            Transform::from_translation(centre + Vec3::Y * 0.03).with_scale(road.slab(
                hw + 1.5,
                0.06,
                over,
            )),
            RunEntity,
        ));
    }
    let tarmac = ctx.mats.tarmac.clone();
    ctx.commands.spawn((
        Mesh3d(cube.clone()),
        MeshMaterial3d(tarmac),
        Transform::from_translation(centre + Vec3::Y * 0.06).with_scale(road.slab(hw, 0.08, over)),
        RunEntity,
    ));

    // Dashed centre line, one dash roughly every 12 units.
    let len = road.to - road.from;
    let n = ((len / 12.0).round() as i32).max(1);
    let paint = ctx.mats.markings.clone();
    let (dash_long, dash_across) = match road.surface {
        Surface::Street { .. } => (3.0, 0.35),
        // A runway's centre line is longer and fatter — it is read at speed.
        Surface::Runway { .. } => (6.0, 0.9),
    };
    for i in 0..n {
        let along = road.from + (i as f32 + 0.5) / n as f32 * len;
        let scl = match road.run {
            Run::NorthSouth => Vec3::new(dash_across, 0.09, dash_long),
            Run::EastWest => Vec3::new(dash_long, 0.09, dash_across),
        };
        ctx.commands.spawn((
            Mesh3d(cube.clone()),
            MeshMaterial3d(paint.clone()),
            Transform::from_translation(ground(road.point(along), 0.10)).with_scale(scl),
            RunEntity,
        ));
    }

    match road.surface {
        Surface::Street {
            walk_half_width, ..
        } => {
            // Pavements stop at the edge of the playfield even where the road
            // runs on past it: a road disappearing into the fog reads as a road
            // going somewhere, but a pavement out there is just a stray slab.
            let limit = match road.run {
                Run::NorthSouth => ctx.level.arena.y,
                Run::EastWest => ctx.level.arena.x,
            };
            let (from, to) = (road.from.max(-limit), road.to.min(limit));
            let mid = (from + to) * 0.5;
            let pavement = ctx.mats.pavement.clone();
            for (i, side) in [-1.0f32, 1.0].into_iter().enumerate() {
                let offset = side * (hw + walk_half_width);
                let at = match road.run {
                    Run::NorthSouth => Vec2::new(road.at + offset, mid),
                    Run::EastWest => Vec2::new(mid, road.at + offset),
                };
                ctx.commands.spawn((
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(pavement.clone()),
                    Transform::from_translation(ground(at, 0.05 + i as f32 * 0.004))
                        .with_scale(road.slab_between(walk_half_width, 0.16, from, to)),
                    RunEntity,
                ));
            }
        }
        Surface::Runway { .. } => {
            // Threshold bars: a ladder of stripes across each end, which is what
            // makes a strip of tarmac read as a runway from the air.
            let paint = ctx.mats.crossing.clone();
            for end in [road.from + 4.0, road.to - 4.0] {
                for b in -3..=3 {
                    let across = b as f32 * (hw / 4.0);
                    let (p, scl) = match road.run {
                        Run::NorthSouth => (
                            Vec2::new(road.at + across, end),
                            Vec3::new(hw / 9.0, 0.06, 7.0),
                        ),
                        Run::EastWest => (
                            Vec2::new(end, road.at + across),
                            Vec3::new(7.0, 0.06, hw / 9.0),
                        ),
                    };
                    ctx.commands.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(paint.clone()),
                        Transform::from_translation(ground(p, 0.11)).with_scale(scl),
                        RunEntity,
                    ));
                }
            }
        }
    }
}

/// Zebra crossings on every arm of every place two streets actually cross.
fn crossings(ctx: &mut Ctx) {
    let cube = ctx.prims.cube.clone();
    let zebra = ctx.mats.crossing.clone();

    // Copied out of the net first: the loop below spawns through `ctx`, which
    // would otherwise be borrowed for the whole iteration.
    let streets: Vec<(Run, f32, f32, f32, f32)> = ctx
        .net
        .roads
        .iter()
        .filter(|r| r.surface.is_street())
        .map(|r| (r.run, r.at, r.from, r.to, r.surface.half_width()))
        .collect();
    for &(_, ns_at, ns_from, ns_to, ns_hw) in streets.iter().filter(|s| s.0 == Run::NorthSouth) {
        for &(_, ew_at, ew_from, ew_to, ew_hw) in streets.iter().filter(|s| s.0 == Run::EastWest) {
            // Only where the two runs genuinely overlap — an arterial that stops
            // short of a cross street does not get a junction painted on it.
            let meets = ns_at >= ew_from && ns_at <= ew_to && ew_at >= ns_from && ew_at <= ns_to;
            if !meets {
                continue;
            }
            let hw = ns_hw.max(ew_hw);
            for (dx, dy) in [(0.0f32, 1.0f32), (0.0, -1.0), (1.0, 0.0), (-1.0, 0.0)] {
                let across = Vec2::new(dy.abs(), dx.abs()); // 1 along the bar-spread axis
                for b in 0..5 {
                    let o = (b as f32 - 2.0) * 1.4;
                    let centre = Vec2::new(
                        ns_at + dx * (hw + 2.0) + across.x * o,
                        ew_at + dy * (hw + 2.0) + across.y * o,
                    );
                    let scl = if dx.abs() > 0.5 {
                        Vec3::new(3.4, 0.06, 0.55)
                    } else {
                        Vec3::new(0.55, 0.06, 3.4)
                    };
                    ctx.commands.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(zebra.clone()),
                        Transform::from_translation(ground(centre, 0.12)).with_scale(scl),
                        RunEntity,
                    ));
                }
            }
        }
    }
}
