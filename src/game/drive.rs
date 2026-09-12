//! **Cars you can get into.**
//!
//! The town was already full of parked cars, and they were pure scenery — a
//! circle of collision you walked around. Making them drivable costs almost
//! nothing (they are already models standing on the road) and changes how the
//! build phase plays: the loudspeaker pitches are spread across a town that
//! takes a while to cross on foot, so a car is the thing that lets you reach
//! the far one before the next wave lands.
//!
//! **The blocky rig drives; the skeletal rig rides out of sight.** Kenney's
//! blocky character ships a `drive` clip — a proper sit-with-hands-up pose —
//! and the Quaternius skeleton has nothing like it. Rather than sit a standing
//! man in a car seat, a skeletal hero's model is hidden while they drive: they
//! got in. That is a real difference between the two casts and worth knowing
//! about before you pick one.
//!
//! The car is a weapon too while you are in it — it flattens the swarm it
//! drives through — but the guitarist can still fire from the driving seat;
//! only building is off the table there.

use bevy::prelude::*;

use super::player::Intent;
use super::roster::Pick;
use super::{
    CurrentLevel, Enemy, GameState, Health, Hitbox, Obstacle, Player, ground, plane,
    resolve_obstacles,
};

/// Top speed, in units per second — a bit over twice a hero's run.
const TOP_SPEED: f32 = 26.0;
const REVERSE_SPEED: f32 = 9.0;
/// How fast the car gets to top speed, and how fast it sheds it.
const ACCEL: f32 = 22.0;
const BRAKE: f32 = 30.0;
/// Radians per second the nose can swing. Scaled by [`grip`] — brisk while
/// crawling (so you can point your way out of a jam), then car-like the faster
/// you go so a car at speed can't snap-turn.
const TURN_RATE: f32 = 2.7;
/// How close a hero has to be to get in.
pub const REACH: f32 = 5.0;
/// Ground radius the body sweeps — what it shoves out of the way and runs over.
const BODY: f32 = 2.4;
/// Where the driver sits, in the car's own space: up onto the seat and a touch
/// back from the nose. The blocky `drive` clip is a sitting pose whose hips are
/// still at the model's origin, so without the lift a driver rides with their
/// legs through the floor pan.
const SEAT: Vec3 = Vec3::new(0.0, 0.62, -0.10);
/// Damage a full-speed ram does. Scaled by how fast you are actually going, so
/// crawling into an enemy does nothing.
const RAM_DAMAGE: f32 = 130.0;

/// A car that can be driven. Sits on the same entity as the [`Obstacle`] the
/// parked car already was.
#[derive(Component)]
pub struct Car {
    /// Signed speed along the car's nose (+Z in its own space).
    speed: f32,
    /// The collision radius it had while parked, put back when you get out.
    parked_radius: f32,
}

impl Car {
    pub fn parked() -> Self {
        Self {
            speed: 0.0,
            parked_radius: 1.9,
        }
    }
}

/// On the car currently being driven, pointing at its driver.
#[derive(Component)]
struct Driver(Entity);

/// Just got out. Blocks getting straight back in, which otherwise happens the
/// instant you step out of a car you are still standing next to — a human
/// tapping the key doesn't notice, but anything holding it (the capture bot,
/// a gamepad with a sticky trigger) flips in and out every frame.
#[derive(Component)]
struct JustLeft(Timer);

/// On a hero who is behind the wheel.
#[derive(Component)]
pub struct Driving {
    pub car: Entity,
    /// Where they were standing when they got in, so getting out puts them
    /// beside the car rather than under it.
    exit_side: Vec2,
}

pub struct DrivePlugin;

impl Plugin for DrivePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (tick_dismount, get_in_or_out, steer, ram)
                .chain()
                .run_if(in_state(GameState::Playing))
                .run_if(super::net::authoritative),
        );
    }
}

fn tick_dismount(
    mut commands: Commands,
    time: Res<Time>,
    mut just_left: Query<(Entity, &mut JustLeft)>,
) {
    for (e, mut timer) in &mut just_left {
        if timer.0.tick(time.delta()).is_finished() {
            commands.entity(e).remove::<JustLeft>();
        }
    }
}

/// Press interact next to a car to get in; press it again to get out.
#[allow(clippy::type_complexity)]
fn get_in_or_out(
    mut commands: Commands,
    mut heroes: Query<
        (
            Entity,
            &mut Transform,
            &Intent,
            Option<&Driving>,
            Option<&JustLeft>,
        ),
        With<Player>,
    >,
    mut cars: Query<(Entity, &Transform, &mut Car, Option<&Driver>), Without<Player>>,
) {
    for (hero, mut transform, intent, driving, just_left) in &mut heroes {
        if !intent.interact {
            continue;
        }
        match driving {
            // Getting out: park it where it stands and step clear of the body.
            Some(seat) => {
                if let Ok((_, car_transform, mut car, _)) = cars.get_mut(seat.car) {
                    car.speed = 0.0;
                    commands.entity(seat.car).remove::<Driver>().insert(Obstacle {
                        radius: car.parked_radius,
                    });
                    // Step out on the side you got in from, clear of the body,
                    // so you aren't left standing inside your own car.
                    let side = if seat.exit_side == Vec2::ZERO {
                        Vec2::X
                    } else {
                        seat.exit_side
                    };
                    let at = plane(car_transform.translation) + side * (BODY + 3.4);
                    transform.translation = ground(at, transform.translation.y);
                }
                commands
                    .entity(hero)
                    .remove::<Driving>()
                    .insert(JustLeft(Timer::from_seconds(0.6, TimerMode::Once)));
            }
            // Getting in: nearest free car within reach.
            None if just_left.is_none() => {
                let here = plane(transform.translation);
                let nearest = cars
                    .iter()
                    .filter(|(_, _, _, driver)| driver.is_none())
                    .map(|(e, t, ..)| (e, plane(t.translation)))
                    .filter(|(_, p)| p.distance(here) <= REACH)
                    .min_by(|a, b| {
                        a.1.distance_squared(here)
                            .total_cmp(&b.1.distance_squared(here))
                    });
                let Some((car, at)) = nearest else { continue };
                // The car stops being something to walk around the moment it
                // becomes something you are inside.
                commands.entity(car).remove::<Obstacle>().insert(Driver(hero));
                commands.entity(hero).insert(Driving {
                    car,
                    exit_side: (here - at).normalize_or_zero(),
                });
            }
            None => {}
        }
    }
}

/// Drive whatever is being driven, and carry its driver along.
#[allow(clippy::type_complexity)]
fn steer(
    time: Res<Time>,
    level: Res<CurrentLevel>,
    obstacles: Query<(&Transform, &Obstacle), (Without<Player>, Without<Car>)>,
    mut set: ParamSet<(
        Query<(&Intent, &Driving)>,
        Query<(&mut Transform, &mut Car, &Driver)>,
        Query<&mut Transform, With<Player>>,
    )>,
) {
    let dt = time.delta_secs();
    let blocks: Vec<(Vec2, f32)> = obstacles
        .iter()
        .map(|(t, o)| (plane(t.translation), o.radius))
        .collect();

    // What each driver is asking for this frame.
    let orders: Vec<(Entity, Vec2)> = set
        .p0()
        .iter()
        .map(|(intent, seat)| (seat.car, intent.move_dir))
        .collect();

    let mut seats: Vec<(Entity, Vec3, Quat)> = Vec::new();
    for (car_entity, want) in orders {
        let mut cars = set.p1();
        let Ok((mut transform, mut car, driver)) = cars.get_mut(car_entity) else {
            continue;
        };

        // Everything here is world space: the car's nose is its own +Z, and a
        // yaw of `atan2(x, z)` is what points +Z at a direction — the same
        // convention `weapon::fire` uses to point a shot down its travel.
        let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        let nose = |yaw: f32| Vec3::new(yaw.sin(), 0.0, yaw.cos());

        if want == Vec2::ZERO {
            // Off the throttle: coast down rather than stopping dead.
            car.speed -= car.speed.signum() * BRAKE * 0.45 * dt;
            if car.speed.abs() < 0.4 {
                car.speed = 0.0;
            }
        } else {
            let want_dir = ground(want.normalize_or_zero(), 0.0);
            let along = want_dir.dot(nose(yaw));
            if along >= -0.35 {
                // Anything that isn't a hard reverse is forward: an arcade car
                // you point rather than a gearbox you operate.
                car.speed = (car.speed + ACCEL * dt).min(TOP_SPEED);
            } else {
                car.speed = (car.speed - ACCEL * dt).max(-REVERSE_SPEED);
            }

            // Swing the nose toward where you're pushing. A car that is barely
            // moving barely turns, which is what stops it spinning on the spot.
            let want_yaw = f32::atan2(want_dir.x, want_dir.z);
            let mut delta = (want_yaw - yaw + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            // Reversing steers the way a reversing car does: mirrored.
            if car.speed < 0.0 {
                delta = -delta;
            }
            let step = (TURN_RATE * grip(car.speed) * dt).min(delta.abs());
            transform.rotate_y(delta.signum() * step);
        }

        let yaw = transform.rotation.to_euler(EulerRot::YXZ).0;
        let travel = nose(yaw) * car.speed * dt;
        let mut at = plane(transform.translation + travel);
        let bound = level.arena - Vec2::splat(3.0);
        at = at.clamp(-bound, bound);
        let resolved = resolve_obstacles(at, BODY, &blocks);
        if resolved != at {
            // Hit something solid: shed most of the speed rather than grinding
            // along the wall at full tilt.
            car.speed *= 0.25;
        }
        transform.translation = ground(resolved, transform.translation.y);
        seats.push((
            driver.0,
            transform.translation + transform.rotation * SEAT,
            transform.rotation,
        ));
    }

    // Glue each driver to their seat. The hero keeps their own entity — only
    // the transform is borrowed — so health, ring and HUD all carry on working.
    let mut players = set.p2();
    for (hero, at, rot) in seats {
        if let Ok(mut t) = players.get_mut(hero) {
            t.translation = at;
            t.rotation = rot;
        }
    }
}

/// How much of [`TURN_RATE`] the nose gets at a given signed speed.
///
/// Near a standstill the car turns almost at full rate, so pushing away from an
/// obstacle points it clear in about a second instead of grinding along. As it
/// picks up speed the rate falls back toward "tracks with how fast you're
/// going", which keeps a car at pace from spinning on a flick of the stick.
fn grip(speed: f32) -> f32 {
    let frac = (speed.abs() / TOP_SPEED).clamp(0.0, 1.0);
    let crawl = (1.0 - frac / 0.35).clamp(0.0, 1.0);
    frac.max(0.85 * crawl).max(0.12)
}

/// A moving car flattens the swarm it drives through.
fn ram(
    cars: Query<(&Transform, &Car), With<Driver>>,
    mut enemies: Query<(&Transform, &Hitbox, &mut Health), With<Enemy>>,
) {
    for (car_transform, car) in &cars {
        let speed = car.speed.abs();
        if speed < 6.0 {
            continue;
        }
        let at = plane(car_transform.translation);
        let force = RAM_DAMAGE * (speed / TOP_SPEED);
        for (enemy_transform, hitbox, mut health) in &mut enemies {
            if plane(enemy_transform.translation).distance(at) < BODY + hitbox.0 {
                health.current -= force;
            }
        }
    }
}

/// Should this character be visible in the driving seat?
///
/// Only the blocky rig has a sitting pose; a skeletal hero would be stood
/// bolt upright through the roof, so they are hidden instead.
pub fn sits_visibly(pick: Pick) -> bool {
    matches!(pick.rig(), super::props::Rig::Boxy)
}
