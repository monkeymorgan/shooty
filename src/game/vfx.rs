//! Hit-and-run visual effects. Right now: the **death pop** — when a baddie goes
//! down it bursts into a shower of chunky cubes with a bright flash and an
//! expanding ground ring, à la BAAM SQUAD. Fired as an [`Explosion`] message so
//! any system can trigger one; `combat::check_death` is the current caller.

use std::f32::consts::FRAC_PI_2;

use bevy::prelude::*;

use super::{EnemyKind, GameState, Lifetime, RunEntity, ground, plane};

/// Request a death pop at `pos`, themed to `kind`. `power` scales the burst
/// (shard count, spread, ring size) — 1.0 for a grunt, ~1.8 for a bruiser.
#[derive(Message, Clone, Copy)]
pub struct Explosion {
    pub pos: Vec3,
    pub kind: EnemyKind,
    pub power: f32,
}

/// A flying cube chunk: ballistic, tumbling, gone when its `Lifetime` ends.
#[derive(Component)]
struct Shard {
    vel: Vec3,
    spin: Vec3,
}

/// Uniform scale-up per second (the flash and the ground ring bloom outward).
#[derive(Component)]
struct Grow(f32);

/// Shared meshes + per-kind materials so a burst allocates nothing.
#[derive(Resource)]
struct VfxAssets {
    cube: Handle<Mesh>,
    flash: Handle<Mesh>,
    ring: Handle<Mesh>,
    shard_mat: [Handle<StandardMaterial>; 4],
    ring_mat: [Handle<StandardMaterial>; 4],
    flash_mat: Handle<StandardMaterial>,
}

/// Body/aura hue per enemy kind — the shards and ring take these so a green
/// rusher pops green, the grey gloom-folk pop ashen.
fn kind_rgb(kind: EnemyKind) -> LinearRgba {
    // A dark mood dissipating — ashen violets and a dark rose, not the old
    // acid-green confetti.
    match kind {
        EnemyKind::Mood => LinearRgba::rgb(0.34, 0.28, 0.48),
        EnemyKind::Head => LinearRgba::rgb(0.56, 0.30, 0.66),
        EnemyKind::Heckler => LinearRgba::rgb(0.62, 0.20, 0.36),
        EnemyKind::Sink => LinearRgba::rgb(0.24, 0.22, 0.34),
    }
}

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Explosion>()
            .add_systems(Startup, setup_vfx_assets)
            .add_systems(
                Update,
                (spawn_explosions, drive_shards, drive_grow).run_if(in_state(GameState::Playing)),
            );
    }
}

fn setup_vfx_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let kinds = [
        EnemyKind::Mood,
        EnemyKind::Head,
        EnemyKind::Heckler,
        EnemyKind::Sink,
    ];
    let shard_mat = kinds.map(|k| {
        let c = kind_rgb(k);
        materials.add(StandardMaterial {
            base_color: Color::linear_rgb(c.red * 0.7, c.green * 0.7, c.blue * 0.7),
            emissive: LinearRgba::new(c.red * 0.6, c.green * 0.6, c.blue * 0.6, 1.0),
            perceptual_roughness: 0.9,
            ..default()
        })
    });
    let ring_mat = kinds.map(|k| {
        let c = kind_rgb(k);
        materials.add(StandardMaterial {
            base_color: Color::linear_rgb(c.red, c.green, c.blue),
            emissive: LinearRgba::new(c.red * 4.0, c.green * 4.0, c.blue * 4.0, 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        })
    });
    commands.insert_resource(VfxAssets {
        cube: meshes.add(Cuboid::from_length(0.26)),
        flash: meshes.add(Sphere::new(0.5)),
        ring: meshes.add(Annulus::new(0.62, 0.92)),
        shard_mat,
        ring_mat,
        flash_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.96, 0.85),
            emissive: LinearRgba::rgb(5.0, 4.4, 3.0),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        }),
    });
}

fn spawn_explosions(
    mut commands: Commands,
    mut ev: MessageReader<Explosion>,
    vfx: Option<Res<VfxAssets>>,
    mut seed: Local<u32>,
) {
    let Some(vfx) = vfx else {
        ev.clear();
        return;
    };
    // cheap deterministic-ish RNG — good enough for confetti
    let mut rand = || {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*seed >> 8) as f32 / (1 << 24) as f32
    };

    for &Explosion { pos, kind, power } in ev.read() {
        let ki = kind as usize;
        let ground_pos = ground(plane(pos), 0.05);
        let centre = ground(plane(pos), 0.9);
        let n = (10.0 * power).round() as usize + 4;

        for _ in 0..n {
            let dir = Vec3::new(rand() * 2.0 - 1.0, rand() * 1.4 + 0.2, rand() * 2.0 - 1.0)
                .normalize_or_zero();
            let speed = (4.0 + rand() * 7.0) * power;
            let s = 0.6 + rand() * 0.9;
            commands.spawn((
                Mesh3d(vfx.cube.clone()),
                MeshMaterial3d(vfx.shard_mat[ki].clone()),
                Transform::from_translation(centre).with_scale(Vec3::splat(s)),
                Shard {
                    vel: dir * speed,
                    spin: Vec3::new(rand() * 2.0 - 1.0, rand() * 2.0 - 1.0, rand() * 2.0 - 1.0)
                        * 12.0,
                },
                Lifetime(Timer::from_seconds(0.5 + rand() * 0.4, TimerMode::Once)),
                RunEntity,
            ));
        }

        // white core flash
        commands.spawn((
            Mesh3d(vfx.flash.clone()),
            MeshMaterial3d(vfx.flash_mat.clone()),
            Transform::from_translation(centre).with_scale(Vec3::splat(0.5 * power)),
            Grow(9.0),
            Lifetime(Timer::from_seconds(0.14, TimerMode::Once)),
            RunEntity,
        ));
        // expanding ground ring
        commands.spawn((
            Mesh3d(vfx.ring.clone()),
            MeshMaterial3d(vfx.ring_mat[ki].clone()),
            Transform::from_translation(ground_pos)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                .with_scale(Vec3::splat(0.4 * power)),
            Grow(11.0),
            Lifetime(Timer::from_seconds(0.32, TimerMode::Once)),
            RunEntity,
        ));
        // muzzle-style light punch
        commands.spawn((
            PointLight {
                color: {
                    let c = kind_rgb(kind);
                    Color::linear_rgb(c.red, c.green, c.blue)
                },
                intensity: 900_000.0 * power,
                range: 12.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(centre),
            Lifetime(Timer::from_seconds(0.12, TimerMode::Once)),
            RunEntity,
        ));
    }
}

fn drive_shards(time: Res<Time>, mut q: Query<(&mut Transform, &mut Shard)>) {
    let dt = time.delta_secs();
    for (mut t, mut sh) in &mut q {
        sh.vel.y -= 24.0 * dt;
        t.translation += sh.vel * dt;
        if t.translation.y < 0.13 {
            t.translation.y = 0.13;
            sh.vel.x *= 0.6;
            sh.vel.z *= 0.6;
            sh.vel.y = sh.vel.y.max(0.0);
            sh.spin *= 0.7;
        }
        t.rotation *= Quat::from_scaled_axis(sh.spin * dt);
    }
}

fn drive_grow(time: Res<Time>, mut q: Query<(&mut Transform, &Grow)>) {
    let dt = time.delta_secs();
    for (mut t, g) in &mut q {
        let f = 1.0 + g.0 * dt;
        t.scale *= f;
    }
}
