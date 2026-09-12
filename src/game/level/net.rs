//! **The road network, laid out before anything is built on it.**
//!
//! The first world hand-wrote a list of roads and then scattered buildings near
//! them, which produced two problems you could see from the air: streets that
//! stopped in the middle of a field, and buildings standing on the pavement,
//! because "near a road" was measured to the tarmac and a building has width.
//!
//! So the order is inverted. A level declares a **skeleton** of arterials and a
//! set of **districts**; this module turns that into a [`Net`], and everything
//! afterwards — where a building stands, how far back it sits, where a tree may
//! not go — is a question asked *of* the net.
//!
//! **Local streets come from recursive subdivision.** A district is a rectangle
//! bounded by arterials; split it with a street that runs wall to wall, then
//! split each half, and stop when the blocks are as small as that district
//! wants. Every street therefore ends on another street by construction — there
//! is no connectivity pass, because there is nothing to connect. Varying where
//! the split falls (never the middle) and how small the blocks get is what
//! makes a suburb read differently from a downtown, and stops the whole thing
//! looking like graph paper.

use bevy::prelude::*;
use rand::Rng;
use rand::rngs::StdRng;

use super::terrain::{Road, Run, Surface};

/// Every road in a level, and the questions the rest of the game asks about
/// them. Built once per run and kept in the [`RoadNet`] resource.
#[derive(Default)]
pub struct Net {
    pub roads: Vec<Road>,
}

/// The live network, so systems outside the level build (the minimap, and
/// anything that wants to know where the tarmac is) can consult it.
#[derive(Resource, Default)]
pub struct RoadNet(pub Net);

impl Net {
    /// Is this ground-plane point on tarmac (or its kerb)?
    pub fn on_road(&self, p: Vec2) -> bool {
        self.roads.iter().any(|r| r.covers(p))
    }

    /// Distance from `p` out to the nearest **paved** edge — tarmac *and*
    /// pavement, not just tarmac.
    ///
    /// This is the number the old code was missing. `on_road` asks "am I on the
    /// road", which is the wrong question for placing a building: a house needs
    /// its whole footprint clear of the paving, and the paving is wider than
    /// the road. Returns [`f32::MAX`] when nothing is near.
    pub fn paved_clearance(&self, p: Vec2) -> f32 {
        self.roads
            .iter()
            .map(|r| r.clearance(p))
            .fold(f32::MAX, f32::min)
    }

    /// Is there room at `p` for something of this radius, clear of all paving?
    pub fn fits(&self, p: Vec2, radius: f32) -> bool {
        self.paved_clearance(p) > radius
    }
}

/// A rectangle of ground, bounded by roads on all four sides.
#[derive(Clone, Copy, Debug)]
pub struct Block {
    pub min: Vec2,
    pub max: Vec2,
}

impl Block {
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            min: Vec2::new(x0, y0),
            max: Vec2::new(x1, y1),
        }
    }
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
    pub fn centre(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }
}

/// **Fill a district with local streets** by splitting it until the blocks are
/// no bigger than `min_block`.
///
/// Splits never land in the middle: `SPLIT` keeps them off-centre, so the
/// blocks come out uneven and the result does not read as graph paper. A
/// district that wants no streets at all (woods, the airfield) simply never
/// calls this.
pub fn subdivide(
    block: Block,
    min_block: Vec2,
    surface: Surface,
    rng: &mut StdRng,
    out: &mut Vec<Road>,
) {
    /// How far off-centre a split may fall. Dead centre would halve every
    /// block and rebuild the grid this exists to avoid.
    const SPLIT: std::ops::Range<f32> = 0.34..0.66;

    let size = block.size();
    // Only split an axis if both halves would still be worth having.
    let can_x = size.x > min_block.x * 2.0;
    let can_y = size.y > min_block.y * 2.0;
    if !can_x && !can_y {
        return;
    }
    // Cut the long way, so blocks tend back toward square rather than
    // degenerating into strips.
    let cut_x = if can_x && can_y {
        size.x >= size.y
    } else {
        can_x
    };

    let f = rng.gen_range(SPLIT);
    if cut_x {
        let at = block.min.x + size.x * f;
        out.push(Road {
            run: Run::NorthSouth,
            at,
            from: block.min.y,
            to: block.max.y,
            surface,
        });
        subdivide(
            Block::new(block.min.x, block.min.y, at, block.max.y),
            min_block,
            surface,
            rng,
            out,
        );
        subdivide(
            Block::new(at, block.min.y, block.max.x, block.max.y),
            min_block,
            surface,
            rng,
            out,
        );
    } else {
        let at = block.min.y + size.y * f;
        out.push(Road {
            run: Run::EastWest,
            at,
            from: block.min.x,
            to: block.max.x,
            surface,
        });
        subdivide(
            Block::new(block.min.x, block.min.y, block.max.x, at),
            min_block,
            surface,
            rng,
            out,
        );
        subdivide(
            Block::new(block.min.x, at, block.max.x, block.max.y),
            min_block,
            surface,
            rng,
            out,
        );
    }
}

/// Every block a district's streets carved out of it, for placing things
/// *inside* blocks rather than along their edges.
///
/// Recomputed by splitting the district the same way [`subdivide`] did, driven
/// by the streets that actually landed — so it cannot drift out of step with
/// them the way a parallel list would.
pub fn blocks_of(district: Block, roads: &[Road]) -> Vec<Block> {
    let mut out = vec![district];
    for road in roads {
        let mut next = Vec::with_capacity(out.len() + 1);
        for b in out.drain(..) {
            match road.run {
                Run::NorthSouth
                    if road.at > b.min.x + 0.1
                        && road.at < b.max.x - 0.1
                        && road.from <= b.min.y + 0.1
                        && road.to >= b.max.y - 0.1 =>
                {
                    next.push(Block::new(b.min.x, b.min.y, road.at, b.max.y));
                    next.push(Block::new(road.at, b.min.y, b.max.x, b.max.y));
                }
                Run::EastWest
                    if road.at > b.min.y + 0.1
                        && road.at < b.max.y - 0.1
                        && road.from <= b.min.x + 0.1
                        && road.to >= b.max.x - 0.1 =>
                {
                    next.push(Block::new(b.min.x, b.min.y, b.max.x, road.at));
                    next.push(Block::new(b.min.x, road.at, b.max.x, b.max.y));
                }
                _ => next.push(b),
            }
        }
        out = next;
    }
    out
}
