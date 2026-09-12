//! M4 — the level loop (see `DESIGN.md` / `ART.md` *Story*).
//!
//! Kills drop **scrap** into a shared co-op pool. A hero presses build to drop a
//! structure at their feet and pay for it; any hero standing close channels it
//! up. Building happens in the calm **between-wave lull** (`enemy::Wave`).
//!
//! * **Loudspeaker** (guitarist, cheap) — a klaxon-on-a-pole. On completion it
//!   *secures* a zone around it: enemies inside are [`Pacified`], the spawn
//!   frontier is pushed past it, and the music bed layers up. It also lays down
//!   suppressing klaxon fire at the zone edge. Permanent.
//! * Loudspeakers **link** when their zones touch. Once [`SPEAKERS_TO_LINK`] of
//!   them form one connected network, the stage can be raised.
//! * **Stage** (drummer, dear, slow) — the full band kit. Raising it on the
//!   linked, secured ground **wins the level** ([`GameState::Victory`]).

use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};

use super::audio::AudioCue;
use super::pickup::EnemyDied;
use super::player::Intent;
use super::scale;
use super::{
    Bullet, Downed, Enemy, GameState, Hitbox, Lifetime, Pacified, Player, RunEntity, ground, plane,
};

pub const SPEAKER_COST: u32 = 18;
const STAGE_COST: u32 = 60;
const SPEAKER_CHANNEL: f32 = 2.6;
const STAGE_CHANNEL: f32 = 6.5;
/// A hero within this of a site advances its channel.
const CHANNEL_REACH: f32 = 6.0;
/// How close a hero has to be to a marked pitch to plant on it. Generous —
/// finding the pitch is the interesting part, not standing on the exact spot.
const PITCH_REACH: f32 = 8.0;
/// Spawns are held back within this of an unfinished site.
pub const LULL_RADIUS: f32 = 20.0;

/// Radius a completed loudspeaker secures / pacifies.
pub const SPEAKER_RADIUS: f32 = 19.0;
/// Two loudspeakers link when their centres are within this — i.e. their zones
/// overlap.
const SPEAKER_LINK_RANGE: f32 = SPEAKER_RADIUS * 1.9;
/// Linked loudspeakers (in one connected network) needed before the stage can
/// be raised.
pub const SPEAKERS_TO_LINK: usize = 3;

/// Radius a completed stage secures.
pub const STAGE_RADIUS: f32 = 30.0;

const SPEAKER_FIRE: f32 = 0.6;
const SPEAKER_RANGE: f32 = SPEAKER_RADIUS + 6.0;
const SPEAKER_DAMAGE: f32 = 13.0;
const SPEAKER_BOLT_SPEED: f32 = 46.0;

/// Vertical klaxon-on-a-pole model. **Measured** native height 2.0 u, centred
/// on its origin (y from -1 to +1) — the old scale treated it as unit-height
/// and buried the pole's foot two units under the pavement.
const SPEAKER_NATIVE_H: f32 = 2.0;
const SPEAKER_SCALE: f32 = scale::SPEAKER * scale::U_PER_M / SPEAKER_NATIVE_H;

/// A patch of ground the band has secured — a loudspeaker's zone or the stage's.
#[derive(Clone, Copy)]
pub struct Zone {
    pub center: Vec2,
    pub radius: f32,
}

/// Shared co-op scrap pool. Enemies drop it; structures cost it.
#[derive(Resource, Default)]
pub struct Scrap(pub u32);

/// Every zone the band holds. Drives the spawn frontier, pacify, and the music
/// bed.
#[derive(Resource, Default)]
pub struct SecuredZones(pub Vec<Zone>);

impl SecuredZones {
    pub fn contains(&self, p: Vec2) -> bool {
        self.0.iter().any(|z| z.center.distance(p) < z.radius)
    }

    /// Shove `p` just outside every zone it sits in, moving it away from `from`.
    pub fn push_out(&self, mut p: Vec2, from: Vec2) -> Vec2 {
        for _ in 0..3 {
            let mut moved = false;
            for z in &self.0 {
                if z.center.distance(p) < z.radius {
                    let dir = (p - from).normalize_or_zero();
                    let dir = if dir == Vec2::ZERO {
                        (p - z.center).normalize_or_zero()
                    } else {
                        dir
                    };
                    let dir = if dir == Vec2::ZERO { Vec2::X } else { dir };
                    p = z.center + dir * (z.radius + 4.0);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        p
    }

    /// How loud the music bed should sit — ramps with secured ground.
    pub fn music_level(&self) -> f32 {
        let staged = self.0.iter().any(|z| z.radius >= STAGE_RADIUS);
        let base = (self.0.len() as f32 / 4.0).min(1.0) * 0.55;
        (base + if staged { 0.35 } else { 0.0 }).min(0.95)
    }
}

/// The loudspeaker network: how big the largest connected cluster is, and the
/// centre-pairs that are linked (for the beams).
#[derive(Resource, Default)]
pub struct SpeakerNet {
    pub largest: usize,
    links: Vec<(Vec2, Vec2)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    Speaker,
    Stage,
}

impl StructureKind {
    fn cost(self) -> u32 {
        match self {
            StructureKind::Speaker => SPEAKER_COST,
            StructureKind::Stage => STAGE_COST,
        }
    }
    fn channel(self) -> f32 {
        match self {
            StructureKind::Speaker => SPEAKER_CHANNEL,
            StructureKind::Stage => STAGE_CHANNEL,
        }
    }
}

/// A dropped-but-unfinished structure. Heroes channel `progress` 0→1.
#[derive(Component)]
pub struct BuildSite {
    pub kind: StructureKind,
    progress: f32,
}

/// A finished loudspeaker — secures its zone and lays down suppressing fire.
#[derive(Component)]
pub struct Speaker {
    fire: Timer,
}

/// A finished stage. Raising it wins the level.
#[derive(Component)]
pub struct Stage;

/// A glowing beam between two linked loudspeakers.
#[derive(Component)]
struct SpeakerLink;

/// A marked pitch on the ground: somewhere a structure is *allowed* to go.
/// Despawned when its structure is planted.
#[derive(Component)]
pub struct Pitch {
    pub at: Vec2,
    pub kind: StructureKind,
}

/// Small salvage mote that homes to the nearest hero and adds to the pool.
#[derive(Component)]
struct ScrapMote {
    value: u32,
    vel: Vec3,
}

pub struct BuildPlugin;

impl Plugin for BuildPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::Playing),
            |mut commands: Commands| {
                commands.insert_resource(Scrap::default());
                commands.insert_resource(SecuredZones::default());
                commands.insert_resource(SpeakerNet::default());
            },
        )
        .add_systems(OnEnter(GameState::Playing), mark_pitches)
        .add_systems(
            Update,
            (
                drop_scrap,
                collect_scrap,
                place_structures,
                channel_sites,
                update_speaker_net,
                speaker_fire,
                pacify_zone,
            )
                .run_if(in_state(GameState::Playing))
                .run_if(super::net::authoritative),
        )
        .add_systems(Update, breathe_pitches.run_if(in_state(GameState::Playing)))
        // On a net client, structures are ghosts raised from the snapshot's
        // secured-zone list (which is append-only on the host).
        .add_systems(
            Update,
            net_apply_zones
                .run_if(in_state(GameState::Playing))
                .run_if(super::net::is_client),
        );
    }
}

/// Net client: for every secured zone the host has that we haven't yet raised a
/// structure ghost for, spawn one (loudspeaker or stage, by radius).
fn net_apply_zones(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    zones: Res<SecuredZones>,
    mut realised: ResMut<super::net::ZonesRealised>,
) {
    while realised.0 < zones.0.len() {
        let z = zones.0[realised.0];
        if z.radius >= STAGE_RADIUS {
            spawn_stage(&mut commands, &mut meshes, &mut materials, z.center);
        } else {
            spawn_speaker(
                &mut commands,
                &assets,
                &mut meshes,
                &mut materials,
                z.center,
            );
        }
        realised.0 += 1;
    }
}

// ---------------------------------------------------------------------------
// Pitches
// ---------------------------------------------------------------------------

/// Paint the town's marked pitches at the start of a run: a pad on the ground
/// with a low box stack on it, in the loudspeaker's own cyan, plus the
/// bandstand pitch the stage goes on in the drummer's amber.
///
/// The marker is built from boxes and a ring like everything else, and it is
/// deliberately readable from the top-down camera at range — the point of
/// fixed pitches is that you can plan a route to them from across the town.
fn mark_pitches(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level: Res<super::CurrentLevel>,
) {
    let pad = meshes.add(Cylinder::new(3.4, 0.08));
    let ring = meshes.add(Torus {
        minor_radius: 0.14,
        major_radius: 3.4,
    });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));

    let mut plot = |commands: &mut Commands, at: Vec2, kind: StructureKind| {
        let (tint, glow) = match kind {
            StructureKind::Speaker => (
                Color::srgba(0.35, 0.85, 1.0, 0.30),
                LinearRgba::rgb(0.12, 0.55, 0.80),
            ),
            StructureKind::Stage => (
                Color::srgba(1.0, 0.68, 0.26, 0.32),
                LinearRgba::rgb(0.85, 0.42, 0.10),
            ),
        };
        let flat = materials.add(StandardMaterial {
            base_color: tint,
            emissive: glow,
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let solid = materials.add(StandardMaterial {
            base_color: Color::srgb(0.14, 0.15, 0.18),
            emissive: glow * 0.35,
            perceptual_roughness: 0.8,
            ..default()
        });
        commands
            .spawn((
                Pitch { at, kind },
                Transform::from_translation(ground(at, 0.02)),
                Visibility::default(),
                RunEntity,
            ))
            .with_children(|c| {
                c.spawn((
                    Mesh3d(pad.clone()),
                    MeshMaterial3d(flat.clone()),
                    Transform::from_xyz(0.0, 0.02, 0.0),
                ));
                c.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(flat.clone()),
                    Transform::from_xyz(0.0, 0.10, 0.0),
                ));
                // Two stubby boxes: the silhouette of a speaker stack waiting
                // to be built, so a pitch reads as a *place for a thing*
                // rather than as decoration.
                for (sx, h) in [(-0.85_f32, 1.1_f32), (0.85, 0.8)] {
                    c.spawn((
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(solid.clone()),
                        Transform::from_xyz(sx, h * 0.5, 0.0)
                            .with_scale(Vec3::new(0.95, h, 0.7)),
                    ));
                }
            });
    };

    for at in level.speaker_sites {
        plot(&mut commands, *at, StructureKind::Speaker);
    }
    plot(&mut commands, level.stage_site, StructureKind::Stage);
}

/// Bob the pitch markers so an empty pitch is visible from across the town.
fn breathe_pitches(time: Res<Time>, mut pitches: Query<(&Pitch, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (pitch, mut transform) in &mut pitches {
        // Offset per pitch so the town doesn't pulse in unison.
        let phase = pitch.at.x * 0.13 + pitch.at.y * 0.07;
        transform.scale = Vec3::splat(1.0 + 0.035 * (t * 1.6 + phase).sin());
    }
}

/// The nearest unbuilt pitch of `kind` within reach of `from`, if any.
fn nearest_pitch(
    from: Vec2,
    kind: StructureKind,
    pitches: &Query<(Entity, &Pitch)>,
) -> Option<(Entity, Vec2)> {
    pitches
        .iter()
        .filter(|(_, p)| p.kind == kind)
        .map(|(e, p)| (e, p.at, p.at.distance(from)))
        .filter(|(_, _, d)| *d <= PITCH_REACH)
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(e, at, _)| (e, at))
}

use super::enemy::scrap_value;

/// Spawn a scrap mote where each enemy fell.
fn drop_scrap(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut died: MessageReader<EnemyDied>,
    mut mote_mesh: Local<Option<Handle<Mesh>>>,
    mut mote_mat: Local<Option<Handle<StandardMaterial>>>,
) {
    // `Local` handles so we make the mesh/material once.
    let mesh = mote_mesh
        .get_or_insert_with(|| meshes.add(Cuboid::new(0.5, 0.5, 0.5)))
        .clone();
    let mat = mote_mat
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgb(0.75, 0.95, 0.55),
                emissive: LinearRgba::rgb(0.6, 1.4, 0.3),
                ..default()
            })
        })
        .clone();
    for EnemyDied { pos, kind } in died.read() {
        commands.spawn((
            ScrapMote {
                value: scrap_value(*kind),
                vel: Vec3::new(fastrand_range(-3.0, 3.0), 6.5, fastrand_range(-3.0, 3.0)),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(ground(plane(*pos), 1.0)).with_scale(Vec3::splat(0.9)),
            RunEntity,
        ));
    }
}

// tiny deterministic-enough jitter without pulling rand into this file's hot path
fn fastrand_range(lo: f32, hi: f32) -> f32 {
    use std::cell::Cell;
    thread_local!(static S: Cell<u32> = const { Cell::new(0x2545F491) });
    S.with(|s| {
        let mut x = s.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        s.set(x);
        lo + (x as f32 / u32::MAX as f32) * (hi - lo)
    })
}

/// Motes arc up, fall, then home to the nearest hero and top up the pool.
fn collect_scrap(
    mut commands: Commands,
    time: Res<Time>,
    mut scrap: ResMut<Scrap>,
    mut cues: MessageWriter<AudioCue>,
    players: Query<&Transform, (With<Player>, Without<ScrapMote>)>,
    mut motes: Query<(Entity, &mut Transform, &mut ScrapMote), Without<Player>>,
) {
    let dt = time.delta_secs();
    for (e, mut t, mut mote) in &mut motes {
        let here = plane(t.translation);
        let nearest = players.iter().map(|p| plane(p.translation)).min_by(|a, b| {
            a.distance_squared(here)
                .total_cmp(&b.distance_squared(here))
        });
        if let Some(target) = nearest {
            let d = target.distance(here);
            if d < 2.0 {
                scrap.0 += mote.value;
                cues.write(AudioCue::Scrap);
                commands.entity(e).despawn();
                continue;
            }
            // Fall until it's near the ground, then home in.
            mote.vel.y -= 22.0 * dt;
            let mut next = t.translation + mote.vel * dt;
            if next.y < 1.0 {
                next.y = 1.0;
                mote.vel.y = 0.0;
                let pull = (ground(target, 1.0) - next).normalize_or_zero() * 26.0;
                mote.vel.x = pull.x;
                mote.vel.z = pull.z;
            }
            t.translation = next;
            t.rotate_y(6.0 * dt);
        }
    }
}

/// A hero pressing build drops a site at their feet and pays for it.
#[allow(clippy::too_many_arguments)]
fn place_structures(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut scrap: ResMut<Scrap>,
    mut cues: MessageWriter<AudioCue>,
    net: Res<SpeakerNet>,
    party: Res<super::Party>,
    players: Query<
        (
            &Transform,
            &Intent,
            &super::Hero,
            Option<&super::drive::Driving>,
        ),
        (With<Player>, Without<Downed>),
    >,
    sites: Query<&Transform, With<BuildSite>>,
    speakers: Query<&Transform, With<Speaker>>,
    stages: Query<&Transform, With<Stage>>,
    pitches: Query<(Entity, &Pitch)>,
) {
    // Solo: there's no drummer to raise the stage, so once the network is
    // linked the guitarist's build press raises it instead of another speaker.
    let solo_stage_ready =
        *party == super::Party::Solo && net.largest >= SPEAKERS_TO_LINK && stages.is_empty();

    for (t, intent, hero, driving) in &players {
        // You have to get out to plant one — a loudspeaker is not a drive-thru.
        if !intent.build || driving.is_some() {
            continue;
        }
        let kind = match hero {
            super::Hero::Guitarist if solo_stage_ready => StructureKind::Stage,
            super::Hero::Guitarist => StructureKind::Speaker,
            super::Hero::Drummer => StructureKind::Stage,
        };
        if scrap.0 < kind.cost() {
            continue;
        }
        // Structures go on **marked pitches**, not wherever a hero happens to
        // be standing: find the nearest free one in reach and build there.
        let Some((pitch, here)) = nearest_pitch(plane(t.translation), kind, &pitches) else {
            continue;
        };
        // Don't stack sites or crowd an existing loudspeaker.
        if sites
            .iter()
            .any(|s| plane(s.translation).distance(here) < 8.0)
        {
            continue;
        }
        if kind == StructureKind::Speaker
            && speakers
                .iter()
                .any(|s| plane(s.translation).distance(here) < 7.0)
        {
            continue;
        }
        if kind == StructureKind::Stage {
            // The stage needs a linked loudspeaker network, and there's only
            // ever one.
            if net.largest < SPEAKERS_TO_LINK || !stages.is_empty() {
                continue;
            }
        }

        scrap.0 -= kind.cost();
        // The pitch has been taken; the marker's job is done.
        commands.entity(pitch).despawn();
        cues.write(AudioCue::Build);
        if kind == StructureKind::Stage {
            cues.write(AudioCue::CountIn);
        }

        let ring = materials.add(StandardMaterial {
            base_color: Color::srgb(0.4, 0.9, 1.0),
            emissive: LinearRgba::rgb(0.3, 1.6, 2.2),
            unlit: true,
            ..default()
        });
        commands
            .spawn((
                BuildSite {
                    kind,
                    progress: 0.0,
                },
                Transform::from_translation(ground(here, 0.05)),
                Visibility::default(),
                RunEntity,
            ))
            .with_children(|c| {
                // A ground ring that brightens/fills as the channel completes.
                c.spawn((
                    Mesh3d(meshes.add(Torus {
                        minor_radius: 0.12,
                        major_radius: CHANNEL_REACH,
                    })),
                    MeshMaterial3d(ring),
                    Transform::from_xyz(0.0, 0.05, 0.0),
                ));
            });
    }
}

/// Heroes standing on a site advance it; when it finishes, swap in the
/// structure. Unattended sites decay slowly.
#[allow(clippy::too_many_arguments)]
fn channel_sites(
    mut commands: Commands,
    time: Res<Time>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cues: MessageWriter<AudioCue>,
    mut zones: ResMut<SecuredZones>,
    mut next_state: ResMut<NextState<GameState>>,
    players: Query<&Transform, (With<Player>, Without<Downed>)>,
    children: Query<&Children>,
    ring_mat: Query<&MeshMaterial3d<StandardMaterial>>,
    mut sites: Query<(Entity, &Transform, &mut BuildSite)>,
) {
    let dt = time.delta_secs();
    for (e, t, mut site) in &mut sites {
        let here = plane(t.translation);
        let helpers = players
            .iter()
            .filter(|p| plane(p.translation).distance(here) <= CHANNEL_REACH)
            .count();
        let rate = match helpers {
            0 => -0.12,
            1 => 1.0,
            _ => 1.8,
        } / site.kind.channel();
        site.progress = (site.progress + rate * dt).clamp(0.0, 1.0);

        // Ride the ring's emissive with progress.
        if let Ok(kids) = children.get(e) {
            for k in kids.iter() {
                if let Ok(m) = ring_mat.get(k)
                    && let Some(mut mat) = materials.get_mut(&m.0)
                {
                    let g = 0.4 + 2.6 * site.progress;
                    mat.emissive = LinearRgba::rgb(0.2 * g, 0.8 * g, 1.1 * g);
                }
            }
        }

        if site.progress >= 1.0 {
            let kind = site.kind;
            commands.entity(e).despawn();
            match kind {
                StructureKind::Speaker => {
                    zones.0.push(Zone {
                        center: here,
                        radius: SPEAKER_RADIUS,
                    });
                    cues.write(AudioCue::ChordStab);
                    cues.write(AudioCue::VoiceSpeaker);
                    spawn_speaker(&mut commands, &assets, &mut meshes, &mut materials, here);
                }
                StructureKind::Stage => {
                    zones.0.push(Zone {
                        center: here,
                        radius: STAGE_RADIUS,
                    });
                    cues.write(AudioCue::ChordStab);
                    cues.write(AudioCue::VoiceStage);
                    spawn_stage(&mut commands, &mut meshes, &mut materials, here);
                    next_state.set(GameState::Victory);
                }
            }
        }
    }
}

/// A loudspeaker: the klaxon GLB on a short concrete footing, a cyan ground
/// disc + perimeter ring marking the zone it holds, and a soft light.
fn spawn_speaker(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec2,
) {
    let footing = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.16, 0.19),
        perceptual_roughness: 0.9,
        ..default()
    });
    // A soft glow pool on the ground marks the secured zone. Overlapping pools
    // from a linked cluster brighten and read as one reclaimed patch — cleaner
    // than a tangle of hard perimeter rings.
    let field = materials.add(StandardMaterial {
        base_color: Color::srgba(0.4, 0.85, 1.0, 0.1),
        emissive: LinearRgba::rgb(0.06, 0.20, 0.30),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    commands
        .spawn((
            Speaker {
                fire: Timer::from_seconds(SPEAKER_FIRE, TimerMode::Repeating),
            },
            Transform::from_translation(ground(at, 0.0)),
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(1.1, 0.4))),
                MeshMaterial3d(footing.clone()),
                Transform::from_xyz(0.0, 0.2, 0.0),
            ));
            // Klaxon model — normalised to unit height, centred on origin.
            c.spawn((
                WorldAssetRoot(
                    assets.load(GltfAssetLabel::Scene(0).from_asset("models/klaxon.glb")),
                ),
                Transform::from_xyz(0.0, 0.4 + 0.5 * SPEAKER_NATIVE_H * SPEAKER_SCALE, 0.0)
                    .with_scale(Vec3::splat(SPEAKER_SCALE)),
            ))
            .observe(on_speaker_ready);
            // Zone glow pool.
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(SPEAKER_RADIUS, 0.05))),
                MeshMaterial3d(field),
                Transform::from_xyz(0.0, 0.08, 0.0),
            ));
            c.spawn((
                PointLight {
                    color: Color::srgb(0.6, 0.88, 1.0),
                    intensity: 600_000.0,
                    range: SPEAKER_RADIUS,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 5.0, 0.0),
            ));
        });
}

/// Tripo bakes the klaxon's cyan horn mouths into the base-colour texture only.
/// Re-point every mesh at a copy that also drives `emissive` off that same
/// texture, so the mouths bloom and the dark body stays dark.
fn on_speaker_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for e in children.iter_descendants(ready.entity) {
        let Ok(m) = mesh_mats.get(e) else { continue };
        let tex = materials
            .get(&m.0)
            .and_then(|s| s.base_color_texture.clone());
        let lit = materials.add(StandardMaterial {
            base_color: Color::srgb(0.52, 0.55, 0.60),
            base_color_texture: tex.clone(),
            emissive_texture: tex,
            emissive: LinearRgba::rgb(0.30, 0.85, 1.15),
            perceptual_roughness: 0.5,
            ..default()
        });
        commands.entity(e).try_insert(MeshMaterial3d(lit));
    }
}

fn spawn_stage(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    at: Vec2,
) {
    let timber = materials.add(StandardMaterial {
        base_color: Color::srgb(0.44, 0.30, 0.18),
        perceptual_roughness: 0.9,
        ..default()
    });
    let field = materials.add(StandardMaterial {
        base_color: Color::srgba(0.5, 0.9, 1.0, 0.14),
        emissive: LinearRgba::rgb(0.10, 0.28, 0.4),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        ..default()
    });
    let rim = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 0.95, 1.0),
        emissive: LinearRgba::rgb(0.6, 2.4, 3.2),
        unlit: true,
        ..default()
    });
    let lights = [
        LinearRgba::rgb(5.0, 0.6, 3.0),
        LinearRgba::rgb(0.4, 4.5, 5.0),
        LinearRgba::rgb(5.0, 3.0, 0.5),
        LinearRgba::rgb(3.5, 0.6, 5.0),
    ];
    commands
        .spawn((
            Stage,
            Transform::from_translation(ground(at, 0.0)),
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            // Two-tier deck so it reads as a raised stage even in a crowd.
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(6.6, 0.5))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(0.0, 0.25, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(5.6, 0.9))),
                MeshMaterial3d(timber.clone()),
                Transform::from_xyz(0.0, 0.9, 0.0),
            ));
            // A back line of amps + a central mic stand so it reads as "the band
            // kit", not just a platform.
            for x in [-3.6, -1.2, 1.2, 3.6] {
                c.spawn((
                    Mesh3d(meshes.add(Cuboid::new(1.5, 1.8, 1.0))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::srgb(0.05, 0.05, 0.06),
                        ..default()
                    })),
                    Transform::from_xyz(x, 2.3, -3.4),
                ));
            }
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.08, 3.4))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.22),
                    metallic: 0.8,
                    ..default()
                })),
                Transform::from_xyz(0.0, 3.1, 1.6),
            ));
            c.spawn((
                Mesh3d(meshes.add(Sphere::new(0.35))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.7, 0.7, 0.72),
                    emissive: LinearRgba::rgb(1.4, 1.4, 1.6),
                    ..default()
                })),
                Transform::from_xyz(0.0, 4.7, 1.6),
            ));
            for (i, col) in lights.into_iter().enumerate() {
                let a = i as f32 / 4.0 * std::f32::consts::TAU;
                let p = Vec2::from_angle(a) * 5.2;
                c.spawn((
                    Mesh3d(meshes.add(Cuboid::new(0.35, 5.0, 0.35))),
                    MeshMaterial3d(timber.clone()),
                    Transform::from_xyz(p.x, 2.5, p.y),
                ));
                c.spawn((
                    Mesh3d(meshes.add(Sphere::new(0.7))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        emissive: col,
                        unlit: true,
                        ..default()
                    })),
                    Transform::from_xyz(p.x, 5.2, p.y),
                ));
            }
            c.spawn((
                Mesh3d(meshes.add(Cylinder::new(STAGE_RADIUS, 0.08))),
                MeshMaterial3d(field),
                Transform::from_xyz(0.0, 0.12, 0.0),
            ));
            c.spawn((
                Mesh3d(meshes.add(Torus {
                    minor_radius: 0.22,
                    major_radius: STAGE_RADIUS,
                })),
                MeshMaterial3d(rim),
                Transform::from_xyz(0.0, 0.15, 0.0),
            ));
            c.spawn((
                PointLight {
                    color: Color::srgb(0.7, 0.9, 1.0),
                    intensity: 900_000.0,
                    range: STAGE_RADIUS,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 6.0, 0.0),
            ));
        });
}

/// Recompute the loudspeaker network (largest linked cluster + link pairs) and
/// keep the beam entities in sync.
fn update_speaker_net(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut net: ResMut<SpeakerNet>,
    mut beam: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut shown: Local<usize>,
    speakers: Query<&Transform, With<Speaker>>,
    links: Query<Entity, With<SpeakerLink>>,
) {
    let pts: Vec<Vec2> = speakers.iter().map(|t| plane(t.translation)).collect();
    let n = pts.len();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    let mut parent: Vec<usize> = (0..n).collect();
    let mut pairs = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            if pts[i].distance(pts[j]) <= SPEAKER_LINK_RANGE {
                pairs.push((pts[i], pts[j]));
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[a] = b;
            }
        }
    }
    let mut sizes = vec![0usize; n];
    for i in 0..n {
        let r = find(&mut parent, i);
        sizes[r] += 1;
    }
    net.largest = sizes.into_iter().max().unwrap_or(0);
    net.links = pairs;

    if *shown == net.links.len() {
        return;
    }
    *shown = net.links.len();
    for e in &links {
        commands.entity(e).despawn();
    }
    let (mesh, mat) = beam
        .get_or_insert_with(|| {
            (
                meshes.add(Cuboid::new(0.7, 0.06, 1.0)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.55, 0.95, 1.0),
                    emissive: LinearRgba::rgb(0.7, 3.0, 3.8),
                    unlit: true,
                    ..default()
                }),
            )
        })
        .clone();
    for (a, b) in net.links.clone() {
        let mid = (a + b) * 0.5;
        let d = b - a;
        let len = d.length().max(0.01);
        commands.spawn((
            SpeakerLink,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(ground(mid, 0.3))
                .with_rotation(Quat::from_rotation_y(f32::atan2(d.x, -d.y)))
                .with_scale(Vec3::new(1.0, 1.0, len)),
            RunEntity,
        ));
    }
}

/// Loudspeakers lay suppressing fire on the nearest live enemy at their zone
/// edge.
fn speaker_fire(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut bolt: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut speakers: Query<(&GlobalTransform, &mut Speaker)>,
    enemies: Query<&Transform, (With<Enemy>, Without<Pacified>)>,
) {
    let (mesh, mat) = bolt
        .get_or_insert_with(|| {
            (
                meshes.add(Sphere::new(0.3)),
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.7, 1.0, 1.0),
                    emissive: LinearRgba::rgb(1.4, 4.0, 4.6),
                    unlit: true,
                    ..default()
                }),
            )
        })
        .clone();

    for (gt, mut speaker) in &mut speakers {
        if !speaker.fire.tick(time.delta()).just_finished() {
            continue;
        }
        let origin = gt.translation() + Vec3::Y * 3.4;
        let from = plane(origin);
        let Some(target) = enemies
            .iter()
            .map(|t| plane(t.translation))
            .filter(|p| p.distance(from) <= SPEAKER_RANGE)
            .min_by(|a, b| {
                a.distance_squared(from)
                    .total_cmp(&b.distance_squared(from))
            })
        else {
            continue;
        };
        let dir = (ground(target, 1.2) - origin).normalize_or_zero();
        commands.spawn((
            Bullet {
                velocity: dir * SPEAKER_BOLT_SPEED,
                damage: SPEAKER_DAMAGE,
            },
            Hitbox(0.5),
            Lifetime(Timer::from_seconds(1.2, TimerMode::Once)),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat.clone()),
            Transform::from_translation(origin),
            RunEntity,
        ));
    }
}

/// Enemies inside a secured zone get pacified: no contact damage, a short fuse,
/// and `move_enemies` drifts them back out.
fn pacify_zone(
    mut commands: Commands,
    zones: Res<SecuredZones>,
    mut cues: MessageWriter<AudioCue>,
    enemies: Query<(Entity, &Transform), (With<Enemy>, Without<Pacified>)>,
) {
    if zones.0.is_empty() {
        return;
    }
    for (e, t) in &enemies {
        if zones.contains(plane(t.translation)) {
            commands.entity(e).try_insert((
                Pacified,
                Lifetime(Timer::from_seconds(3.5, TimerMode::Once)),
            ));
            cues.write(AudioCue::Beat);
        }
    }
}

/// Read by `enemy::move_enemies` — the vector that drifts a pacified enemy back
/// out of the nearest zone.
pub fn drift_out(zones: &SecuredZones, p: Vec2) -> Vec2 {
    zones
        .0
        .iter()
        .min_by(|a, b| {
            a.center
                .distance_squared(p)
                .total_cmp(&b.center.distance_squared(p))
        })
        .map(|z| (p - z.center).normalize_or_zero())
        .unwrap_or(Vec2::ZERO)
}
