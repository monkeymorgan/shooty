//! **The waves — other people's dark moods.**
//!
//! A wave is not monsters. The five miserable townsfolk in [`super::gloom`] are
//! leaking bad energy, and what pours out of them is *them*: dark, drained
//! copies of the disco kid, the techno raver, the country fan — recognisably
//! that person, wearing that person's genre outfit, but shadowed near-black
//! with the life gone out of them and their eyes lit up wrong. You are fighting
//! a citizen's gloom, not the citizen; clear it and they cheer up (`gloom.rs`).
//!
//! Every enemy carries [`super::gloom::FromGloom`], the index of the source it
//! came out of — which is also an index into [`super::gloom::STEMS`], so it
//! says *which* of the five genres this mood is wearing.
//!
//! Four tiers, all on the boxy `blocky/base.glb` rig (the house style, the same
//! rig the heroes stand on):
//!
//! * [`EnemyKind::Mood`] — a walking dark clone. The bulk of a wave.
//! * [`EnemyKind::Head`] — just the head, detached and drifting. Fast, fragile.
//! * [`EnemyKind::Heckler`] — hangs back and throws a dark bad-vibe bolt.
//! * [`EnemyKind::Sink`] — an oversized clone. Soaks fire, walks you down.

use std::time::Duration;

use bevy::prelude::*;
use bevy::world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady};
use rand::Rng;

use super::player::boxy_atlas;
use super::{
    CurrentLevel, Downed, Enemy, EnemyBullet, EnemyKind, GameState, Health, Hitbox, Lifetime,
    Obstacle, Player, RunEntity, ground, nearest_player, plane, resolve_obstacles, scale, skins,
};

pub struct EnemyPlugin;

/// Number of gloom sources / genres — the array width for the per-genre skins.
const GENRES: usize = super::gloom::STEMS.len();

/// The three expressions a detached [`EnemyKind::Head`] drifts around in: the
/// PNG under `assets/models/faces/`, the hue its eyes glow, and how hard.
///
/// These used to be the entire enemy roster (rusher / sponge / flyer). Now they
/// are only the moods a loose head wears — the genre identity lives on the
/// humanoid tiers instead. Shared with `src/bin/lab.rs`.
pub const FACE_STYLES: [(&str, Color, f32); 3] = [
    ("anxious", Color::srgb(0.42, 0.92, 0.18), 1.9),
    ("despairing", Color::srgb(0.60, 0.34, 0.96), 1.6),
    ("bitter", Color::srgb(0.96, 0.28, 0.24), 2.2),
];

/// Heights, in metres, of the humanoid tiers. A [`EnemyKind::Mood`] stands a
/// head shorter than the 1.8 m citizen it copies — a diminished version of the
/// person — and only the [`EnemyKind::Sink`] is allowed to loom over a hero.
const MOOD_H: f32 = 1.42;
const HECKLER_H: f32 = 1.6;
const SINK_H: f32 = 2.5;

/// How far a [`EnemyKind::Heckler`] tries to keep between itself and the nearest
/// hero. Inside this it stops closing and just shoots.
const HECKLER_STANDOFF: f32 = 13.0;

/// The bad-vibe bolt.
const BOLT_SPEED: f32 = 22.0;
const BOLT_DAMAGE: f32 = 9.0;

fn body_height(kind: EnemyKind) -> f32 {
    match kind {
        EnemyKind::Heckler => HECKLER_H,
        EnemyKind::Sink => SINK_H,
        _ => MOOD_H,
    }
}

/// The uniform scale to spawn `blocky/base.glb` at for a humanoid tier.
fn body_scale(kind: EnemyKind) -> f32 {
    scale::fit(skins::BOXY_NATIVE_H, body_height(kind))
}

/// Is this tier the boxy rig walking upright (as opposed to a loose head)?
fn is_humanoid(kind: EnemyKind) -> bool {
    matches!(
        kind,
        EnemyKind::Mood | EnemyKind::Heckler | EnemyKind::Sink
    )
}

/// Local transforms for a [`EnemyKind::Head`]'s face quad(s) on a *unit* cube
/// (the caller applies `s.visual`). A head tumbles, so it wears the face on all
/// four sides and the top.
pub fn face_placements(kind: EnemyKind) -> Vec<Transform> {
    if kind != EnemyKind::Head {
        return vec![];
    }
    let out = 0.5 + 0.02;
    let flat_top = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)
        * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    vec![
        Transform::from_xyz(0.0, 0.0, out),
        Transform::from_xyz(0.0, 0.0, -out)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        Transform::from_xyz(out, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
        Transform::from_xyz(-out, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
        Transform::from_xyz(0.0, out, 0.0).with_rotation(flat_top),
    ]
}

/// The (near-black body, glowing face) material pair for one [`FACE_STYLES`]
/// entry. The face PNG is used as **both** base-colour and emissive map — its
/// ground is near-black and its eyes/mouth are the only lit thing — so one
/// texture makes the features glow while the head around them stays dark.
///
/// Shared by the game and by `src/bin/lab.rs`, which must not be able to
/// preview a look the game does not spawn.
pub fn vibe_materials(
    style: usize,
    assets: &AssetServer,
    materials: &mut Assets<StandardMaterial>,
) -> (Handle<StandardMaterial>, Handle<StandardMaterial>) {
    let (name, hue, glow) = FACE_STYLES[style % FACE_STYLES.len()];
    let l = hue.to_linear();
    let body = materials.add(StandardMaterial {
        base_color: Color::linear_rgb(
            l.red * 0.03 + 0.010,
            l.green * 0.03 + 0.010,
            l.blue * 0.03 + 0.013,
        ),
        perceptual_roughness: 0.95,
        ..default()
    });
    let tex = assets.load(format!("models/faces/{name}.png"));
    let face = materials.add(StandardMaterial {
        base_color_texture: Some(tex.clone()),
        emissive_texture: Some(tex),
        emissive: LinearRgba::rgb(glow, glow, glow),
        perceptual_roughness: 0.85,
        ..default()
    });
    (body, face)
}

/// Everything a wave needs to build any of the four tiers, allocated once.
#[derive(Resource)]
struct EnemyAssets {
    /// Head tier.
    cube: Handle<Mesh>,
    face_quad: Handle<Mesh>,
    head_faces: [Handle<StandardMaterial>; 3],

    /// Humanoid tiers: the boxy rig in a drained genre skin.
    base_scene: Handle<WorldAsset>,
    base_gltf: Handle<Gltf>,
    /// Drained genre atlas, one per gloom source / genre.
    dark_skin: [Handle<StandardMaterial>; GENRES],
    /// The glowing dark-mood face decal for a body's head, genre-hued.
    mood_face: [Handle<StandardMaterial>; GENRES],
    /// Built once `base_gltf` has loaded (it needs the baked clip handles).
    anim: Option<BaseAnim>,

    /// The Heckler's bad-vibe bolt + its little muzzle spark.
    bolt_mesh: Handle<Mesh>,
    bolt_mat: Handle<StandardMaterial>,
}

/// The clips the humanoid tiers run, resolved once `blocky/base.glb` is in.
#[derive(Clone)]
struct BaseAnim {
    graph: Handle<AnimationGraph>,
    walk: AnimationNodeIndex,
    hold: AnimationNodeIndex,
}

/// On a humanoid enemy's model-root child: what `on_enemy_ready` needs once the
/// GLB scene has instantiated.
#[derive(Component)]
struct EnemyModel {
    kind: EnemyKind,
    /// Gloom source index — picks the drained genre skin + face.
    genre: usize,
}

/// On a [`EnemyKind::Heckler`]: its shot cooldown.
#[derive(Component)]
struct HecklerGun(Timer);

impl EnemyKind {
    fn scrap(self) -> u32 {
        match self {
            EnemyKind::Mood | EnemyKind::Head => 1,
            EnemyKind::Heckler => 3,
            EnemyKind::Sink => 5,
        }
    }
}

fn load_enemy_assets(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let head_faces = std::array::from_fn(|i| vibe_materials(i, &assets, &mut materials).1);

    let dark_skin = std::array::from_fn(|i| {
        let atlas = skins::boxy(super::gloom::STEMS[i]).atlas;
        // A cool multiply over the painted genre atlas: drained and shadowed,
        // but not so far down that the crowd is one black blob. You should be
        // able to tell a dark-disco clone (white suit → grey) from a dark-techno
        // one (black + a cyan visor) — the enemy has to read *as* that citizen.
        let hue = skins::sound(super::gloom::STEMS[i]).color.to_linear();
        materials.add(StandardMaterial {
            // Dark and cool enough to still read as a shadow of a person, light
            // enough that the garment values come through — a dark-disco clone
            // (pale suit → grey) vs a dark-hip-hop one (red puffer → maroon).
            base_color: Color::srgb(0.24, 0.23, 0.30),
            base_color_texture: Some(boxy_atlas(&assets, atlas)),
            // Barely-there district hue in the shadow, so the five waves still
            // separate by colour before you can see a face.
            emissive: LinearRgba::rgb(hue.red * 0.025, hue.green * 0.025, hue.blue * 0.035),
            perceptual_roughness: 0.94,
            ..default()
        })
    });
    let mood_face = std::array::from_fn(|i| {
        // Genre-hued glow, so a wave from the hip-hop source reads red and one
        // from techno reads cyan even before you pick out a face.
        let hue = skins::sound(super::gloom::STEMS[i]).color.to_linear();
        let tex = assets.load("models/faces/despairing.png");
        materials.add(StandardMaterial {
            base_color_texture: Some(tex.clone()),
            emissive_texture: Some(tex),
            emissive: LinearRgba::rgb(hue.red * 1.7, hue.green * 1.7, hue.blue * 1.7),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.85,
            ..default()
        })
    });

    commands.insert_resource(EnemyAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        face_quad: meshes.add(Rectangle::new(1.0, 1.0)),
        head_faces,
        base_scene: assets.load(GltfAssetLabel::Scene(0).from_asset(skins::BOXY_BASE)),
        base_gltf: assets.load(skins::BOXY_BASE),
        dark_skin,
        mood_face,
        anim: None,
        bolt_mesh: meshes.add(Sphere::new(0.32)),
        bolt_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(0.10, 0.05, 0.16),
            emissive: LinearRgba::rgb(1.6, 0.35, 2.4),
            unlit: false,
            ..default()
        }),
    });
}

/// Build the humanoid walk/hold graph once `blocky/base.glb` has loaded.
fn prepare_base_anim(
    mut art: ResMut<EnemyAssets>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    if art.anim.is_some() {
        return;
    }
    let Some(gltf) = gltfs.get(&art.base_gltf) else {
        return;
    };
    let clip = |n: &str| {
        gltf.named_animations
            .get(n)
            .cloned()
            .unwrap_or_else(|| gltf.named_animations.values().next().cloned().unwrap())
    };
    let (graph, idx) = AnimationGraph::from_clips([clip("walk"), clip("holding-right")]);
    art.anim = Some(BaseAnim {
        graph: graphs.add(graph),
        walk: idx[0],
        hold: idx[1],
    });
}

/// Which part of the wave cadence we're in (M4). `Prep` is the calm
/// between-wave lull — the window to build; `Spawning` pours the wave in;
/// `Clearing` waits for the field to thin out before the next lull.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum WavePhase {
    Prep,
    Spawning,
    Clearing,
}

/// The discrete-wave state machine that replaced M3's continuous trickle.
#[derive(Resource)]
pub struct Wave {
    pub number: u32,
    pub phase: WavePhase,
    /// Prep countdown, or the Clearing timeout.
    phase_timer: Timer,
    /// Enemies left to pour in this wave.
    budget: u32,
    /// Interval between spawn bursts while `Spawning`.
    burst: Timer,
}

impl Default for Wave {
    fn default() -> Self {
        Self {
            number: 0,
            phase: WavePhase::Prep,
            phase_timer: Timer::from_seconds(3.0, TimerMode::Once),
            budget: 0,
            burst: Timer::from_seconds(0.45, TimerMode::Repeating),
        }
    }
}

impl Wave {
    /// Seconds left in the current Prep lull (0 outside Prep) — for the HUD.
    pub fn prep_left(&self) -> f32 {
        if self.phase == WavePhase::Prep {
            self.phase_timer.remaining_secs()
        } else {
            0.0
        }
    }

    /// Overwrite the display-relevant fields from a net snapshot (client only).
    pub fn net_apply(&mut self, number: u32, phase: WavePhase, prep_left: f32) {
        self.number = number;
        self.phase = phase;
        self.phase_timer = Timer::from_seconds(prep_left.max(0.001), TimerMode::Once);
    }
}

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_enemy_assets)
            .add_systems(OnEnter(GameState::Playing), |mut commands: Commands| {
                commands.insert_resource(Wave::default());
            })
            .add_systems(Update, prepare_base_anim)
            .add_systems(
                Update,
                (run_wave, move_enemies, heckler_fire, move_enemy_bullets)
                    .run_if(in_state(GameState::Playing))
                    .run_if(super::net::authoritative),
            )
            .add_systems(
                Update,
                net_apply_enemies
                    .run_if(in_state(GameState::Playing))
                    .run_if(super::net::is_client),
            );
    }
}

/// Per-wave enemy budget and burst shape.
fn wave_shape(number: u32, capture: bool) -> (u32, u32, f32) {
    if capture {
        let n = number.max(1);
        ((40 + 18 * n).min(280), (10 + n).min(28), 0.24)
    } else {
        let n = number.max(1);
        ((18 + 10 * n).min(130), (5 + n / 2).min(12), 0.46)
    }
}

pub struct Stats {
    pub hp: f32,
    pub speed: f32,
    pub touch_damage: f32,
    /// Collision radius on the ground plane.
    pub size: f32,
    /// Resting height of the parent transform.
    pub hover: f32,
    /// Cube side length of a [`EnemyKind::Head`]; unused for the humanoid tiers
    /// (they are a GLB scaled by [`body_scale`]).
    pub visual: f32,
}

pub fn stats(kind: EnemyKind) -> Stats {
    match kind {
        EnemyKind::Mood => Stats {
            hp: 44.0,
            speed: 8.0,
            touch_damage: 8.0,
            size: 1.0,
            hover: 0.0,
            visual: 0.0,
        },
        EnemyKind::Head => Stats {
            hp: 24.0,
            speed: 11.0,
            touch_damage: 6.0,
            size: 0.72,
            hover: 1.5,
            visual: scale::m(scale::VIBE_FLYER),
        },
        EnemyKind::Heckler => Stats {
            hp: 62.0,
            speed: 4.6,
            touch_damage: 7.0,
            size: 1.0,
            hover: 0.0,
            visual: 0.0,
        },
        EnemyKind::Sink => Stats {
            hp: 210.0,
            speed: 2.8,
            touch_damage: 22.0,
            size: 1.5,
            hover: 0.0,
            visual: 0.0,
        },
    }
}

#[derive(Component)]
struct Hover(f32);

#[allow(clippy::too_many_arguments)]
fn run_wave(
    mut commands: Commands,
    time: Res<Time>,
    level: Res<CurrentLevel>,
    art: Res<EnemyAssets>,
    mut wave: ResMut<Wave>,
    autoplay: Option<Res<super::AutoPlay>>,
    zones: Option<Res<super::build::SecuredZones>>,
    sites: Query<&Transform, With<super::build::BuildSite>>,
    players: Query<(&Transform, Option<&Downed>), With<Player>>,
    enemies: Query<(), With<Enemy>>,
    gloom: Query<(&super::gloom::GloomSource, &Transform)>,
) {
    let capture = autoplay.is_some();
    let alive = enemies.iter().count();
    let dt = time.delta();

    let mut trickle = false;
    match wave.phase {
        WavePhase::Prep => {
            if wave.phase_timer.tick(dt).is_finished() {
                wave.number += 1;
                let (budget, _, burst) = wave_shape(wave.number, capture);
                wave.budget = budget;
                wave.burst = Timer::from_seconds(burst, TimerMode::Repeating);
                wave.phase = WavePhase::Spawning;
                return;
            }
            if !(capture && wave.burst.tick(dt).just_finished()) {
                return;
            }
            trickle = true;
        }
        WavePhase::Spawning => {
            if wave.budget == 0 {
                let timeout = if capture { 12.0 } else { 34.0 };
                wave.phase_timer = Timer::from_seconds(timeout, TimerMode::Once);
                wave.phase = WavePhase::Clearing;
                return;
            }
            if !wave.burst.tick(dt).just_finished() {
                return;
            }
        }
        WavePhase::Clearing => {
            let cleared = alive <= if capture { 40 } else { 3 };
            if cleared || wave.phase_timer.tick(dt).is_finished() {
                let lull = if capture {
                    3.0
                } else if wave.number == 0 {
                    5.0
                } else {
                    14.0
                };
                wave.phase_timer = Timer::from_seconds(lull, TimerMode::Once);
                wave.phase = WavePhase::Prep;
                return;
            }
            return;
        }
    }

    if alive > if capture { 260 } else { 120 } {
        return;
    }

    let live: Vec<Vec2> = players
        .iter()
        .filter(|(_, d)| d.is_none())
        .map(|(t, _)| plane(t.translation))
        .collect();
    let focus = if live.is_empty() {
        players
            .iter()
            .next()
            .map(|(t, _)| plane(t.translation))
            .unwrap_or(Vec2::ZERO)
    } else {
        live.iter().copied().sum::<Vec2>() / live.len() as f32
    };

    // **Where a wave comes from.** It boils off whichever brooding townsperson
    // is closest to the party — so the mob always turns up in the district you
    // are actually standing in, and clearing that source before you move on is
    // how you reclaim the map a piece at a time. A source only broods once you
    // have been near it (`gloom.rs`), so early on this is just "the one you woke".
    let vent = gloom
        .iter()
        .filter(|(g, _)| g.mood == super::gloom::Mood::Brooding)
        .map(|(g, t)| (g.idx, plane(t.translation)))
        .min_by(|a, b| {
            a.1.distance_squared(focus)
                .total_cmp(&b.1.distance_squared(focus))
        });
    let Some((genre, vent)) = vent else {
        return;
    };
    let bound = level.arena - Vec2::splat(3.0);

    let mut rng = rand::thread_rng();
    let (_, burst_size, _) = wave_shape(wave.number, capture);
    if trickle && alive > 48 {
        return;
    }
    let batch = if trickle {
        5
    } else {
        let b = burst_size.min(wave.budget);
        wave.budget -= b;
        b
    };

    for _ in 0..batch {
        let roll: f32 = rng.r#gen();
        let kind = if roll < 0.58 {
            EnemyKind::Mood
        } else if roll < 0.80 {
            EnemyKind::Head
        } else if roll < 0.93 {
            EnemyKind::Heckler
        } else {
            EnemyKind::Sink
        };

        let ang = rng.gen_range(0.0..std::f32::consts::TAU);
        let dist = rng.gen_range(2.4..6.0);
        let mut pos = vent + Vec2::from_angle(ang) * dist;
        if let Some(z) = &zones {
            pos = z.push_out(pos, focus);
        }
        for st in &sites {
            let c = plane(st.translation);
            if pos.distance(c) < super::build::LULL_RADIUS {
                pos = c + (pos - c).normalize_or_zero() * super::build::LULL_RADIUS;
            }
        }
        let pos = pos.clamp(-bound, bound);
        let style = rng.gen_range(0..FACE_STYLES.len());

        let e = spawn_enemy(&mut commands, &art, kind, pos, genre, style);
        commands.entity(e).insert(super::gloom::FromGloom(genre));
    }
}

/// Spawn one enemy of `kind` at `pos`. `genre` is the gloom source index (which
/// drained genre skin it wears); `style` is the head-face expression.
fn spawn_enemy(
    commands: &mut Commands,
    art: &EnemyAssets,
    kind: EnemyKind,
    pos: Vec2,
    genre: usize,
    style: usize,
) -> Entity {
    let s = stats(kind);
    let mut ent = commands.spawn((
        Enemy {
            kind,
            touch_damage: s.touch_damage,
            speed: s.speed,
        },
        Health::new(s.hp),
        Hitbox(s.size),
        Hover(s.hover),
        Transform::from_translation(ground(pos, s.hover)),
        Visibility::default(),
        RunEntity,
    ));

    if kind == EnemyKind::Head {
        let v = s.visual;
        ent.with_children(|c| {
            c.spawn((
                Mesh3d(art.cube.clone()),
                MeshMaterial3d(art.head_faces[style % 3].clone()),
                Transform::from_xyz(0.0, v * 0.5, 0.0).with_scale(Vec3::splat(v * 0.62)),
            ));
            for mut q in face_placements(kind) {
                q.translation = Vec3::new(0.0, v * 0.5, 0.0) + q.translation * v * 0.62;
                q.scale = Vec3::splat(v * 0.62);
                c.spawn((
                    Mesh3d(art.face_quad.clone()),
                    MeshMaterial3d(art.head_faces[style % 3].clone()),
                    q,
                ));
            }
        });
        return ent.id();
    }

    // Humanoid: the boxy rig, dressed and animated by `on_enemy_ready`.
    // Per-clone height jitter so a crowd of one genre still isn't stamped.
    let mut rng = rand::thread_rng();
    let sc = body_scale(kind) * rng.gen_range(0.9..1.12);
    let hunch = if kind == EnemyKind::Sink { 0.12 } else { 0.0 };
    ent.with_children(|c| {
        c.spawn((
            WorldAssetRoot(art.base_scene.clone()),
            EnemyModel { kind, genre },
            Transform::from_scale(Vec3::splat(sc)).with_rotation(Quat::from_rotation_x(hunch)),
        ))
        .observe(on_enemy_ready);
    });
    if kind == EnemyKind::Heckler {
        ent.insert(HecklerGun(Timer::from_seconds(
            rng.gen_range(1.6..2.6),
            TimerMode::Repeating,
        )));
    }
    ent.id()
}

/// Once a humanoid enemy's GLB scene has instantiated: paint every part with the
/// drained genre skin, hang the glowing dark-mood face over its head, and loop
/// the right clip (walk for Mood/Sink, a menacing hold for the Heckler).
fn on_enemy_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    art: Res<EnemyAssets>,
    model: Query<&EnemyModel>,
    children: Query<&Children>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let root = ready.entity;
    let Ok(&EnemyModel { kind, genre }) = model.get(root) else {
        return;
    };

    let skin = art.dark_skin[genre % GENRES].clone();
    for e in children.iter_descendants(root) {
        if mesh_mats.contains(e) {
            commands.entity(e).insert(MeshMaterial3d(skin.clone()));
        }
    }

    // The dark-mood face: a genre-hued glowing panel over the head front. Pinned
    // to the model root at head height (native units — the root carries the
    // scale) rather than the head node, so it can't race a same-frame despawn.
    commands.entity(root).with_child((
        Mesh3d(art.face_quad.clone()),
        MeshMaterial3d(art.mood_face[genre % GENRES].clone()),
        Transform::from_xyz(0.0, 2.30, 0.46).with_scale(Vec3::splat(0.66)),
    ));

    let Some(anim) = &art.anim else { return };
    let Some(anim_entity) = children.iter_descendants(root).find(|e| players.contains(*e)) else {
        warn!("no AnimationPlayer under the boxy enemy scene");
        return;
    };
    let mut player = players.get_mut(anim_entity).unwrap();
    let node = if kind == EnemyKind::Heckler {
        anim.hold
    } else {
        anim.walk
    };
    let mut rng = rand::thread_rng();
    let mut transitions = AnimationTransitions::new();
    transitions
        .play(&mut player, node, Duration::ZERO)
        .repeat()
        .set_speed(rng.gen_range(0.85..1.12))
        .seek_to(rng.gen_range(0.0..1.0));
    commands
        .entity(anim_entity)
        .insert((AnimationGraphHandle(anim.graph.clone()), transitions));
}

/// A Heckler at a standoff throws a bad-vibe bolt at the nearest hero.
fn heckler_fire(
    mut commands: Commands,
    time: Res<Time>,
    art: Res<EnemyAssets>,
    mut hecklers: Query<(&Transform, &mut HecklerGun)>,
    players: Query<(&Transform, Option<&Downed>), With<Player>>,
) {
    let live: Vec<Vec2> = players
        .iter()
        .filter(|(_, d)| d.is_none())
        .map(|(t, _)| plane(t.translation))
        .collect();
    if live.is_empty() {
        return;
    }
    for (t, mut gun) in &mut hecklers {
        if !gun.0.tick(time.delta()).just_finished() {
            continue;
        }
        let here = plane(t.translation);
        let Some(target) = nearest_player(here, live.iter().map(|p| (*p, false))) else {
            continue;
        };
        // Only shoot once you're roughly in range — a Heckler stuck far off
        // behind a building shouldn't be lobbing blind.
        if here.distance(target) > HECKLER_STANDOFF * 2.2 {
            continue;
        }
        let from = t.translation + Vec3::Y * 1.4;
        let to = ground(target, 1.0);
        let dir = (to - from).normalize_or_zero();
        if dir == Vec3::ZERO {
            continue;
        }
        commands.spawn((
            EnemyBullet {
                velocity: dir * BOLT_SPEED,
                damage: BOLT_DAMAGE,
            },
            Hitbox(0.45),
            Lifetime(Timer::from_seconds(3.0, TimerMode::Once)),
            Mesh3d(art.bolt_mesh.clone()),
            MeshMaterial3d(art.bolt_mat.clone()),
            Transform::from_translation(from),
            RunEntity,
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.75, 0.30, 1.0),
                intensity: 180_000.0,
                range: 6.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(from),
            Lifetime(Timer::from_seconds(0.08, TimerMode::Once)),
            RunEntity,
        ));
    }
}

fn move_enemy_bullets(time: Res<Time>, mut q: Query<(&EnemyBullet, &mut Transform)>) {
    let dt = time.delta_secs();
    for (b, mut t) in &mut q {
        t.translation += b.velocity * dt;
    }
}

/// Net client: reconcile the ghost enemy set against the latest snapshot.
fn net_apply_enemies(
    mut commands: Commands,
    latest: Res<super::net::LatestSnapshot>,
    mut ghosts: ResMut<super::net::EnemyGhosts>,
    art: Option<Res<EnemyAssets>>,
    mut tf: Query<&mut Transform, With<Enemy>>,
) {
    let (Some(snap), Some(art)) = (&latest.0, art) else {
        return;
    };

    let mut seen = std::collections::HashSet::with_capacity(snap.enemies.len());
    for e in &snap.enemies {
        seen.insert(e.id);
        let pos = Vec3::from(e.pos);
        let rot = Quat::from_rotation_y(e.yaw);
        if let Some(&ent) = ghosts.0.get(&e.id) {
            if let Ok(mut t) = tf.get_mut(ent) {
                t.translation = pos;
                t.rotation = rot;
            }
        } else {
            let ent = spawn_enemy(
                &mut commands,
                &art,
                EnemyKind::from_u8(e.kind),
                plane(pos),
                (e.id as usize) % GENRES,
                (e.id as usize) % FACE_STYLES.len(),
            );
            commands
                .entity(ent)
                .insert(Transform::from_translation(pos).with_rotation(rot));
            ghosts.0.insert(e.id, ent);
        }
    }

    ghosts.0.retain(|id, ent| {
        let keep = seen.contains(id);
        if !keep {
            commands.entity(*ent).try_despawn();
        }
        keep
    });
}

#[allow(clippy::type_complexity)]
fn move_enemies(
    time: Res<Time>,
    zones: Option<Res<super::build::SecuredZones>>,
    players: Query<(&Transform, Option<&Downed>), (With<Player>, Without<Enemy>)>,
    obstacles: Query<(&Transform, &Obstacle), (Without<Enemy>, Without<Player>)>,
    mut enemies: Query<
        (
            Entity,
            &Enemy,
            &Hitbox,
            &Hover,
            &mut Transform,
            Option<&super::Pacified>,
        ),
        (Without<Player>, Without<Obstacle>),
    >,
) {
    let heroes: Vec<(Vec2, bool)> = players
        .iter()
        .map(|(t, d)| (plane(t.translation), d.is_some()))
        .collect();
    if heroes.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    let clock = time.elapsed_secs();

    let blocks: Vec<(Vec2, f32)> = obstacles
        .iter()
        .map(|(t, o)| (plane(t.translation), o.radius))
        .collect();

    let bodies: Vec<(Entity, Vec2, f32)> = enemies
        .iter()
        .map(|(e, _, hb, _, t, _)| (e, plane(t.translation), hb.0))
        .collect();

    for (me, enemy, hitbox, hover, mut transform, pacified) in &mut enemies {
        let pos = plane(transform.translation);
        let (target, calm) = if pacified.is_some() {
            let out = zones
                .as_deref()
                .map(|z| super::build::drift_out(z, pos))
                .unwrap_or(Vec2::ZERO);
            (pos + out * 20.0, true)
        } else {
            (
                nearest_player(pos, heroes.iter().copied()).unwrap_or(pos),
                false,
            )
        };

        // A Heckler closes only to its standoff, then holds and shoots.
        let hold = !calm
            && enemy.kind == EnemyKind::Heckler
            && pos.distance(target) < HECKLER_STANDOFF;
        let seek = if hold {
            Vec2::ZERO
        } else {
            (target - pos).normalize_or_zero()
        };
        let speed_mul = if calm { 0.4 } else { 1.0 };

        let mut avoid = Vec2::ZERO;
        if hover.0 == 0.0 && seek != Vec2::ZERO {
            for (c, r) in &blocks {
                let to_c = *c - pos;
                let dist = to_c.length();
                let reach = r + hitbox.0 + 4.5;
                if dist < 1e-3 || dist > reach {
                    continue;
                }
                let ahead = to_c / dist;
                if ahead.dot(seek) <= 0.05 {
                    continue;
                }
                let tangent = Vec2::new(-ahead.y, ahead.x);
                let side = if tangent.dot(seek) >= 0.0 { 1.0 } else { -1.0 };
                let strength = (reach - dist) / reach;
                avoid += tangent * side * strength * 1.9 - ahead * strength * 0.7;
            }
            avoid = avoid.clamp_length_max(2.2);
        }

        let mut heading = (seek + avoid).normalize_or_zero();
        if heading == Vec2::ZERO {
            heading = seek;
        }

        let mut push = Vec2::ZERO;
        for (other, opos, oradius) in &bodies {
            if *other == me {
                continue;
            }
            let away = pos - *opos;
            let min_gap = hitbox.0 + oradius;
            let d = away.length();
            if d < min_gap && d > 1e-4 {
                push += (away / d) * (min_gap - d) / min_gap;
            }
        }
        push = push.clamp_length_max(2.5);

        let mut next =
            pos + (heading * enemy.speed * speed_mul + push * (enemy.speed.max(6.0) * 1.4)) * dt;
        if hover.0 == 0.0 {
            next = resolve_obstacles(next, hitbox.0, &blocks);
        }

        let bob = match enemy.kind {
            EnemyKind::Head => 0.5 * (clock * 5.0 + pos.x).sin(),
            _ => 0.0,
        };
        transform.translation = ground(next, hover.0 + bob);

        // A loose head tumbles; a humanoid turns to face where it's going (or,
        // when holding, toward the hero it's shooting at).
        if enemy.kind == EnemyKind::Head {
            transform.rotation = Quat::from_rotation_y(clock * 2.5)
                * Quat::from_rotation_x(0.4 * (clock * 1.7).sin());
        } else if is_humanoid(enemy.kind) {
            let look = if hold { target - pos } else { next - pos };
            if look.length_squared() > 1e-5 {
                // plane (x, y) → world dir (x, 0, -y); mesh yaw is atan2(x, z).
                let want = Quat::from_rotation_y(f32::atan2(look.x, -look.y));
                transform.rotation = transform.rotation.slerp(want, (9.0 * dt).clamp(0.0, 1.0));
            }
        }
    }
}

// Keep the scrap value reachable from `build.rs` without it matching on the
// (private-ish) tier set itself.
pub fn scrap_value(kind: EnemyKind) -> u32 {
    kind.scrap()
}
