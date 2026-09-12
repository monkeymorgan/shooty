//! **Things in the air.** Currently one blimp, going nowhere in particular.
//!
//! It is scenery, not a vehicle: it drifts a slow circle over the middle of the
//! world at a fixed altitude, nose along its own travel, and nothing collides
//! with it. The point is entirely that you look up and there is a blimp.

use bevy::prelude::*;

use super::kits;
use crate::game::scale::{self, fit};
use crate::game::{GameState, ground};

/// Drifts a slow circle. `phase` is where it is round that circle, in radians.
#[derive(Component)]
pub struct Blimp {
    phase: f32,
    /// Half-extents of the circle it flies, so it tracks the level's shape
    /// rather than a fixed radius.
    ellipse: Vec2,
}

/// How long one lap takes, in seconds. Slow: it should read as barely moving.
const LAP: f32 = 150.0;

/// Put the blimp in the sky. Called from a level's `dress`.
pub fn spawn_blimp(ctx: &mut super::Ctx) {
    let (path, native, metres, lift) = kits::BLIMP;
    let ellipse = ctx.level.arena * 0.55;
    let at = Vec2::new(ellipse.x, 0.0);
    let e = super::kit::model(
        ctx.commands,
        ctx.assets,
        path,
        at,
        scale::m(scale::BLIMP_ALTITUDE),
        0.0,
        fit(native, metres),
        lift,
        None,
    );
    ctx.commands.entity(e).insert(Blimp {
        phase: 0.0,
        ellipse,
    });
}

/// Walk the blimp round its circle, nose along the direction of travel.
pub fn drift(time: Res<Time>, mut blimps: Query<(&mut Blimp, &mut Transform)>) {
    let step = time.delta_secs() * std::f32::consts::TAU / LAP;
    for (mut blimp, mut transform) in &mut blimps {
        blimp.phase = (blimp.phase + step) % std::f32::consts::TAU;
        let (s, c) = blimp.phase.sin_cos();
        let at = Vec2::new(blimp.ellipse.x * c, blimp.ellipse.y * s);
        // Tangent to the ellipse: where it is going, which is where it points.
        let heading = Vec2::new(-blimp.ellipse.x * s, blimp.ellipse.y * c);
        transform.translation = ground(at, scale::m(scale::BLIMP_ALTITUDE));
        // The model's nose is +Z, and `ground` maps plane-y to world -z, so the
        // yaw that points it along `heading` is the same one the cars use.
        transform.rotation = Quat::from_rotation_y(f32::atan2(heading.x, -heading.y));
    }
}

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drift.run_if(in_state(GameState::Playing)));
    }
}
