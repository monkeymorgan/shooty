//! **Where the bad vibes come from, and how the music comes back.**
//!
//! The town's music has gone, and five people are taking it badly. Each stands
//! somewhere out in the streets, grey and head-shaking, and the waves you fight
//! are not spawned by the arena — they pour out of *them*. A rusher is one
//! person's bad mood made solid.
//!
//! That turns the wave loop into something with a subject. Clear the crowd a
//! given source has put out and they snap out of it: colour floods back, they
//! walk to the bandstand, and they stand there nodding along. And because each
//! of them is one of the five crowd genres in [`skins::BOXY`], each one carries
//! **a stem of the soundtrack** — see `assets/audio/stems/`, built by
//! `tools/gen_stems.py`. Cheer up the hip-hop kid and the drums come back.
//! Cheer up all five and the whole track is playing.
//!
//! The pieces:
//!
//! * [`GloomSource`] — one of the five, with the [`Mood`] it is in.
//! * [`Mood::Brooding`] — the live emitters. `enemy.rs::run_wave` queries for
//!   these and spawns its wave *around* them instead of in a ring around the
//!   party.
//! * [`FromGloom`] — on every enemy, saying whose mood it is. Counting these is
//!   how a source knows its crowd is cleared.
//! * [`Chorus`] — per-stem target volume, read by `audio.rs`. The one thing
//!   that crosses from gameplay into the mixer.
//!
//! **The cure rule** is two facts, both read straight off the field: this
//! source has had at least one of its crowd alive at some point, and right now
//! it has none. That is all. No phase check and no timer, which means a cure
//! can land *mid-wave* — focus one miserable person, clear what they have put
//! out, and their stem joins while you are still fighting. The wave's remaining
//! budget simply redistributes to whoever is still brooding.
//!
//! The `seen` latch is not bookkeeping for its own sake: enemy spawns go
//! through `Commands` and so do not exist until the next flush, and without it
//! a source would read "emitted, nothing alive" for one frame and cure itself
//! for free the instant a wave sent its first batch.

use std::time::Duration;

use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};

use super::enemy::Wave;
use super::player::boxy_atlas;
use super::{
    CurrentLevel, GameState, Obstacle, Player, RunEntity, ground, plane, resolve_obstacles, skins,
};

/// How close a hero has to get to one of the five before they start brooding
/// and their district's waves begin. Sized so walking into a district trips it
/// but the next district over stays quiet — "the area reacts to you".
const WAKE_RADIUS: f32 = 70.0;

/// The five joyless townspeople: which crowd character they are, and where in
/// town they stand.
///
/// The order is the **wake order** — wave 1 rouses the first, wave 2 the
/// second, and so on — so it is also the order the track tends to assemble in.
/// It is arranged as an arrangement would be: kit, then bass, then chords, then
/// the guitar hook, then the topline. Any subset works (the stems are written
/// for that), but built in this order it sounds like a song being counted in.
///
/// **Where** they stand is not here: that is level design, and it lives in
/// `Level::gloom_sites`, index for index with this list. What is here is the
/// part that is the same in every environment — who they are and in what order
/// they wake up.
pub const STEMS: [&str; 5] = ["hiphop", "techno", "disco", "country", "jpop"];

/// Ground a gloom source keeps clear of buildings and street dressing, so you
/// can always walk up to one and see it.
pub const SITE_CLEARANCE: f32 = 6.0;

/// How close a cured source has to get to its slot at the bandstand to count as
/// arrived.
const ARRIVED: f32 = 1.4;

/// March speed on the way to the stage, in units/sec. Brisk — they are keen.
const MARCH_SPEED: f32 = 9.0;

/// Radius of the ring of standing slots around the bandstand.
const STAGE_RING: f32 = 6.5;

/// What state one of the five is in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    /// Miserable, but the waves have not reached them yet — no vibes coming
    /// out. Looks exactly like [`Mood::Brooding`]; the difference is only
    /// whether they are a spawn vent this wave.
    Dormant,
    /// Actively leaking bad vibes. This is a live vent.
    Brooding,
    /// Snapped out of it and walking to the bandstand. Their stem is already
    /// fading up on the way — the payoff should not wait for the walk.
    Marching,
    /// At the stage, nodding along. Stem at full.
    Happy,
}

impl Mood {
    fn cured(self) -> bool {
        matches!(self, Mood::Marching | Mood::Happy)
    }
}

/// One of the five. Sits on the character entity.
#[derive(Component)]
pub struct GloomSource {
    /// Index into [`STEMS`], and into [`Chorus::level`] — so this is also
    /// "which stem is this person".
    pub idx: usize,
    pub mood: Mood,
    /// Where they stand while miserable.
    pub home: Vec2,
    /// Where they stand once they have joined the crowd at the bandstand.
    slot: Vec2,
    /// Has any of this source's crowd ever been alive on the field? Latches
    /// on and never resets — it is what separates "cleared them out" from
    /// "hasn't started yet", including across a wave boundary.
    seen: bool,
    /// Which way they are facing, eased so the turn to the stage is not a snap.
    facing: f32,
}

/// On every enemy: which of the five is responsible for it. Counting the live
/// ones per index is the whole cure test.
#[derive(Component, Clone, Copy)]
pub struct FromGloom(pub usize);

/// On the child entity holding the GLB scene, so the ready-observer can find
/// its way back to the source and dress it.
#[derive(Component)]
struct GloomModel {
    owner: Entity,
    idx: usize,
    gltf: Handle<Gltf>,
}

/// The clips this cast uses, resolved once the GLB is in.
#[derive(Component)]
struct GloomAnim {
    owner: Entity,
    brood: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    cheer: AnimationNodeIndex,
    current: Mood,
}

/// The one material a boxy character wears, kept on the source so the mood can
/// drain and restore its colour without touching the atlas.
#[derive(Component)]
struct GloomLook(Handle<StandardMaterial>);

/// A mark of the misery pouring off a source — how you spot one across town,
/// and how you tell at a glance that it is still pouring. Carries the scale and
/// colour it was spawned at, so `drive_gloom_look` can ride both of them
/// toward zero on the cure without hard-coding either.
#[derive(Component)]
struct GloomAura(Vec3, Color);

/// **The mixer's view of the town.** One target volume per stem, in [`STEMS`]
/// order. `audio.rs` rides the real sink volumes toward these.
#[derive(Resource)]
pub struct Chorus {
    pub level: [f32; STEMS.len()],
    /// How many of the five have been cheered up — for the HUD.
    pub cured: usize,
}

impl Default for Chorus {
    fn default() -> Self {
        Self {
            level: [0.0; STEMS.len()],
            cured: 0,
        }
    }
}

impl Chorus {
    /// Have all five come back? The town is quiet and the song is whole.
    pub fn complete(&self) -> bool {
        self.cured == STEMS.len()
    }
}

/// Drained-out grey. Multiplies the painted atlas, so the character is still
/// legibly *themselves* — same clothes, same silhouette — just with the life
/// pulled out of them. That contrast is the point: you should recognise the
/// disco kid before and after.
const DRAINED: Color = Color::srgb(0.46, 0.47, 0.56);

pub struct GloomPlugin;

impl Plugin for GloomPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Chorus>()
            .add_systems(OnEnter(GameState::Playing), spawn_gloom_cast)
            .add_systems(
                Update,
                (track_gloom, march_to_stage, drive_gloom_anim, drive_gloom_look)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

/// Stand the five out in the town, grey and miserable, with a vent of bad light
/// over each.
fn spawn_gloom_cast(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chorus: ResMut<Chorus>,
    level: Res<CurrentLevel>,
) {
    *chorus = Chorus::default();

    // Standing slots: a ring on the town side of the bandstand, so the crowd
    // gathers *facing* the stage rather than surrounding it.
    let aura_mesh = meshes.add(Cylinder::new(0.5, 1.0));
    let pool_mesh = meshes.add(Cylinder::new(0.5, 1.0));

    for (idx, (id, home)) in STEMS.iter().zip(level.gloom_sites).enumerate() {
        let boxy = skins::boxy(id);
        let ang = std::f32::consts::PI * (0.25 + 0.5 * idx as f32 / (STEMS.len() - 1) as f32);
        let slot = level.stage_site + Vec2::from_angle(ang) * STAGE_RING;

        // Face roughly into town to start with — miserable, but not with their
        // back to you.
        let facing = (-*home).to_angle();

        let source = commands
            .spawn((
                GloomSource {
                    idx,
                    mood: Mood::Dormant,
                    home: *home,
                    slot,
                    seen: false,
                    facing,
                },
                Transform::from_translation(ground(*home, 0.0)),
                Visibility::default(),
                RunEntity,
            ))
            .id();

        // Two marks, because the game is played from a steep top-down camera
        // and a vertical column is mostly edge-on from up there: a stain on the
        // ground that reads at any range, and a sickly haze standing over them
        // that reads against the buildings. Both unlit so they glow at night
        // without lighting the street.
        let pool_scale = Vec3::new(9.0, 0.02, 9.0);
        // Deep enough to read as a stain on pale pavement and to glow on dark
        // tarmac — the first pass was light lavender and vanished on both.
        let pool_color = Color::srgba(0.34, 0.16, 0.62, 0.72);
        commands.spawn((
            GloomAura(pool_scale, pool_color),
            ChildOf(source),
            Mesh3d(pool_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: pool_color,
                emissive: LinearRgba::rgb(0.26, 0.10, 0.62),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.06, 0.0).with_scale(pool_scale),
        ));
        // The plume starts **above head height**. The first version was a wide
        // column centred on the body, and it worked exactly as designed and
        // was wrong: the character disappeared inside their own aura, so you
        // could not see which of the five you were looking at — which is the
        // whole before-and-after the mechanic trades on.
        let haze_scale = Vec3::new(2.8, 7.0, 2.8);
        let haze_color = Color::srgba(0.48, 0.36, 0.80, 0.22);
        commands.spawn((
            GloomAura(haze_scale, haze_color),
            ChildOf(source),
            Mesh3d(aura_mesh.clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: haze_color,
                emissive: LinearRgba::rgb(0.26, 0.14, 0.58),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })),
            Transform::from_xyz(0.0, 6.4, 0.0).with_scale(haze_scale),
        ));

        // A cold, dim lamp so a grey person is still legible standing in an
        // unlit street at night — without it the drained palette reads as a
        // silhouette rather than as somebody you recognise.
        commands.spawn((
            ChildOf(source),
            PointLight {
                color: Color::srgb(0.72, 0.76, 1.0),
                intensity: 220_000.0,
                range: 12.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, 2.6, 1.4),
        ));

        // The GLB spawns asynchronously; `on_gloom_ready` dresses it and wires
        // the three clips this cast needs.
        commands
            .spawn((
                ChildOf(source),
                WorldAssetRoot(
                    assets.load(GltfAssetLabel::Scene(0).from_asset(skins::BOXY_BASE)),
                ),
                GloomModel {
                    owner: source,
                    idx,
                    gltf: assets.load(skins::BOXY_BASE),
                },
                Transform::from_xyz(0.0, skins::BOXY_LIFT, 0.0)
                    .with_scale(Vec3::splat(boxy.model_scale())),
            ))
            .observe(on_gloom_ready);
    }
}

/// Dress the blocky mesh in this character's painted atlas — but through a
/// material *this source owns*, so [`drive_gloom_look`] can drain and restore
/// its colour — and build the three-clip graph the moods run on.
fn on_gloom_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    model: Query<&GloomModel>,
    children: Query<&Children>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let root = ready.entity;
    let Ok(GloomModel { owner, idx, gltf }) = model.get(root) else {
        return;
    };
    let (owner, idx) = (*owner, *idx);
    let Some(gltf) = gltfs.get(gltf) else {
        warn!("gloom glb not in Assets<Gltf> when scene became ready");
        return;
    };

    let atlas = skins::boxy(STEMS[idx]).atlas;
    let mat = materials.add(StandardMaterial {
        base_color: DRAINED,
        base_color_texture: Some(boxy_atlas(&assets, atlas)),
        perceptual_roughness: 0.85,
        ..default()
    });
    for e in children.iter_descendants(root) {
        if mesh_mats.contains(e) {
            commands.entity(e).insert(MeshMaterial3d(mat.clone()));
        }
    }
    commands.entity(owner).insert(GloomLook(mat));

    // `emote-no` is the whole characterisation: a slow, hopeless head-shake.
    // `emote-yes` on the other end is them nodding along to a beat.
    let names = ["emote-no", "walk", "emote-yes"];
    let handles: Vec<Handle<AnimationClip>> = names
        .iter()
        .map(|n| {
            gltf.named_animations.get(*n).cloned().unwrap_or_else(|| {
                warn!("blocky base is missing the `{n}` clip");
                gltf.named_animations
                    .values()
                    .next()
                    .cloned()
                    .expect("blocky base has no animations at all")
            })
        })
        .collect();
    let (graph, node) = AnimationGraph::from_clips(handles);
    let graph = graphs.add(graph);

    let Some(anim_entity) = children
        .iter_descendants(root)
        .find(|e| players.contains(*e))
    else {
        warn!("no AnimationPlayer under the gloom glb scene");
        return;
    };
    let mut player = players.get_mut(anim_entity).unwrap();
    let mut transitions = AnimationTransitions::new();
    transitions.play(&mut player, node[0], Duration::ZERO).repeat();

    commands.entity(anim_entity).insert((
        AnimationGraphHandle(graph),
        transitions,
        GloomAnim {
            owner,
            brood: node[0],
            walk: node[1],
            cheer: node[2],
            current: Mood::Brooding,
        },
    ));
}

/// The state machine: wake a source when a hero comes near, count each one's
/// live crowd, cure the ones that have been emptied, and publish [`Chorus`] for
/// the mixer.
#[allow(clippy::too_many_arguments)]
fn track_gloom(
    time: Res<Time>,
    wave: Option<Res<Wave>>,
    mut sources: Query<(&mut GloomSource, &Transform)>,
    players: Query<&Transform, (With<Player>, Without<GloomSource>)>,
    enemies: Query<&FromGloom>,
    mut chorus: ResMut<Chorus>,
    clock: Option<Res<super::hud::RunClock>>,
    mut cues: MessageWriter<super::audio::AudioCue>,
    mut bursts: MessageWriter<super::vfx::Explosion>,
) {
    let Some(_wave) = wave else { return };

    let heroes: Vec<Vec2> = players.iter().map(|t| plane(t.translation)).collect();

    // Live crowd per source.
    let mut alive = [0u32; STEMS.len()];
    for FromGloom(i) in &enemies {
        if let Some(slot) = alive.get_mut(*i) {
            *slot += 1;
        }
    }

    let mut cured = 0;

    for (mut src, tf) in &mut sources {
        let idx = src.idx;

        // Walk into a source's patch and they start brooding — that is what
        // releases the district's waves. Waking is one-way.
        if src.mood == Mood::Dormant
            && heroes.iter().any(|h| h.distance(src.home) < WAKE_RADIUS)
        {
            src.mood = Mood::Brooding;
        }
        if alive[idx] > 0 {
            src.seen = true;
        }

        // The cure: their crowd was out there, and now none of it is.
        if src.mood == Mood::Brooding && src.seen && alive[idx] == 0 {
            src.mood = Mood::Marching;
            // The run time is in the line on purpose: it is what lets the
            // capture pipeline build a soundtrack whose stems come in at the
            // same moments the footage shows them being earned.
            info!(
                "t={:.2}s gloom source `{}` cheered up — stem {} joins the mix",
                clock.as_ref().map(|c| c.0).unwrap_or(0.0),
                STEMS[idx],
                idx
            );
            cues.write(super::audio::AudioCue::CountIn);
            bursts.write(super::vfx::Explosion {
                pos: tf.translation + Vec3::Y * 1.2,
                kind: super::EnemyKind::Sink,
                power: 1.6,
            });
        }

        if src.mood.cured() {
            cured += 1;
        }

        // The stem swells on the walk and sits at full once they arrive, so the
        // reward starts the instant they turn around.
        let target = match src.mood {
            Mood::Dormant | Mood::Brooding => 0.0,
            Mood::Marching => 0.6,
            Mood::Happy => 1.0,
        };
        let l = &mut chorus.level[idx];
        let k = 1.0 - (-1.2 * time.delta_secs()).exp();
        *l += (target - *l) * k;
    }

    chorus.cured = cured;
}

/// Walk a cured source to its slot at the bandstand, steering round buildings,
/// then turn it to face the stage.
fn march_to_stage(
    time: Res<Time>,
    mut sources: Query<(&mut GloomSource, &mut Transform), Without<Obstacle>>,
    obstacles: Query<(&Transform, &Obstacle)>,
    level: Res<CurrentLevel>,
) {
    let dt = time.delta_secs();
    let solids: Vec<(Vec2, f32)> = obstacles
        .iter()
        .map(|(t, o)| (plane(t.translation), o.radius))
        .collect();

    for (mut src, mut tf) in &mut sources {
        let here = plane(tf.translation);
        let goal = match src.mood {
            Mood::Marching => src.slot,
            _ => {
                // Standing still: just hold the current facing.
                tf.rotation = Quat::from_rotation_y(src.facing);
                continue;
            }
        };

        let to = goal - here;
        if to.length() < ARRIVED {
            src.mood = Mood::Happy;
            info!(
                "`{}` reached the bandstand — stem {} to full",
                STEMS[src.idx], src.idx
            );
            // Turn to the stage now they are there.
            let look = level.stage_site - here;
            src.facing = look.to_angle();
        } else {
            let step = to.normalize_or_zero() * MARCH_SPEED * dt;
            let next = resolve_obstacles(here + step, 0.6, &solids);
            let next = next.clamp(
                -level.arena + Vec2::splat(2.0),
                level.arena - Vec2::splat(2.0),
            );
            tf.translation = ground(next, tf.translation.y);
            let moved = next - here;
            if moved.length_squared() > 1e-6 {
                src.facing = moved.to_angle();
            }
        }
        // Ease the turn. `facing` is a ground-plane angle; the model's yaw runs
        // the other way because screen-up is -Z (see `game::ground`).
        let want = Quat::from_rotation_y(src.facing);
        tf.rotation = tf.rotation.slerp(want, (10.0 * dt).clamp(0.0, 1.0));
    }
}

/// Swap each source's looping clip when its mood changes.
fn drive_gloom_anim(
    sources: Query<&GloomSource>,
    mut anims: Query<(&mut GloomAnim, &mut AnimationTransitions, &mut AnimationPlayer)>,
) {
    for (mut track, mut transitions, mut player) in &mut anims {
        let Ok(src) = sources.get(track.owner) else {
            continue;
        };
        // Dormant and Brooding look the same — both are the head-shake.
        let want = match src.mood {
            Mood::Dormant => Mood::Brooding,
            m => m,
        };
        if want == track.current {
            continue;
        }
        let node = match want {
            Mood::Marching => track.walk,
            Mood::Happy => track.cheer,
            _ => track.brood,
        };
        transitions
            .play(&mut player, node, Duration::from_millis(220))
            .repeat();
        track.current = want;
    }
}

/// Ride each source's colour and its aura with its mood: grey and shrouded
/// while miserable, full-colour and glowing once they have come round.
fn drive_gloom_look(
    time: Res<Time>,
    sources: Query<(&GloomSource, &GloomLook, &Children)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut auras: Query<(
        &mut Transform,
        &MeshMaterial3d<StandardMaterial>,
        &GloomAura,
    )>,
) {
    let dt = time.delta_secs();
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 1.9).sin();

    for (src, look, children) in &sources {
        let cured = src.mood.cured();

        if let Some(mut mat) = materials.get_mut(&look.0) {
            let want = if cured { Color::WHITE } else { DRAINED };
            let (a, b) = (mat.base_color.to_linear(), want.to_linear());
            let k = 1.0 - (-3.0 * dt).exp();
            mat.base_color = Color::LinearRgba(a * (1.0 - k) + b * k);
            // A warm rim once they are happy, so the crowd at the bandstand
            // glows rather than just standing there.
            let glow = if src.mood == Mood::Happy { 0.30 } else { 0.0 };
            let e = mat.emissive;
            let want_e = LinearRgba::rgb(glow, glow * 0.86, glow * 0.55);
            mat.emissive = LinearRgba::rgb(
                e.red + (want_e.red - e.red) * k,
                e.green + (want_e.green - e.green) * k,
                e.blue + (want_e.blue - e.blue) * k,
            );
        }

        for child in children.iter() {
            let Ok((mut tf, mat, base)) = auras.get_mut(child) else {
                continue;
            };
            // The column breathes while it is venting and collapses on the cure.
            let want = if cured {
                0.0
            } else if src.mood == Mood::Brooding {
                1.0 + 0.12 * pulse
            } else {
                0.55
            };
            let k = 1.0 - (-4.0 * dt).exp();
            let cur = base.0.x.max(1e-4);
            let f = (tf.scale.x / cur) + (want - tf.scale.x / cur) * k;
            // The ground stain keeps its thickness; only the column stretches.
            tf.scale = Vec3::new(
                base.0.x * f,
                (base.0.y * f.max(0.001)).max(base.0.y * 0.02),
                base.0.z * f,
            );
            if let Some(mut m) = materials.get_mut(&mat.0) {
                let c = base.1.to_srgba();
                m.base_color = Color::srgba(c.red, c.green, c.blue, c.alpha * f);
            }
        }
    }
}
