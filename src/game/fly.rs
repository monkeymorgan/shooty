//! **Flying.** `E` next to the plane on the apron gets you in; `E` again, once
//! you are back on the ground and slow, gets you out.
//!
//! This is an arcade model, not a simulator, and the whole thing is three
//! numbers: throttle, airspeed, altitude. You cannot climb until you are going
//! fast enough to fly ([`TAKEOFF`]), and if you let the airspeed fall below that
//! in the air you sink whatever you do with the stick — which is the one bit of
//! real aerodynamics worth keeping, because it is what makes taking off and
//! landing feel like anything at all.
//!
//! It deliberately mirrors [`drive`](super::drive): same interact key, same
//! mount/dismount shape, same "the vehicle carries its pilot" rule. Two
//! vehicles that behave differently at the same key would be worse than either.
//!
//! **Controls** (P1): `W`/`S` throttle · `A`/`D` turn · `Space` climb ·
//! `LeftShift` descend · `E` in/out.

use bevy::prelude::*;

use super::player::Intent;
use super::{CurrentLevel, GameState, Obstacle, Player, RunEntity, ground, plane, scale};

/// Flat-out airspeed, in units per second — comfortably faster than a car, so
/// crossing the world is quick enough to be worth doing.
const TOP_SPEED: f32 = 74.0;
/// Below this the wings do nothing: on the ground you are taxiing, and in the
/// air you are descending whatever the stick says.
const TAKEOFF: f32 = 30.0;
const ACCEL: f32 = 20.0;
/// Drag when the throttle is closed.
const DRAG: f32 = 12.0;
/// Radians per second the nose swings at full deflection, at flying speed.
const TURN_RATE: f32 = 0.95;
/// Climb and sink rates, units per second.
const CLIMB_RATE: f32 = 26.0;
const SINK_RATE: f32 = 34.0;
/// How high it will go. High enough to look down on the whole world, low enough
/// to stay inside the level's fog.
const CEILING: f32 = 150.0;
/// Altitude below which you are considered on the ground.
const GROUNDED: f32 = 0.6;
/// How close a hero has to be to climb in.
const REACH: f32 = 8.0;
/// Ground radius the parked aircraft occupies.
const PARKED_RADIUS: f32 = 4.5;
/// Where the pilot sits, in the plane's own space.
const SEAT: Vec3 = Vec3::new(0.0, 1.1, 0.2);

/// A flyable aircraft. Sits on the same entity the model was spawned on.
#[derive(Component, Default)]
pub struct Plane {
    /// 0..1, eased toward by the throttle input.
    throttle: f32,
    /// Along the nose (+Z in the plane's own space).
    airspeed: f32,
    /// Above the ground plane, in world units.
    pub altitude: f32,
    /// Visual roll into the turn. Cosmetic, but a plane that turns flat looks
    /// like a car with wings.
    bank: f32,
}

/// On the aircraft being flown, pointing at its pilot.
#[derive(Component)]
struct Pilot(Entity);

/// On a hero at the controls.
#[derive(Component)]
pub struct Flying {
    pub aircraft: Entity,
    /// Where they were standing when they got in.
    exit_side: Vec2,
}

/// Blocks getting straight back in the frame after stepping out.
#[derive(Component)]
struct JustLanded(Timer);

pub struct FlyPlugin;

impl Plugin for FlyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (tick_dismount, board, airborne)
                .chain()
                .run_if(in_state(GameState::Playing))
                .run_if(super::net::authoritative),
        );
    }
}

/// Mark an aircraft as flyable, and give it the ground collision a parked
/// aircraft should have. Called by a level when it puts one on an apron.
pub fn make_flyable(commands: &mut Commands, aircraft: Entity) {
    commands.entity(aircraft).insert((
        Plane::default(),
        Obstacle {
            radius: PARKED_RADIUS,
        },
        RunEntity,
    ));
}

fn tick_dismount(
    mut commands: Commands,
    time: Res<Time>,
    mut landed: Query<(Entity, &mut JustLanded)>,
) {
    for (e, mut timer) in &mut landed {
        if timer.0.tick(time.delta()).is_finished() {
            commands.entity(e).remove::<JustLanded>();
        }
    }
}

/// `E` to climb in, `E` to climb out — but only once you are back on the
/// ground and nearly stopped. Stepping out at altitude would drop a hero out
/// of the sky, and there is no parachute.
#[allow(clippy::type_complexity)]
fn board(
    mut commands: Commands,
    mut heroes: Query<
        (
            Entity,
            &mut Transform,
            &Intent,
            Option<&Flying>,
            Option<&JustLanded>,
        ),
        With<Player>,
    >,
    mut craft: Query<(Entity, &Transform, &mut Plane, Option<&Pilot>), Without<Player>>,
) {
    for (hero, mut transform, intent, flying, just_landed) in &mut heroes {
        if !intent.interact {
            continue;
        }
        match flying {
            Some(seat) => {
                let Ok((_, craft_transform, mut aircraft, _)) = craft.get_mut(seat.aircraft) else {
                    continue;
                };
                // Refuse to open the door in mid-air.
                if aircraft.altitude > GROUNDED || aircraft.airspeed.abs() > 6.0 {
                    continue;
                }
                aircraft.airspeed = 0.0;
                aircraft.throttle = 0.0;
                commands
                    .entity(seat.aircraft)
                    .remove::<Pilot>()
                    .insert(Obstacle {
                        radius: PARKED_RADIUS,
                    });
                let side = if seat.exit_side == Vec2::ZERO {
                    Vec2::X
                } else {
                    seat.exit_side
                };
                let at = plane(craft_transform.translation) + side * (PARKED_RADIUS + 3.0);
                transform.translation = ground(at, transform.translation.y);
                commands
                    .entity(hero)
                    .remove::<Flying>()
                    .insert(JustLanded(Timer::from_seconds(0.6, TimerMode::Once)));
            }
            None if just_landed.is_none() => {
                let here = plane(transform.translation);
                let nearest = craft
                    .iter()
                    .filter(|(_, _, _, pilot)| pilot.is_none())
                    .map(|(e, t, ..)| (e, plane(t.translation)))
                    .filter(|(_, p)| p.distance(here) <= REACH)
                    .min_by(|a, b| {
                        a.1.distance_squared(here)
                            .total_cmp(&b.1.distance_squared(here))
                    });
                let Some((aircraft, at)) = nearest else {
                    continue;
                };
                commands
                    .entity(aircraft)
                    .remove::<Obstacle>()
                    .insert(Pilot(hero));
                commands.entity(hero).insert(Flying {
                    aircraft,
                    exit_side: (here - at).normalize_or_zero(),
                });
            }
            None => {}
        }
    }
}

/// Fly it, and carry the pilot along.
#[allow(clippy::type_complexity)]
fn airborne(
    time: Res<Time>,
    level: Res<CurrentLevel>,
    mut set: ParamSet<(
        Query<(&Intent, &Flying)>,
        Query<(&mut Transform, &mut Plane, &Pilot)>,
        Query<&mut Transform, With<Player>>,
    )>,
) {
    let dt = time.delta_secs();

    // What the pilot is asking for, keyed by aircraft.
    let commands_in: Vec<(Entity, Vec2, f32)> = set
        .p0()
        .iter()
        .map(|(intent, flying)| (flying.aircraft, intent.move_dir, intent.climb))
        .collect();
    if commands_in.is_empty() {
        return;
    }

    let mut seats: Vec<(Entity, Vec3, Quat)> = Vec::new();
    for (aircraft, input, stick) in commands_in {
        let mut planes = set.p1();
        let Ok((mut transform, mut craft, pilot)) = planes.get_mut(aircraft) else {
            continue;
        };

        // ---- Throttle and airspeed.
        craft.throttle = (craft.throttle + input.y * dt * 0.9).clamp(0.0, 1.0);
        let target = craft.throttle * TOP_SPEED;
        let rate = if target > craft.airspeed { ACCEL } else { DRAG };
        craft.airspeed += (target - craft.airspeed).clamp(-rate * dt, rate * dt);
        craft.airspeed = craft.airspeed.max(0.0);

        // ---- Turning. A plane turns by going somewhere, so the rate scales
        // with how fast it is actually moving.
        let authority = (craft.airspeed / TAKEOFF).clamp(0.0, 1.4);
        let yaw_step = -input.x * TURN_RATE * authority * dt;
        transform.rotate_y(yaw_step);
        // Roll into the turn and level out of it.
        let want_bank = -input.x * 0.5 * authority;
        craft.bank += (want_bank - craft.bank) * (1.0 - (-4.0 * dt).exp());

        // ---- Lift. Below flying speed the wings do nothing and you sink.
        let flying_speed = craft.airspeed >= TAKEOFF;
        let vertical = if flying_speed {
            stick * CLIMB_RATE
        } else if craft.altitude > 0.0 {
            -SINK_RATE
        } else {
            0.0
        };
        craft.altitude = (craft.altitude + vertical * dt).clamp(0.0, CEILING);

        // ---- Travel. Nose is +Z in the plane's own space, same as the cars.
        let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        let nose = Vec2::new(yaw.sin(), -yaw.cos());
        let mut at = plane(transform.translation) + nose * craft.airspeed * dt;
        let bound = level.arena - Vec2::splat(4.0);
        at = at.clamp(-bound, bound);

        transform.translation = ground(at, craft.altitude);
        // Pitch with the climb, and roll with the bank — both cosmetic, both
        // the difference between flying and sliding around at altitude.
        let pitch = if flying_speed { stick * 0.22 } else { -0.16 };
        transform.rotation = Quat::from_rotation_y(yaw)
            * Quat::from_rotation_x(pitch)
            * Quat::from_rotation_z(craft.bank);

        if (time.elapsed_secs() * 2.0).fract() < time.delta_secs() * 2.0 {
            debug!(
                "flight: throttle {:.2} airspeed {:.1} altitude {:.1}",
                craft.throttle, craft.airspeed, craft.altitude
            );
        }
        seats.push((
            pilot.0,
            transform.translation + transform.rotation * SEAT,
            transform.rotation,
        ));
    }

    let mut players = set.p2();
    for (pilot, at, rot) in seats {
        if let Ok(mut t) = players.get_mut(pilot) {
            t.translation = at;
            t.rotation = rot;
        }
    }
}

/// The altitude the camera should allow for — the highest any hero currently
/// is. Zero when everyone is on foot.
pub fn camera_lift(altitude: f32) -> f32 {
    // Pull back as well as up, or a plane at the ceiling fills the frame.
    altitude * 0.85 + scale::m(0.0)
}
