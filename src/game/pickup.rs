//! Timed / dropped pickups and the temporary upgrades they grant. Each is a
//! chunky low-poly music-gear model (see `ART.md` — royal-blue body, emissive
//! cyan screens, red/teal controls):
//!
//! * **Boombox** → *Dual guitar* — fire a pick from *both* hands for a while.
//! * **Amp** → *Encore* — an on-demand shockwave blast (RMB), like a band
//!   slamming the final chord of a set.
//! * **Synth** → *Arpeggio* — rapid fire for a while.

use bevy::prelude::*;
use rand::Rng;

use super::player::{GuitarGun, Intent, spawn_guitar_gun};
use super::{
    CurrentLevel, Enemy, EnemyKind, GameState, Health, Hero, Hitbox, Player, RunEntity, ground,
    plane,
};

const BUFF_SECS: f32 = 12.0;
const PICKUP_REACH: f32 = 1.8;
const WAVE_SPEED: f32 = 20.0;
const WAVE_MAX: f32 = 14.0;
const WAVE_DAMAGE: f32 = 60.0;
const WAVE_KNOCK: f32 = 4.0;
const ENCORE_COOLDOWN: f32 = 4.0;

#[derive(Clone, Copy, PartialEq)]
pub enum PickupKind {
    DualGuitar,
    Encore,
    Arpeggio,
}

impl PickupKind {
    const ALL: [PickupKind; 3] = [
        PickupKind::Encore,
        PickupKind::DualGuitar,
        PickupKind::Arpeggio,
    ];
}

#[derive(Component)]
pub struct Pickup(pub PickupKind);

/// Bob + spin animation state for a floating pickup.
#[derive(Component)]
struct Float(f32);

/// On the player: fire from both hands until the timer runs out.
#[derive(Component)]
pub struct DualWield(pub Timer);

/// On the player: fire rate is boosted until the timer runs out (Synth pickup).
#[derive(Component)]
pub struct RapidFire(pub Timer);

/// On the player: the Encore blast is available (window) on a cooldown.
#[derive(Component)]
pub struct Encore {
    window: Timer,
    cooldown: Timer,
}

/// Expanding shockwave ring spawned by an Encore blast.
#[derive(Component)]
struct SoundWave {
    radius: f32,
    hit: Vec<Entity>,
}

/// Sent by `combat` when an enemy dies — a pickup can occasionally drop
/// (`drop_from_kills`) and `build` mints scrap from it (`build::drop_scrap`).
#[derive(Message)]
pub struct EnemyDied {
    pub pos: Vec3,
    pub kind: EnemyKind,
}

#[derive(Resource)]
struct PickupAssets {
    /// Unit cube, unit cylinder (axis = Y) — every model part is one of these
    /// scaled into a box or a disc.
    cube: Handle<Mesh>,
    disc: Handle<Mesh>,
    blue: Handle<StandardMaterial>,
    dark: Handle<StandardMaterial>,
    cream: Handle<StandardMaterial>,
    metal: Handle<StandardMaterial>,
    glow: Handle<StandardMaterial>,
    red: Handle<StandardMaterial>,
    teal: Handle<StandardMaterial>,
    wave: Handle<Mesh>,
    wave_mat: Handle<StandardMaterial>,
}

type Part = (Handle<Mesh>, Handle<StandardMaterial>, Transform);

/// A box-shaped part: unit cube translated to `pos`, scaled to `size`.
fn boxy(art: &PickupAssets, mat: &Handle<StandardMaterial>, pos: Vec3, size: Vec3) -> Part {
    (
        art.cube.clone(),
        mat.clone(),
        Transform::from_translation(pos).with_scale(size),
    )
}

/// A disc facing `+Z` (the unit Y-cylinder tipped forward): `size` is
/// (width, thickness, height).
fn disc(art: &PickupAssets, mat: &Handle<StandardMaterial>, pos: Vec3, size: Vec3) -> Part {
    (
        art.disc.clone(),
        mat.clone(),
        Transform::from_translation(pos)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_scale(size),
    )
}

/// An upright knob (unit Y-cylinder, `size` = (width, height, depth)).
fn knob(art: &PickupAssets, mat: &Handle<StandardMaterial>, pos: Vec3, r: f32) -> Part {
    (
        art.disc.clone(),
        mat.clone(),
        Transform::from_translation(pos).with_scale(Vec3::new(r, r * 0.9, r)),
    )
}

#[derive(Resource)]
struct PickupTimer(Timer);

pub struct PickupPlugin;

impl Plugin for PickupPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemyDied>()
            .add_systems(Startup, load_pickup_assets)
            .add_systems(OnEnter(GameState::Playing), |mut commands: Commands| {
                commands.insert_resource(PickupTimer(Timer::from_seconds(2.8, TimerMode::Once)));
            })
            .add_systems(
                Update,
                (
                    float_pickups,
                    spawn_timed_pickup,
                    drop_from_kills,
                    collect_pickups,
                    manage_dual_gun,
                    encore_blast,
                    expand_sound_waves,
                    expire_buffs,
                )
                    .run_if(in_state(GameState::Playing))
                    .run_if(super::net::authoritative),
            );
    }
}

fn load_pickup_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let solid = |r: f32, g: f32, b: f32, rough: f32| StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: rough,
        ..default()
    };
    let lit = |r: f32, g: f32, b: f32| StandardMaterial {
        base_color: Color::srgb(0.75, 0.98, 1.0),
        emissive: LinearRgba::rgb(r, g, b),
        unlit: true,
        ..default()
    };
    commands.insert_resource(PickupAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        disc: meshes.add(Cylinder::new(0.5, 1.0)),
        blue: materials.add(solid(0.14, 0.34, 0.78, 0.55)),
        dark: materials.add(solid(0.06, 0.06, 0.08, 0.6)),
        cream: materials.add(solid(0.90, 0.88, 0.80, 0.7)),
        metal: materials.add(solid(0.32, 0.34, 0.40, 0.35)),
        glow: materials.add(lit(0.6, 4.2, 5.4)),
        red: materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.22, 0.26),
            emissive: LinearRgba::rgb(0.5, 0.05, 0.06),
            ..default()
        }),
        teal: materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.78, 0.72),
            emissive: LinearRgba::rgb(0.05, 0.4, 0.36),
            ..default()
        }),
        wave: meshes.add(Torus {
            minor_radius: 0.055,
            major_radius: 1.0,
        }),
        wave_mat: materials.add(lit(3.5, 7.5, 8.5)),
    });
}

// The pickups read from a distant top-down camera, so each leans on one bold
// silhouette cue: the amp is the tall box with a bright face, the boombox has a
// chunky handle arch over the top, the synth is a low wedge with a white keybed
// along its front edge.

/// Guitar amp — the Encore blast.
fn amp_parts(a: &PickupAssets) -> Vec<Part> {
    let mut v = vec![
        // tall cabinet + blue base plinth
        boxy(
            a,
            &a.dark,
            Vec3::new(0.0, 0.1, 0.0),
            Vec3::new(3.0, 2.8, 1.7),
        ),
        boxy(
            a,
            &a.blue,
            Vec3::new(0.0, -1.35, 0.0),
            Vec3::new(3.2, 0.4, 1.9),
        ),
        // full-height front grille + bright brand bar across it
        boxy(
            a,
            &a.metal,
            Vec3::new(0.0, 0.0, 0.9),
            Vec3::new(2.5, 2.1, 0.12),
        ),
        boxy(
            a,
            &a.glow,
            Vec3::new(0.0, 1.15, 0.95),
            Vec3::new(2.4, 0.42, 0.08),
        ),
        // top control ledge
        boxy(
            a,
            &a.dark,
            Vec3::new(0.0, 1.6, 0.25),
            Vec3::new(3.0, 0.3, 1.0),
        ),
    ];
    for x in [-0.9, 0.0, 0.9] {
        v.push(disc(
            a,
            &a.dark,
            Vec3::new(x, -0.35, 0.98),
            Vec3::new(0.95, 0.16, 0.95),
        ));
        v.push(disc(
            a,
            &a.glow,
            Vec3::new(x, -0.35, 1.02),
            Vec3::new(0.3, 0.16, 0.3),
        ));
    }
    for (i, x) in [-1.0, -0.35, 0.35, 1.0].into_iter().enumerate() {
        let m = if i % 2 == 0 { &a.teal } else { &a.red };
        v.push(knob(a, m, Vec3::new(x, 1.78, 0.2), 0.18));
    }
    v
}

/// Boombox — Dual guitar. The handle arch is the top-down tell.
fn boombox_parts(a: &PickupAssets) -> Vec<Part> {
    let mut v = vec![
        boxy(a, &a.blue, Vec3::ZERO, Vec3::new(4.0, 1.9, 1.2)),
        // carry handle arch, standing well clear of the body
        boxy(
            a,
            &a.metal,
            Vec3::new(-1.5, 1.5, 0.0),
            Vec3::new(0.22, 1.3, 0.22),
        ),
        boxy(
            a,
            &a.metal,
            Vec3::new(1.5, 1.5, 0.0),
            Vec3::new(0.22, 1.3, 0.22),
        ),
        boxy(
            a,
            &a.metal,
            Vec3::new(0.0, 2.05, 0.0),
            Vec3::new(3.2, 0.22, 0.22),
        ),
        // cassette window + bright EQ bar on the face
        boxy(
            a,
            &a.dark,
            Vec3::new(0.0, 0.05, 0.62),
            Vec3::new(1.3, 1.1, 0.1),
        ),
        boxy(
            a,
            &a.cream,
            Vec3::new(0.0, 0.0, 0.66),
            Vec3::new(0.9, 0.5, 0.05),
        ),
        boxy(
            a,
            &a.glow,
            Vec3::new(0.0, 0.72, 0.64),
            Vec3::new(1.5, 0.22, 0.06),
        ),
    ];
    for x in [-1.25, 1.25] {
        v.push(disc(
            a,
            &a.dark,
            Vec3::new(x, 0.0, 0.6),
            Vec3::new(1.4, 0.16, 1.4),
        ));
        v.push(disc(
            a,
            &a.metal,
            Vec3::new(x, 0.0, 0.66),
            Vec3::new(0.95, 0.14, 0.95),
        ));
        v.push(disc(
            a,
            &a.glow,
            Vec3::new(x, 0.0, 0.72),
            Vec3::new(0.34, 0.14, 0.34),
        ));
    }
    for (i, x) in [-0.9, -0.3, 0.3, 0.9].into_iter().enumerate() {
        let m = if i % 2 == 0 { &a.red } else { &a.teal };
        v.push(boxy(
            a,
            m,
            Vec3::new(x, -0.78, 0.6),
            Vec3::new(0.28, 0.16, 0.08),
        ));
    }
    v
}

/// Synth — Arpeggio rapid fire. Low wedge with a white keybed along the front.
fn synth_parts(a: &PickupAssets) -> Vec<Part> {
    let mut v = vec![
        boxy(
            a,
            &a.blue,
            Vec3::new(0.0, 0.35, -0.15),
            Vec3::new(4.0, 0.7, 1.5),
        ),
        // white keybed proud of the front edge + chunky black keys
        boxy(
            a,
            &a.cream,
            Vec3::new(0.0, 0.05, 0.75),
            Vec3::new(3.6, 0.3, 0.7),
        ),
        // bright screen + a fat slider on the top panel
        boxy(
            a,
            &a.glow,
            Vec3::new(-1.0, 0.72, -0.35),
            Vec3::new(1.1, 0.1, 0.55),
        ),
        boxy(
            a,
            &a.dark,
            Vec3::new(0.35, 0.72, -0.2),
            Vec3::new(1.1, 0.08, 0.2),
        ),
        boxy(
            a,
            &a.glow,
            Vec3::new(0.6, 0.78, -0.2),
            Vec3::new(0.22, 0.18, 0.26),
        ),
    ];
    for k in 0..6 {
        let x = -1.35 + k as f32 * 0.54;
        v.push(boxy(
            a,
            &a.dark,
            Vec3::new(x, 0.24, 0.78),
            Vec3::new(0.16, 0.22, 0.42),
        ));
    }
    for (i, x) in [1.2, 1.6].into_iter().enumerate() {
        let m = if i == 0 { &a.red } else { &a.teal };
        v.push(knob(a, m, Vec3::new(x, 0.74, -0.3), 0.18));
    }
    v
}

fn spawn_pickup(
    commands: &mut Commands,
    art: &PickupAssets,
    kind: PickupKind,
    at: Vec2,
    arena: Vec2,
) {
    let bound = arena - Vec2::splat(2.0);
    let pos = at.clamp(-bound, bound);
    let parts = match kind {
        PickupKind::Encore => amp_parts(art),
        PickupKind::DualGuitar => boombox_parts(art),
        PickupKind::Arpeggio => synth_parts(art),
    };
    commands
        .spawn((
            Pickup(kind),
            Float(rand::thread_rng().gen_range(0.0..std::f32::consts::TAU)),
            Transform::from_translation(ground(pos, 1.9)).with_scale(Vec3::splat(0.7)),
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            for (mesh, mat, transform) in parts {
                c.spawn((Mesh3d(mesh), MeshMaterial3d(mat), transform));
            }
        });
}

fn float_pickups(time: Res<Time>, mut q: Query<(&mut Float, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut f, mut t) in &mut q {
        f.0 += dt;
        t.rotation = Quat::from_rotation_y(f.0 * 1.3);
        let base = t.translation;
        t.translation = Vec3::new(base.x, 2.0 + 0.4 * (f.0 * 2.2).sin(), base.z);
    }
}

fn spawn_timed_pickup(
    mut commands: Commands,
    time: Res<Time>,
    level: Res<CurrentLevel>,
    art: Res<PickupAssets>,
    mut timer: ResMut<PickupTimer>,
    players: Query<&Transform, With<Player>>,
    mut turn: Local<usize>,
) {
    if !timer.0.tick(time.delta()).is_finished() {
        return;
    }
    timer.0 = Timer::from_seconds(8.0, TimerMode::Once);

    let n = players.iter().count().max(1) as f32;
    let focus = players.iter().map(|t| plane(t.translation)).sum::<Vec2>() / n;
    let mut rng = rand::thread_rng();
    let ang = rng.gen_range(0.0..std::f32::consts::TAU);
    let at = focus + Vec2::from_angle(ang) * rng.gen_range(13.0..22.0);

    // Cycle through the three, Encore first (see `PickupKind::ALL`).
    let kind = PickupKind::ALL[*turn % PickupKind::ALL.len()];
    *turn += 1;
    spawn_pickup(&mut commands, &art, kind, at, level.arena);
}

fn drop_from_kills(
    mut commands: Commands,
    art: Res<PickupAssets>,
    level: Res<CurrentLevel>,
    mut died: MessageReader<EnemyDied>,
) {
    let mut rng = rand::thread_rng();
    for EnemyDied { pos, .. } in died.read() {
        if rng.r#gen::<f32>() < 0.035 {
            let kind = PickupKind::ALL[rng.gen_range(0..PickupKind::ALL.len())];
            spawn_pickup(&mut commands, &art, kind, plane(*pos), level.arena);
        }
    }
}

/// Either hero can scoop a pickup up, but the buffs are all guitar-themed, so
/// they always land on the guitarist.
fn collect_pickups(
    mut commands: Commands,
    pickups: Query<(Entity, &Transform, &Pickup)>,
    players: Query<(Entity, &Transform, &Hitbox, &Hero), With<Player>>,
) {
    let Some(guitarist) = players
        .iter()
        .find(|(_, _, _, h)| **h == Hero::Guitarist)
        .map(|(e, ..)| e)
    else {
        return;
    };

    for (e, t, pickup) in &pickups {
        let pp = plane(t.translation);
        let touched = players
            .iter()
            .any(|(_, pt, phb, _)| plane(pt.translation).distance(pp) <= PICKUP_REACH + phb.0);
        if !touched {
            continue;
        }
        match pickup.0 {
            PickupKind::DualGuitar => {
                commands
                    .entity(guitarist)
                    .insert(DualWield(Timer::from_seconds(BUFF_SECS, TimerMode::Once)));
            }
            PickupKind::Encore => {
                let mut ready = Timer::from_seconds(ENCORE_COOLDOWN, TimerMode::Once);
                ready.tick(ready.duration()); // first blast is ready immediately
                commands.entity(guitarist).insert(Encore {
                    window: Timer::from_seconds(BUFF_SECS, TimerMode::Once),
                    cooldown: ready,
                });
            }
            PickupKind::Arpeggio => {
                commands
                    .entity(guitarist)
                    .insert(RapidFire(Timer::from_seconds(BUFF_SECS, TimerMode::Once)));
            }
        }
        commands.entity(e).despawn();
    }
}

/// Keep exactly one left-hand gun iff the player is dual-wielding.
fn manage_dual_gun(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dual: Query<(), (With<Player>, With<DualWield>)>,
    left_guns: Query<(Entity, &GuitarGun)>,
) {
    let want = !dual.is_empty();
    let have = left_guns.iter().any(|(_, g)| g.left);
    if want && !have {
        spawn_guitar_gun(&mut commands, &mut meshes, &mut materials, true);
    } else if !want && have {
        for (e, g) in &left_guns {
            if g.left {
                commands.entity(e).despawn();
            }
        }
    }
}

fn encore_blast(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<PickupAssets>,
    mut cues: MessageWriter<super::audio::AudioCue>,
    mut player: Query<(&Transform, &Intent, &mut Encore), With<Player>>,
) {
    let Ok((pt, intent, mut enc)) = player.single_mut() else {
        return;
    };
    enc.cooldown.tick(time.delta());
    if !enc.cooldown.is_finished() {
        return;
    }
    if !intent.special {
        return;
    }
    let pp = plane(pt.translation);
    enc.cooldown.reset();
    cues.write(super::audio::AudioCue::ChordStab);

    commands.spawn((
        SoundWave {
            radius: 1.0,
            hit: Vec::new(),
        },
        Mesh3d(art.wave.clone()),
        MeshMaterial3d(art.wave_mat.clone()),
        Transform::from_translation(ground(pp, 0.6)).with_scale(Vec3::splat(1.0)),
        Visibility::default(),
        RunEntity,
    ));
}

fn expand_sound_waves(
    mut commands: Commands,
    time: Res<Time>,
    mut waves: Query<(Entity, &mut SoundWave, &mut Transform), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Health), With<Enemy>>,
) {
    let dt = time.delta_secs();
    for (e, mut wave, mut wt) in &mut waves {
        wave.radius += WAVE_SPEED * dt;
        wt.scale = Vec3::splat(wave.radius);
        let center = plane(wt.translation);

        for (enemy, mut et, mut hp) in &mut enemies {
            if wave.hit.contains(&enemy) {
                continue;
            }
            let to = plane(et.translation) - center;
            if (to.length() - wave.radius).abs() < 1.2 {
                hp.current -= WAVE_DAMAGE;
                wave.hit.push(enemy);
                let shove = to.normalize_or_zero() * WAVE_KNOCK;
                et.translation += ground(shove, 0.0);
            }
        }

        if wave.radius >= WAVE_MAX {
            commands.entity(e).despawn();
        }
    }
}

fn expire_buffs(
    mut commands: Commands,
    time: Res<Time>,
    mut player: Query<
        (
            Entity,
            &Hero,
            Option<&mut DualWield>,
            Option<&mut Encore>,
            Option<&mut RapidFire>,
        ),
        With<Player>,
    >,
) {
    let Some((player, _, dual, encore, rapid)) =
        player.iter_mut().find(|(_, h, ..)| **h == Hero::Guitarist)
    else {
        return;
    };
    if let Some(mut d) = dual
        && d.0.tick(time.delta()).is_finished()
    {
        commands.entity(player).remove::<DualWield>();
    }
    if let Some(mut e) = encore
        && e.window.tick(time.delta()).is_finished()
    {
        commands.entity(player).remove::<Encore>();
    }
    if let Some(mut r) = rapid
        && r.0.tick(time.delta()).is_finished()
    {
        commands.entity(player).remove::<RapidFire>();
    }
}
