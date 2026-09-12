//! The drummer's kit — the co-op second hero's weapon set, deliberately unlike
//! the guitarist's aimed ranged picks:
//!
//! * **Beat pulses** — a steady metronome ([`Beat`]) fires a small shockwave
//!   ring off the drummer every beat, a bigger one on the downbeat. Passive
//!   area denial that rewards standing in the thick of it. (No audio yet — a
//!   silent click track until M3 brings stems.)
//! * **Drumstick melee** — [`Intent::attack`] swings the sticks: a short
//!   forward arc that hits hard with knockback, on a tight cooldown.
//! * **Beat slam** — [`Intent::special`] slams a full-size shockwave on demand,
//!   like the guitarist's Encore, on a longer cooldown.

use std::time::Duration;

use bevy::math::primitives::CircularSector;
use bevy::prelude::*;

use super::audio::AudioCue;
use super::player::{Aim, Intent};
use super::{Downed, Enemy, GameState, Health, Hero, Player, RunEntity, ground, plane};

/// Beats per minute of the drummer's click track.
const BPM: f32 = 124.0;
const BEAT_SECS: f32 = 60.0 / BPM;

const PULSE_DAMAGE: f32 = 7.0;
const PULSE_RADIUS: f32 = 4.2;
const DOWNBEAT_DAMAGE: f32 = 13.0;
const DOWNBEAT_RADIUS: f32 = 6.5;

const SLAM_DAMAGE: f32 = 55.0;
const SLAM_RADIUS: f32 = 15.0;
const SLAM_KNOCK: f32 = 5.0;
const SLAM_COOLDOWN: f32 = 3.2;

const MELEE_DAMAGE: f32 = 42.0;
const MELEE_RANGE: f32 = 3.6;
/// Cosine of the half-angle of the swing arc (~70°).
const MELEE_ARC_COS: f32 = 0.34;
const MELEE_KNOCK: f32 = 3.5;
const MELEE_ACTIVE: f32 = 0.34;
const MELEE_COOLDOWN: f32 = 0.52;

/// The drummer's whole action state, kept on the drummer entity for its whole
/// life so the animation driver and the systems below read it without
/// insert/remove churn.
#[derive(Component)]
pub struct DrumKit {
    /// `Some` while a swing is playing out — the arc is live for hit checks.
    swing: Option<Timer>,
    swing_cd: Timer,
    swing_hit: Vec<Entity>,
    slam_cd: Timer,
}

impl Default for DrumKit {
    fn default() -> Self {
        let done = |secs: f32| {
            let mut t = Timer::from_seconds(secs, TimerMode::Once);
            t.tick(Duration::from_secs_f32(secs));
            t
        };
        Self {
            swing: None,
            swing_cd: done(MELEE_COOLDOWN),
            swing_hit: Vec::new(),
            slam_cd: done(SLAM_COOLDOWN),
        }
    }
}

impl DrumKit {
    /// True while a drumstick swing is mid-animation — read by
    /// `player::drive_animation` to pick the melee gait.
    pub fn is_swinging(&self) -> bool {
        self.swing.is_some()
    }
}

/// The click track. `count` increments every beat; every 4th is a downbeat.
#[derive(Resource)]
struct Beat {
    timer: Timer,
    count: u32,
}

impl Default for Beat {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(BEAT_SECS, TimerMode::Repeating),
            count: 0,
        }
    }
}

/// An expanding damage ring — shared by the beat pulses and the beat slam.
#[derive(Component)]
struct Shockwave {
    radius: f32,
    max: f32,
    speed: f32,
    damage: f32,
    knock: f32,
    hit: Vec<Entity>,
}

/// A short-lived translucent wedge that reads the drumstick swing from the
/// top-down camera. World-space (not parented) — a 0.34 s snapshot.
#[derive(Component)]
struct SwingArc(Timer);

#[derive(Resource)]
struct DrumAssets {
    ring: Handle<Mesh>,
    pulse_mat: Handle<StandardMaterial>,
    slam_mat: Handle<StandardMaterial>,
    arc: Handle<Mesh>,
    arc_mat: Handle<StandardMaterial>,
}

pub struct DrummerPlugin;

impl Plugin for DrummerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_drum_assets)
            .add_systems(OnEnter(GameState::Playing), |mut commands: Commands| {
                commands.insert_resource(Beat::default());
            })
            .add_systems(
                Update,
                (
                    beat_pulses,
                    drummer_actions,
                    tick_swing_arcs,
                    expand_shockwaves,
                )
                    .run_if(in_state(GameState::Playing))
                    .run_if(super::net::authoritative),
            );
    }
}

fn load_drum_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let lit = |r: f32, g: f32, b: f32| StandardMaterial {
        base_color: Color::srgb(1.0, 0.8, 0.55),
        emissive: LinearRgba::rgb(r, g, b),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    };
    commands.insert_resource(DrumAssets {
        ring: meshes.add(Torus {
            minor_radius: 0.06,
            major_radius: 1.0,
        }),
        pulse_mat: materials.add(lit(5.5, 2.6, 0.7)),
        slam_mat: materials.add(lit(8.0, 3.4, 0.9)),
        arc: meshes.add(CircularSector::from_radians(MELEE_RANGE, 1.4)),
        arc_mat: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.7, 0.3, 0.5),
            emissive: LinearRgba::rgb(3.0, 1.4, 0.4),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        }),
    });
}

/// Fire a shockwave off the drummer on every beat — a fat one on the downbeat.
fn beat_pulses(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<DrumAssets>,
    mut beat: ResMut<Beat>,
    mut cues: MessageWriter<AudioCue>,
    drummer: Query<(&Transform, &Hero), (With<Player>, Without<Downed>)>,
) {
    if !beat.timer.tick(time.delta()).just_finished() {
        return;
    }
    beat.count = beat.count.wrapping_add(1);
    let Some((t, _)) = drummer.iter().find(|(_, h)| **h == Hero::Drummer) else {
        return;
    };
    let downbeat = beat.count % 4 == 0;
    cues.write(AudioCue::Beat);
    let (radius, damage, mat) = if downbeat {
        (DOWNBEAT_RADIUS, DOWNBEAT_DAMAGE, art.slam_mat.clone())
    } else {
        (PULSE_RADIUS, PULSE_DAMAGE, art.pulse_mat.clone())
    };
    spawn_shockwave(
        &mut commands,
        &art,
        plane(t.translation),
        radius,
        damage,
        1.4,
        mat,
    );
}

/// Drumstick swings and the on-demand beat slam.
#[allow(clippy::type_complexity)]
fn drummer_actions(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<DrumAssets>,
    mut cues: MessageWriter<AudioCue>,
    mut drummer: Query<
        (&Transform, &Aim, &Intent, &mut DrumKit, &Hero),
        (With<Player>, Without<Downed>),
    >,
    mut enemies: Query<(Entity, &mut Transform, &mut Health), (With<Enemy>, Without<Player>)>,
) {
    let Some((t, aim, intent, mut kit, _)) =
        drummer.iter_mut().find(|(.., h)| **h == Hero::Drummer)
    else {
        return;
    };
    let ppos = plane(t.translation);
    let face = plane(aim.0).normalize_or_zero();

    kit.swing_cd.tick(time.delta());
    kit.slam_cd.tick(time.delta());

    // --- Beat slam ---
    if intent.special && kit.slam_cd.is_finished() {
        kit.slam_cd.reset();
        cues.write(AudioCue::ChordStab);
        spawn_shockwave(
            &mut commands,
            &art,
            ppos,
            SLAM_RADIUS,
            SLAM_DAMAGE,
            SLAM_KNOCK,
            art.slam_mat.clone(),
        );
    }

    // --- Drumstick swing ---
    if intent.attack && kit.swing.is_none() && kit.swing_cd.is_finished() && face != Vec2::ZERO {
        kit.swing = Some(Timer::from_seconds(MELEE_ACTIVE, TimerMode::Once));
        kit.swing_hit.clear();
        kit.swing_cd = Timer::from_seconds(MELEE_COOLDOWN, TimerMode::Once);
        // Lay the sector flat (mesh XY → world XZ, mesh +Y → world -Z), then
        // yaw it about world-Y to point its centreline down the aim.
        let yaw = f32::atan2(-face.x, face.y);
        commands.spawn((
            SwingArc(Timer::from_seconds(MELEE_ACTIVE, TimerMode::Once)),
            Mesh3d(art.arc.clone()),
            MeshMaterial3d(art.arc_mat.clone()),
            Transform::from_translation(ground(ppos, 0.35)).with_rotation(
                Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
            ),
            RunEntity,
        ));
    }

    if let Some(mut swing) = kit.swing.take() {
        swing.tick(time.delta());
        let finished = swing.is_finished();
        if face != Vec2::ZERO {
            for (e, mut et, mut hp) in &mut enemies {
                if kit.swing_hit.contains(&e) {
                    continue;
                }
                let to = plane(et.translation) - ppos;
                let dist = to.length();
                if dist > MELEE_RANGE || dist < 1e-3 || (to / dist).dot(face) < MELEE_ARC_COS {
                    continue;
                }
                hp.current -= MELEE_DAMAGE;
                cues.write(AudioCue::Hit);
                kit.swing_hit.push(e);
                et.translation += ground((to / dist) * MELEE_KNOCK, 0.0);
            }
        }
        if !finished {
            kit.swing = Some(swing);
        }
    }
}

fn spawn_shockwave(
    commands: &mut Commands,
    art: &DrumAssets,
    at: Vec2,
    max: f32,
    damage: f32,
    knock: f32,
    mat: Handle<StandardMaterial>,
) {
    let start = 0.8;
    commands.spawn((
        Shockwave {
            radius: start,
            max,
            speed: (max / 0.5).max(16.0),
            damage,
            knock,
            hit: Vec::new(),
        },
        Mesh3d(art.ring.clone()),
        MeshMaterial3d(mat),
        Transform::from_translation(ground(at, 0.5)).with_scale(Vec3::splat(start)),
        Visibility::default(),
        RunEntity,
    ));
}

fn expand_shockwaves(
    mut commands: Commands,
    time: Res<Time>,
    mut cues: MessageWriter<AudioCue>,
    mut waves: Query<(Entity, &mut Shockwave, &mut Transform), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Health), With<Enemy>>,
) {
    let dt = time.delta_secs();
    for (e, mut wave, mut wt) in &mut waves {
        wave.radius += wave.speed * dt;
        wt.scale = Vec3::splat(wave.radius);
        let center = plane(wt.translation);

        for (enemy, mut et, mut hp) in &mut enemies {
            if wave.hit.contains(&enemy) {
                continue;
            }
            let to = plane(et.translation) - center;
            if (to.length() - wave.radius).abs() < 1.2 {
                hp.current -= wave.damage;
                cues.write(AudioCue::Hit);
                wave.hit.push(enemy);
                if wave.knock > 0.0 {
                    et.translation += ground(to.normalize_or_zero() * wave.knock, 0.0);
                }
            }
        }

        if wave.radius >= wave.max {
            commands.entity(e).despawn();
        }
    }
}

fn tick_swing_arcs(
    mut commands: Commands,
    time: Res<Time>,
    mut arcs: Query<(Entity, &mut SwingArc, &mut Transform)>,
) {
    for (e, mut arc, mut t) in &mut arcs {
        arc.0.tick(time.delta());
        let f = arc.0.fraction();
        t.scale = Vec3::splat(1.0 - 0.35 * f);
        if arc.0.is_finished() {
            commands.entity(e).despawn();
        }
    }
}
