//! **One metric standard for the whole town.**
//!
//! Every model in the game arrives normalised to something arbitrary — a
//! Kenney house is ~1 unit tall, a Quaternius person ~1.9, a Tripo GLB fits a
//! unit box — so *every* spawner has to pick a scale factor. Picking them
//! independently is how the town ended up with cars taller than the band and
//! houses a rocker could see over: each number looked fine on its own.
//!
//! So no spawner picks a scale any more. It states **how big the thing is in
//! metres**, and [`fit`] turns that into a factor using the model's own
//! measured native height. Read a size here and you are reading a real-world
//! measurement, which is a claim you can check against a photograph.
//!
//! Native heights were measured off the glTF `POSITION` accessor bounds with
//! node transforms applied, not eyeballed — the old `CITY_MODELS` table had
//! every tower a full unit too tall (it assumed feet at y = -1; they are at
//! y = 0), which is why the commercial blocks came out barely person-height.

use bevy::math::Vec3;

/// World units per metre.
///
/// Set so a 1.8 m person stands ~2.3 u — i.e. the height the heroes already
/// were. The characters were the one part of the game that was internally
/// consistent, so the town moves to meet them rather than the other way round.
pub const U_PER_M: f32 = 1.28;

/// `metres` expressed in world units.
#[inline]
pub fn m(metres: f32) -> f32 {
    metres * U_PER_M
}

/// The uniform scale that makes a model of measured height `native` stand
/// `metres` tall.
#[inline]
pub fn fit(native: f32, metres: f32) -> f32 {
    m(metres) / native
}

/// Per-axis fit: the scale that takes a model of measured size `native` to
/// `metres` on each axis independently.
///
/// Reach for this only when a model's proportions genuinely disagree with the
/// real object's — Kenney's furniture bench is 0.40 long by 0.47 tall, a ratio
/// of 0.85:1, where a real bench is nearer 1.8:1. Fitting it uniformly by
/// height leaves a 0.72 m bench you could not sit two people on; fitting it
/// uniformly by length leaves one 1.6 m tall. Stretching it is the only way to
/// get a bench that measures like a bench, which is what [`scale`] is for.
#[inline]
pub fn fit3(native: Vec3, metres: Vec3) -> Vec3 {
    Vec3::new(
        m(metres.x) / native.x,
        m(metres.y) / native.y,
        m(metres.z) / native.z,
    )
}

// ---------------------------------------------------------------------------
// How tall things are, in metres. The whole town is sized from this block.
// ---------------------------------------------------------------------------

/// A hero, head to heel. Ordinary adult.
pub const PERSON: f32 = 1.80;
/// A gloom goon. A shade taller than the band — a heavy, not a giant.
pub const GOON: f32 = 1.88;

/// The face-cube "bad vibes" — knee-, chest- and shoulder-high respectively.
/// They are *creatures*, not people, and they read as a threat by swarming, so
/// none of them is allowed to loom over the character you are playing.
pub const VIBE_RUSHER: f32 = 0.70;
pub const VIBE_SPONGE: f32 = 1.25;
pub const VIBE_FLYER: f32 = 0.62;

/// Roof height of a parked car. Kenney's car kit is deliberately stubby, so
/// fitting it by height leaves it short for its width — which is the look.
pub const CAR: f32 = 1.46;

/// Suburban houses: single storey plus a roof, up to a small two-storey.
pub const HOUSE: (f32, f32) = (5.4, 7.2);
/// Commercial blocks in the middle of town: three to five storeys.
pub const TOWER: (f32, f32) = (9.0, 14.0);
/// The two skyscraper models, kept as landmarks — tall enough to read as the
/// centre of town, short enough not to spear the top-down camera.
pub const SKYSCRAPER: (f32, f32) = (16.0, 20.0);

/// Street trees and garden trees.
pub const TREE: (f32, f32) = (4.5, 7.0);
/// Bushes and rocks left in the open ground, **measured across their widest
/// side**. These models are flat and wide, so fitting them by height — which is
/// what the tree table does — inflated their footprints badly: `rock_largeA`
/// came out 5.5 m across for a nominal 1.4 m rock.
pub const SHRUB: (f32, f32) = (0.9, 1.9);

/// Street furniture.
pub const LAMP_POST: f32 = 4.6;
/// A park bench: length, height to the top of the back, depth. Fitted per-axis
/// with [`fit3`] because the model's proportions are not a bench's.
pub const BENCH: Vec3 = Vec3::new(1.55, 0.85, 0.62);
pub const BIN: f32 = 0.95;
pub const HYDRANT: f32 = 0.75;
pub const ROAD_SIGN: f32 = 2.4;
pub const FENCE: f32 = 1.1;

/// A loudspeaker on its pole — head and shoulders above the crowd it is
/// playing to, so you can find one across the town.
pub const SPEAKER: f32 = 4.4;

// ---------------------------------------------------------------------------
// The wider world (`level::world`) — districts the town never had.
// ---------------------------------------------------------------------------

/// Industrial sheds and warehouses. Low and wide: an estate reads differently
/// from a suburb because the buildings are the wrong shape for living in.
pub const WAREHOUSE: (f32, f32) = (7.0, 12.0);
/// An aircraft hangar — tall enough to be obviously not a warehouse.
pub const HANGAR: f32 = 13.0;
/// A shipping container. Real ones are 2.59 m; these are stacked, so the number
/// has to be right or the stacks look like toys next to the sheds.
pub const CONTAINER: f32 = 2.59;
/// A wind turbine on the edge of the estate.
pub const WINDMILL: (f32, f32) = (26.0, 34.0);
/// A ground-mounted solar array.
pub const SOLAR: f32 = 2.4;
/// Conifers in the wooded belts — taller and narrower than the street trees.
pub const CONIFER: (f32, f32) = (7.0, 13.0);
/// A row of crops in a field. Small things read as noise from the play
/// camera's height, so these are deliberately larger than life.
pub const CROP_ROW: f32 = 2.8;
/// A field fence.
pub const FARM_FENCE: f32 = 1.3;

/// Aircraft, measured across the **wingspan** — a light single-engine plane.
pub const PLANE_SPAN: f32 = 11.0;
/// A small airship, measured nose to tail.
pub const BLIMP_LENGTH: f32 = 50.0;
/// How high the blimp drifts. High enough to read as "up there", low enough
/// that it is still inside the world's fog and actually visible from the ground.
pub const BLIMP_ALTITUDE: f32 = 58.0;
