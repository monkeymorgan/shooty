use std::time::Duration;

use bevy::animation::RepeatAnimation;
use bevy::gltf::GltfMaterialName;
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};

use super::props::{self, Carry, Prop};
use super::roster::{Gait, Pick, Roster};
use super::{
    AutoPlay, CurrentLevel, Downed, Enemy, GameState, Health, Hero, Hitbox, Player, RunEntity,
    ground, plane,
};

const MOVE_SPEED: f32 = 10.5;
const ROLL_SPEED: f32 = 29.0;
const ROLL_TIME: f32 = 0.22;
const ROLL_COOLDOWN: f32 = 0.75;
pub const PLAYER_RADIUS: f32 = 1.0;
const BODY_Y: f32 = 0.0;
// A hero's model scale is no longer a constant here: it comes from the height
// in metres the character's `skins::Skin` / `skins::Boxy` entry declares, via
// `roster::Pick::model_scale`. The collision + combat distances (arena, speeds,
// hitboxes, build radii) are a separate, larger layer and unaffected by it.

/// Where each hero starts, either side of the central crossroads.
const GUITARIST_SPAWN: Vec2 = Vec2::new(-5.0, 0.0);
const DRUMMER_SPAWN: Vec2 = Vec2::new(5.0, 0.0);

#[derive(Component)]
pub struct Aim(pub Vec3);

/// Set by `movement` each frame so the animation driver can pick a gait.
#[derive(Component, Default)]
pub struct Moving(pub bool);

/// Per-player intent for this frame, filled by the input systems (`p1_input`,
/// `p2_input`, or the capture bot) and consumed by `movement` and the two
/// weapon kits. Decouples the sim from which device — or script — is driving.
#[derive(Component, Default)]
pub struct Intent {
    /// Desired ground-plane move direction, magnitude 0..1.
    pub move_dir: Vec2,
    /// Ranged fire held (guitarist).
    pub fire: bool,
    /// Melee swing pressed this frame (drummer).
    pub attack: bool,
    /// Special pressed this frame — Encore blast (guitarist) / beat slam (drummer).
    pub special: bool,
    /// Dodge-roll pressed this frame.
    pub dodge: bool,
    /// Build pressed this frame — plant a loudspeaker (guitarist) / raise the
    /// stage (drummer). See `build.rs`.
    pub build: bool,
    /// Interact pressed this frame — get into, or out of, the nearest car.
    /// See `drive.rs`.
    pub interact: bool,
    /// Stick, for flight: +1 climbing, -1 descending. See `fly.rs`. It shares
    /// the dodge key, which is safe because `movement` ignores a hero who is
    /// flying, so only one of the two can ever act on a press.
    pub climb: f32,
}

/// Bridges the async-spawned `AnimationPlayer` (buried in the GLB scene) to the
/// gait state machine. Lives on the same entity as that `AnimationPlayer`;
/// `owner` points back at the `Player` whose state drives it.
#[derive(Component)]
struct PlayerAnim {
    owner: Entity,
    /// One graph node per [`Gait`], in `Gait::ALL` order. Keeping it a lookup
    /// rather than six named fields is what lets the two rigs — which share no
    /// clip names at all — feed the same state machine.
    clips: [AnimationNodeIndex; Gait::ALL.len()],
    current: Gait,
}

impl PlayerAnim {
    fn clip(&self, gait: Gait) -> AnimationNodeIndex {
        self.clips[Gait::ALL.iter().position(|g| *g == gait).unwrap_or(0)]
    }
}

/// On a hero's model-root child: everything `on_model_ready` needs once the GLB
/// scene has instantiated.
#[derive(Component)]
struct HeroModel {
    owner: Entity,
    hero: Hero,
    pick: Pick,
    gltf: Handle<Gltf>,
}

/// The rocker's red guitar-gun. Parented to a hand node of the guitarist's
/// animated mesh (`attach_guns`) so it rides the animation; shots leave from
/// its [`Muzzle`].
#[derive(Component)]
pub struct GuitarGun {
    /// Which hand — `false` = right (the starting gun), `true` = left (pickup).
    pub left: bool,
    /// How it is worn right now. `None` until it has been mounted at all.
    pub carry: Option<Carry>,
}

/// Tip of a guitar-gun's neck; `weapon::fire` reads its `GlobalTransform` for
/// the shot origin.
#[derive(Component)]
pub struct Muzzle;

/// Tags a mount bone *of the guitarist's* skeleton — `Wrist.L`, `Wrist.R` or
/// `Chest` — so gun attachment doesn't grab an identically-named bone on a goon
/// or the drummer. The name is kept so [`carry_guns`] can ask for the bone a
/// carry pose wants by [`Carry::bone`].
#[derive(Component)]
struct HeroBone(&'static str);

/// Bright ground ring under a hero so they never get lost in the swarm — cyan
/// for the guitarist, amber for the drummer.
#[derive(Component)]
struct HeroRing;

#[derive(Component)]
pub struct Dodge {
    pub cooldown: Timer,
    pub active: Option<(Timer, Vec3)>,
}

impl Default for Dodge {
    fn default() -> Self {
        let mut cooldown = Timer::from_seconds(ROLL_COOLDOWN, TimerMode::Once);
        cooldown.tick(cooldown.duration());
        Self {
            cooldown,
            active: None,
        }
    }
}

impl Dodge {
    pub fn is_rolling(&self) -> bool {
        self.active.is_some()
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_players)
            .add_systems(
                Update,
                (p1_input, p2_input, bot_input, movement)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (
                    drive_animation,
                    carry_guns,
                    hide_gun_while_rolling,
                    hide_body_in_car,
                ),
            )
            // After transform propagation so it reads *this* frame's arm pose;
            // it patches the gun + parts' `GlobalTransform`s in place.
            .add_systems(PostUpdate, aim_guns.after(TransformSystems::Propagate));
    }
}

fn spawn_players(
    mut commands: Commands,
    party: Res<super::Party>,
    roster: Res<Roster>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    spawn_hero(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        Hero::Guitarist,
        roster.of(Hero::Guitarist),
    );
    if *party == super::Party::Duo {
        spawn_hero(
            &mut commands,
            &assets,
            &mut meshes,
            &mut materials,
            Hero::Drummer,
            roster.of(Hero::Drummer),
        );
    }
}

fn spawn_hero(
    commands: &mut Commands,
    assets: &AssetServer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    hero: Hero,
    pick: Pick,
) {
    let (spawn, hp, ring_color, ring_emissive) = match hero {
        Hero::Guitarist => (
            GUITARIST_SPAWN,
            200.0,
            Color::srgb(0.20, 0.85, 0.95),
            LinearRgba::rgb(0.2, 2.6, 3.2),
        ),
        Hero::Drummer => (
            DRUMMER_SPAWN,
            230.0,
            Color::srgb(1.0, 0.62, 0.22),
            LinearRgba::rgb(3.0, 1.3, 0.25),
        ),
    };
    let model_path = pick.model();

    let player = commands
        .spawn((
            Player,
            hero,
            pick,
            Aim(Vec3::NEG_Z),
            Dodge::default(),
            Moving(false),
            Intent::default(),
            Health::new(hp),
            Hitbox(PLAYER_RADIUS),
            Transform::from_translation(ground(spawn, BODY_Y)),
            Visibility::default(),
            RunEntity,
        ))
        .id();

    // Bright ground ring + warm key light, parented so they follow the hero.
    commands.spawn((
        HeroRing,
        ChildOf(player),
        // Snug to the person-scale model — a findability halo, not a hula hoop.
        Mesh3d(meshes.add(Torus {
            minor_radius: 0.09,
            major_radius: 1.15,
        })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: ring_color,
            emissive: ring_emissive,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.08, 0.0),
    ));
    commands.spawn((
        ChildOf(player),
        PointLight {
            color: Color::srgb(1.0, 0.93, 0.82),
            intensity: 550_000.0,
            range: 16.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 3.4, 1.2),
    ));

    match hero {
        Hero::Guitarist => {
            // Spawned unparented; `attach_guns` clips it into the right hand.
            spawn_guitar_gun(commands, meshes, materials, false);
        }
        Hero::Drummer => {
            commands
                .entity(player)
                .insert(super::drummer::DrumKit::default());
        }
    }

    // The GLB scene spawns asynchronously as descendants of this child; its
    // baked `AnimationPlayer` is wired up by `on_model_ready` once ready.
    commands
        .spawn((
            ChildOf(player),
            WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(model_path))),
            HeroModel {
                owner: player,
                hero,
                pick,
                gltf: assets.load(model_path),
            },
            Transform::from_xyz(0.0, pick.lift(), 0.0).with_scale(Vec3::splat(pick.model_scale())),
        ))
        .observe(on_model_ready);
}

/// A chunky low-poly flying-V guitar held like a gun: red body with a forked
/// tail, a black neck ending in a glowing muzzle. Spawned unparented; a caller
/// clips it into a hand node of the animated mesh. Returns the gun entity.
pub fn spawn_guitar_gun(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    left: bool,
) -> Entity {
    let red = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.09, 0.12),
        emissive: LinearRgba::rgb(0.45, 0.03, 0.04),
        perceptual_roughness: 0.3,
        ..default()
    });
    let black = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.05, 0.06),
        perceptual_roughness: 0.45,
        ..default()
    });
    let glow = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.85, 0.4),
        emissive: LinearRgba::rgb(4.0, 2.2, 0.6),
        unlit: true,
        ..default()
    });

    // Compact body/fork behind the origin (-Z); a long neck + muzzle run
    // forward (+Z) so from the top-down camera it reads as a barrel.
    let prong = meshes.add(Cuboid::new(0.2, 0.36, 0.6));
    commands
        .spawn((
            GuitarGun { left, carry: None },
            Transform::from_scale(Vec3::ZERO), // real transform set on attach
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            for sign in [-1.0_f32, 1.0] {
                c.spawn((
                    Mesh3d(prong.clone()),
                    MeshMaterial3d(red.clone()),
                    Transform::from_xyz(sign * 0.28, 0.0, -0.62)
                        .with_rotation(Quat::from_rotation_y(-sign * 0.5)),
                ));
            }
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.44, 0.38, 0.6))),
                MeshMaterial3d(red.clone()),
                Transform::from_xyz(0.0, 0.0, -0.35),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.15, 0.13, 2.0))),
                MeshMaterial3d(black.clone()),
                Transform::from_xyz(0.0, 0.02, 0.95),
            ));
            c.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.28, 0.13, 0.34))),
                MeshMaterial3d(black),
                Transform::from_xyz(0.0, 0.02, 2.1),
            ));
            c.spawn((
                Mesh3d(meshes.add(Sphere::new(0.14))),
                MeshMaterial3d(glow),
                Transform::from_xyz(0.0, 0.02, 2.28),
            ));
            c.spawn((Muzzle, Transform::from_xyz(0.0, 0.02, 2.5)));
        })
        .id()
}

/// Local transform for the synty rect-eye **face plane** on a character's `Head`
/// bone (Quaternius meshes have no face UV — see memory `shooty-people-artdir`).
/// The face texture is RGBA with a transparent field, so only the features land
/// on the character's own head skin; the quad just has to sit a hair proud of
/// the face and roughly cover it. Shared by `player.rs` and `enemy.rs` (and the
/// asset lab) so the number lives in one place.
pub fn face_in_head() -> Transform {
    Transform::from_xyz(0.0, 0.055, 0.07).with_scale(Vec3::new(0.20, 0.24, 1.0))
}

/// The `StandardMaterial` for a face plane, given its RGBA feature texture.
pub fn face_material(texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(texture),
        perceptual_roughness: 0.9,
        alpha_mode: AlphaMode::Blend,
        ..default()
    }
}

/// Point every guitar-gun's neck (+Z) down the guitarist's aim in world space,
/// cancelling whatever the animated arm is doing.
///
/// Runs *after* transform propagation and writes the gun's — and its parts' —
/// `GlobalTransform`s directly, so the aim is built from this frame's arm pose.
fn aim_guns(
    guitarist: Query<(&Aim, &Hero), With<Player>>,
    parts: Query<&Children>,
    part_local: Query<&Transform, Without<GuitarGun>>,
    mut guns: Query<(
        Entity,
        &ChildOf,
        &mut Transform,
        &mut GlobalTransform,
        &GuitarGun,
    )>,
    mut gts: ParamSet<(
        Query<&GlobalTransform, Without<GuitarGun>>,
        Query<&mut GlobalTransform, Without<GuitarGun>>,
    )>,
) {
    let Some((aim, _)) = guitarist.iter().find(|(_, h)| **h == Hero::Guitarist) else {
        return;
    };
    let a = aim.0.normalize_or_zero();
    if a == Vec3::ZERO {
        return;
    }
    let want = Quat::from_rotation_y(f32::atan2(a.x, a.z))
        * Quat::from_rotation_x(-0.08)
        * Quat::from_rotation_z(0.12);

    let mut fixed: Vec<(Entity, GlobalTransform)> = Vec::new();
    for (gun, parent, mut t, mut g, carry) in &mut guns {
        // Only the hand pose tracks the cursor. A slung guitar hangs on its
        // strap and is driven by the animation like any other worn thing.
        if carry.carry != Some(Carry::Aim) {
            continue;
        }
        let arm = gts.p0().get(parent.parent()).copied().unwrap_or_default();
        t.rotation = arm.rotation().inverse() * want;
        let global = arm.mul_transform(*t);
        *g = global;
        fixed.push((gun, global));
    }

    let mut child_gts = gts.p1();
    for (gun, global) in fixed {
        let Ok(children) = parts.get(gun) else {
            continue;
        };
        for child in children.iter() {
            let local = part_local.get(child).copied().unwrap_or_default();
            if let Ok(mut cg) = child_gts.get_mut(child) {
                *cg = global.mul_transform(local);
            }
        }
    }
}

/// Hold the aim pose for a beat after the last shot, so tapping fire doesn't
/// make the guitar snap between the chest and the hand every frame.
const AIM_LINGER: f32 = 0.45;

/// Put every guitar-gun in the pose the guitarist's state calls for, moving it
/// between mount bones when that changes.
///
/// **Slung across the chest is the resting pose.** The gun used to live on the
/// wrist permanently with `aim_guns` twisting it at the cursor every frame,
/// which read as a guitar floating off the hand at an odd angle through every
/// walk, idle and roll. Now it hangs on its strap like an instrument and only
/// comes up to the hand while the guitarist is actually shooting.
///
/// This also covers first attachment — a freshly spawned gun has no parent and
/// no `carry`, so it falls into the same path.
fn carry_guns(
    mut commands: Commands,
    time: Res<Time>,
    guitarist: Query<(&Intent, &Hero, &Pick), With<Player>>,
    bones: Query<(Entity, &HeroBone)>,
    mut guns: Query<(Entity, &mut GuitarGun)>,
    mut since_fire: Local<f32>,
) {
    let Some((intent, _, pick)) = guitarist.iter().find(|(_, h, _)| **h == Hero::Guitarist) else {
        return;
    };
    let (firing, rig) = (intent.fire, pick.rig());
    *since_fire = if firing {
        0.0
    } else {
        *since_fire + time.delta_secs()
    };
    let want = if *since_fire < AIM_LINGER {
        Carry::Aim
    } else {
        Carry::Slung
    };

    for (gun, mut g) in &mut guns {
        if g.carry == Some(want) {
            continue;
        }
        let bone_name = want.bone(rig, g.left);
        let Some((bone, _)) = bones.iter().find(|(_, b)| b.0 == bone_name) else {
            // The skeleton has not instantiated yet; try again next frame.
            continue;
        };
        commands.entity(gun).insert((
            ChildOf(bone),
            props::carry_transform(want, Prop::FlyingV, rig, g.left),
        ));
        g.carry = Some(want);
    }
}

/// Hide a hero's model while they are driving a car their rig has no sitting
/// pose for. The blocky rig has a `drive` clip and stays visible at the wheel;
/// the skeletal one would be stood bolt upright through the roof, so it gets
/// out of sight instead.
fn hide_body_in_car(
    heroes: Query<
        (
            &Pick,
            Option<&super::drive::Driving>,
            Option<&super::fly::Flying>,
            &Children,
        ),
        With<Player>,
    >,
    mut models: Query<&mut Visibility, With<HeroModel>>,
) {
    for (pick, driving, flying, kids) in &heroes {
        // Nothing in any pack sits in a cockpit, and the fuselage is smaller
        // than a person, so a pilot is simply not drawn.
        let hide = flying.is_some() || (driving.is_some() && !super::drive::sits_visibly(*pick));
        for kid in kids.iter() {
            if let Ok(mut vis) = models.get_mut(kid) {
                *vis = if hide {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                };
            }
        }
    }
}

fn hide_gun_while_rolling(
    guitarist: Query<(&Dodge, &Hero, Option<&super::drive::Driving>), With<Player>>,
    mut guns: Query<&mut Visibility, With<GuitarGun>>,
) {
    let rolling = guitarist
        .iter()
        .find(|(_, h, _)| **h == Hero::Guitarist)
        .is_some_and(|(d, _, driving)| d.is_rolling() || driving.is_some());
    for mut v in &mut guns {
        *v = if rolling {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
}

/// When a hero's GLB scene has finished instantiating, find its `AnimationPlayer`,
/// build a graph from the clips we use, recolour the marked materials, and tag
/// its wrist bones (if it's the guitarist).
fn on_model_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    model: Query<&HeroModel>,
    children: Query<&Children>,
    names: Query<&Name>,
    mat_names: Query<&GltfMaterialName>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    gltfs: Res<Assets<Gltf>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    use std::collections::HashMap;

    let root = ready.entity;
    let Ok(HeroModel {
        owner,
        hero,
        pick,
        gltf: handle,
    }) = model.get(root)
    else {
        return;
    };
    let (hero, pick) = (*hero, *pick);
    let Some(gltf) = gltfs.get(handle) else {
        warn!("hero glb not in Assets<Gltf> when scene became ready");
        return;
    };

    // A boxy character wears one painted atlas on every part — hair, face,
    // collar and boot are all in the texture, so there is nothing to tint and
    // no face plane to hang. A skeletal one is the opposite: flat-coloured
    // materials recoloured by name, with the face on a quad.
    if let Some(atlas) = pick.atlas() {
        let dressed = materials.add(StandardMaterial {
            base_color_texture: Some(boxy_atlas(&assets, atlas)),
            perceptual_roughness: 0.82,
            ..default()
        });
        for e in children.iter_descendants(root) {
            if mesh_mats.contains(e) {
                commands.entity(e).insert(MeshMaterial3d(dressed.clone()));
            }
        }
    }

    // Clone-and-recolour each tinted material once, sharing the result across
    // every primitive that used the same source material.
    let mut recoloured: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>> =
        HashMap::new();
    let mut head = None;
    for e in children.iter_descendants(root) {
        if let (Ok(mat_name), Ok(cur)) = (mat_names.get(e), mesh_mats.get(e))
            && let Some(tint) = pick.tint(mat_name.0.as_str())
        {
            let src = cur.id();
            let new = recoloured
                .entry(src)
                .or_insert_with(|| {
                    let mut m = materials.get(src).cloned().unwrap_or_default();
                    m.base_color = tint;
                    materials.add(m)
                })
                .clone();
            commands.entity(e).insert(MeshMaterial3d(new));
        }

        if let Ok(name) = names.get(e) {
            let name = name.as_str();
            if name == "Head" {
                head = Some(e);
            }
            if pick.hides(name) {
                commands.entity(e).insert(Visibility::Hidden);
            }
            // Tag the mount bones *this* rig's carry poses ask for, so gun
            // attachment can't grab an identically-named bone on a goon or on
            // the other hero.
            if hero == Hero::Guitarist
                && let Some(bone) = mount_bone(pick, name)
            {
                commands.entity(e).insert(HeroBone(bone));
            }
        }
    }

    // The synty rect-eye face rides a small quad on the `Head` bone (the
    // Quaternius meshes have no face UV — see memory `shooty-people-artdir`).
    if let (Some(head), Some(face)) = (head, pick.face()) {
        commands.spawn((
            ChildOf(head),
            Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial3d(materials.add(face_material(assets.load(face)))),
            face_in_head(),
        ));
    }

    // One clip per gait, named for whichever rig this character is on.
    let armed = hero == Hero::Guitarist;
    let mut missing = Vec::new();
    let handles: Vec<Handle<AnimationClip>> = Gait::ALL
        .iter()
        .map(|g| {
            let name = pick.clip(*g, armed);
            gltf.named_animations.get(name).cloned().unwrap_or_else(|| {
                missing.push(name);
                // Fall back to the first clip rather than panicking: a missing
                // clip should cost you an animation, not the run.
                gltf.named_animations
                    .values()
                    .next()
                    .cloned()
                    .expect("character glb has no animations at all")
            })
        })
        .collect();
    if !missing.is_empty() {
        warn!("{} is missing clips {:?}", pick.id(), missing);
    }
    let (graph, idx) = AnimationGraph::from_clips(handles);
    let graph = graphs.add(graph);

    let Some(anim_entity) = children
        .iter_descendants(root)
        .find(|e| players.contains(*e))
    else {
        warn!("no AnimationPlayer found under hero glb scene");
        return;
    };
    let mut player = players.get_mut(anim_entity).unwrap();

    let mut transitions = AnimationTransitions::new();
    transitions
        .play(&mut player, idx[0], Duration::ZERO)
        .repeat();

    commands.entity(anim_entity).insert((
        AnimationGraphHandle(graph),
        transitions,
        PlayerAnim {
            owner: *owner,
            clips: std::array::from_fn(|i| idx[i]),
            current: Gait::Idle,
        },
    ));
}

/// Is `name` a bone one of this character's carry poses hangs gear off? Returns
/// the `&'static str` so [`HeroBone`] can hold it and [`carry_guns`] can match
/// it against [`Carry::bone`] later.
fn mount_bone(pick: Pick, name: &str) -> Option<&'static str> {
    Carry::ALL
        .iter()
        .flat_map(|c| [c.bone(pick.rig(), false), c.bone(pick.rig(), true)])
        .find(|b| *b == name)
}

/// Load a painted atlas with the sampler the Kenney blocky meshes need.
///
/// Their UVs run outside [0,1] and only land on the right atlas island under
/// **Repeat** addressing — the default clamp sampler squashes every body part
/// onto the atlas edge, which reads as a character dipped in one colour.
pub fn boxy_atlas(assets: &AssetServer, path: &str) -> Handle<Image> {
    use bevy::image::{
        ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor,
    };
    assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Nearest,
                min_filter: ImageFilterMode::Nearest,
                ..default()
            });
        })
        .load(path.to_string())
}

/// Picks a gait from each hero's state and cross-fades to it.
fn drive_animation(
    state: Res<State<GameState>>,
    players: Query<
        (
            &Hero,
            &Dodge,
            &Moving,
            &Intent,
            Option<&Downed>,
            Option<&super::drive::Driving>,
            Option<&super::drummer::DrumKit>,
        ),
        With<Player>,
    >,
    mut anim: Query<(
        &mut AnimationPlayer,
        &mut AnimationTransitions,
        &mut PlayerAnim,
    )>,
) {
    for (mut anim_player, mut transitions, mut track) in &mut anim {
        let Ok((hero, dodge, moving, intent, downed, driving, melee)) = players.get(track.owner)
        else {
            continue;
        };

        let gait = if *state.get() == GameState::GameOver || downed.is_some() {
            Gait::Die
        } else if driving.is_some() {
            Gait::Drive
        } else if dodge.is_rolling() {
            Gait::Sprint
        } else if melee.is_some_and(|m| m.is_swinging()) {
            Gait::Melee
        } else if *hero == Hero::Guitarist && intent.fire {
            Gait::Shoot
        } else if moving.0 {
            Gait::Walk
        } else {
            Gait::Idle
        };

        if gait == track.current {
            continue;
        }

        let active = transitions.play(
            &mut anim_player,
            track.clip(gait),
            Duration::from_millis(120),
        );
        match gait {
            Gait::Die => {
                active.set_repeat(RepeatAnimation::Never);
            }
            Gait::Melee => {
                active.set_repeat(RepeatAnimation::Never);
            }
            _ => {
                active.repeat();
            }
        }
        track.current = gait;
    }
}

/// Nearest enemy on the ground plane to `from`.
fn nearest_enemy(
    from: Vec2,
    enemies: &Query<&Transform, (With<Enemy>, Without<Player>)>,
) -> Option<Vec2> {
    enemies.iter().map(|t| plane(t.translation)).min_by(|a, b| {
        a.distance_squared(from)
            .total_cmp(&b.distance_squared(from))
    })
}

/// P1 — keyboard + mouse, the guitarist. WASD move, cursor aim (ground
/// raycast), hold LMB fire, RMB Encore, Space dodge.
fn p1_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    autoplay: Option<Res<AutoPlay>>,
    role: Res<super::net::NetRole>,
    camera: Option<Single<(&Camera, &GlobalTransform)>>,
    window: Option<Single<&Window>>,
    mut guitarist: Query<(&Transform, &mut Aim, &mut Intent, &Hero), With<Player>>,
) {
    // On a net client the guitarist is a remote ghost, driven by the snapshot.
    if autoplay.is_some() || *role == super::net::NetRole::Client {
        return;
    }
    let Some((transform, mut aim, mut intent, _)) =
        guitarist.iter_mut().find(|(.., h)| **h == Hero::Guitarist)
    else {
        return;
    };

    // WASD only — the arrow keys are the drummer's keyboard-fallback move set.
    let mut mv = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        mv.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        mv.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        mv.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        mv.x += 1.0;
    }
    intent.move_dir = mv.normalize_or_zero();
    intent.fire = buttons.pressed(MouseButton::Left);
    intent.special = buttons.just_pressed(MouseButton::Right);
    intent.dodge = keys.just_pressed(KeyCode::Space);
    intent.climb = if keys.pressed(KeyCode::Space) {
        1.0
    } else if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        -1.0
    } else {
        0.0
    };
    intent.build = keys.just_pressed(KeyCode::KeyQ);
    intent.interact = keys.just_pressed(KeyCode::KeyE);
    intent.attack = false;

    // Cursor → ground plane → aim direction from the player.
    if let (Some(camera), Some(window)) = (camera, window) {
        let (camera, cam_t) = *camera;
        let ppos = plane(transform.translation);
        let hit = window
            .cursor_position()
            .and_then(|c| camera.viewport_to_world(cam_t, c).ok())
            .and_then(|ray| {
                let dy = ray.direction.y;
                (dy.abs() > 1e-4).then(|| {
                    let t = -ray.origin.y / dy;
                    plane(ray.origin + *ray.direction * t)
                })
            });
        if let Some(t) = hit {
            let d = (t - ppos).normalize_or_zero();
            if d != Vec2::ZERO {
                aim.0 = ground(d, 0.0);
            }
        }
    }
}

/// P2 — the drummer. Gamepad first (left stick move, right stick aim, South =
/// drum-swing, West = beat slam, East = dodge); the keyboard fallback set
/// (arrows move, `/` swing, `,` slam, `.` dodge) is always live too so one
/// person can drive both heroes.
fn p2_input(
    keys: Res<ButtonInput<KeyCode>>,
    autoplay: Option<Res<AutoPlay>>,
    role: Res<super::net::NetRole>,
    pads: Query<&Gamepad>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
    mut drummer: Query<(&Transform, &mut Aim, &mut Intent, &Hero), With<Player>>,
) {
    // On the host of a net game the drummer is driven by the remote client's
    // input (`net::apply_remote_intent`); locally and on the client it's P2.
    if autoplay.is_some() || *role == super::net::NetRole::Host {
        return;
    }
    let Some((transform, mut aim, mut intent, _)) =
        drummer.iter_mut().find(|(.., h)| **h == Hero::Drummer)
    else {
        return;
    };
    let ppos = plane(transform.translation);

    let mut mv = Vec2::ZERO;
    let mut stick_aim = Vec2::ZERO;
    let (mut swing, mut slam, mut dodge) = (false, false, false);

    if let Some(pad) = pads.iter().next() {
        let lx = pad.get(GamepadAxis::LeftStickX).unwrap_or(0.0);
        let ly = pad.get(GamepadAxis::LeftStickY).unwrap_or(0.0);
        let rx = pad.get(GamepadAxis::RightStickX).unwrap_or(0.0);
        let ry = pad.get(GamepadAxis::RightStickY).unwrap_or(0.0);
        let l = Vec2::new(lx, ly);
        if l.length() > 0.22 {
            mv += Vec2::new(l.x, l.y); // stick up (+Y) == screen up (-Z) via ground()
        }
        let r = Vec2::new(rx, ry);
        if r.length() > 0.3 {
            stick_aim = r;
        }
        swing |= pad.just_pressed(GamepadButton::South);
        slam |= pad.just_pressed(GamepadButton::West);
        dodge |= pad.just_pressed(GamepadButton::East);
    }

    // Keyboard fallback — right-hand cluster by the arrow keys.
    let mut kb = Vec2::ZERO;
    if keys.any_pressed([KeyCode::ArrowUp]) {
        kb.y += 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowDown]) {
        kb.y -= 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowLeft]) {
        kb.x -= 1.0;
    }
    if keys.any_pressed([KeyCode::ArrowRight]) {
        kb.x += 1.0;
    }
    mv += kb;
    swing |= keys.just_pressed(KeyCode::Slash);
    slam |= keys.just_pressed(KeyCode::Comma);
    dodge |= keys.just_pressed(KeyCode::Period);
    let build = keys.just_pressed(KeyCode::Semicolon)
        || pads
            .iter()
            .next()
            .is_some_and(|p| p.just_pressed(GamepadButton::North));

    intent.move_dir = mv.normalize_or_zero();
    intent.attack = swing;
    intent.special = slam;
    intent.dodge = dodge;
    intent.build = build;
    intent.interact = keys.just_pressed(KeyCode::Quote)
        || pads
            .iter()
            .next()
            .is_some_and(|p| p.just_pressed(GamepadButton::RightTrigger));
    intent.fire = false;

    // Aim: right stick if given, else nearest enemy, else travel direction.
    let dir = if stick_aim != Vec2::ZERO {
        stick_aim.normalize()
    } else if let Some(e) = nearest_enemy(ppos, &enemies) {
        (e - ppos).normalize_or_zero()
    } else {
        intent.move_dir
    };
    if dir != Vec2::ZERO {
        aim.0 = ground(dir, 0.0);
    }
}

/// Capture-bot input — scripts both heroes so the proof video runs itself. The
/// two stay a loose pair (the drummer never strays far from the guitarist), and
/// a standing hero peels off to revive a downed team-mate.
#[allow(clippy::type_complexity)]
fn bot_input(
    time: Res<Time>,
    level: Res<CurrentLevel>,
    autoplay: Option<Res<AutoPlay>>,
    scrap: Option<Res<super::build::Scrap>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>)>,
    sites: Query<&Transform, (With<super::build::BuildSite>, Without<Player>)>,
    stages_q: Query<&Transform, (With<super::build::Stage>, Without<Player>)>,
    speakers_q: Query<&Transform, (With<super::build::Speaker>, Without<Player>)>,
    net: Option<Res<super::build::SpeakerNet>>,
    pitches: Query<&super::build::Pitch, Without<Player>>,
    cars: Query<&Transform, (With<super::drive::Car>, Without<Player>)>,
    obstacles: Query<(&Transform, &super::Obstacle), Without<Player>>,
    mut players: Query<
        (
            &Transform,
            &mut Aim,
            &mut Intent,
            &Hero,
            Option<&Downed>,
            Option<&super::drive::Driving>,
        ),
        With<Player>,
    >,
) {
    use super::build::{SPEAKER_COST, SPEAKERS_TO_LINK};
    if autoplay.is_none() {
        return;
    }
    let t = time.elapsed_secs();
    let bank = scrap.map(|s| s.0).unwrap_or(0);
    let site_positions: Vec<Vec2> = sites.iter().map(|tr| plane(tr.translation)).collect();
    let nearest_site = |from: Vec2| {
        site_positions.iter().copied().min_by(|a, b| {
            a.distance_squared(from)
                .total_cmp(&b.distance_squared(from))
        })
    };
    let stage_centres: Vec<Vec2> = stages_q.iter().map(|t| plane(t.translation)).collect();
    let speaker_positions: Vec<Vec2> = speakers_q.iter().map(|t| plane(t.translation)).collect();
    let speaker_count = speaker_positions.len();
    let linked = net.as_deref().map(|n| n.largest).unwrap_or(0);
    let blocks: Vec<(Vec2, f32)> = obstacles
        .iter()
        .map(|(o, b)| (plane(o.translation), b.radius))
        .collect();

    // Snapshot the party so each hero can steer relative to the other.
    let party: Vec<(Vec2, bool, Hero)> = players
        .iter()
        .map(|(tr, _, _, h, d, _)| (plane(tr.translation), d.is_some(), *h))
        .collect();
    let downed_mate = party.iter().find(|(_, d, _)| *d).map(|(p, _, _)| *p);
    let guitarist_pos = party
        .iter()
        .find(|(_, _, h)| *h == Hero::Guitarist)
        .map(|(p, _, _)| *p)
        .unwrap_or(Vec2::ZERO);

    // Loudspeakers go on the town's marked pitches now, so the bot walks a
    // route between them rather than a triangle it made up. It heads for the
    // free pitch nearest the speakers it has already planted (nearest the
    // party for the first one), which is what makes the zones link up.
    let free_pitches: Vec<(Vec2, bool)> = pitches
        .iter()
        .map(|p| (p.at, p.kind == super::build::StructureKind::Stage))
        .collect();
    let anchor_for_pitches = if speaker_positions.is_empty() {
        guitarist_pos
    } else {
        speaker_positions.iter().copied().sum::<Vec2>() / speaker_positions.len() as f32
    };
    let next_slot = free_pitches
        .iter()
        .filter(|(_, is_stage)| !is_stage)
        .map(|(at, _)| *at)
        .min_by(|a, b| {
            a.distance_squared(anchor_for_pitches)
                .total_cmp(&b.distance_squared(anchor_for_pitches))
        });
    let stage_pitch = free_pitches
        .iter()
        .find(|(_, is_stage)| *is_stage)
        .map(|(at, _)| *at);

    // Shared slow roam anchor, kept off the arena edges. Once stages are up the
    // party drifts toward the unclaimed side of town so the frontier reads.
    let ang = t * 0.22;
    // Hug the centre while planting the loudspeaker cluster so their zones link;
    // roam wider once the network is up.
    let anchor_r = if speaker_count < SPEAKERS_TO_LINK && stage_centres.is_empty() {
        0.07
    } else {
        0.16
    };
    let mut anchor = Vec2::new(ang.cos(), ang.sin()) * (level.arena * anchor_r);
    if let Some(last) = stage_centres.last() {
        let away = (anchor - *last).normalize_or_zero();
        let away = if away == Vec2::ZERO { Vec2::X } else { away };
        anchor = (*last + away * (super::build::STAGE_RADIUS + 24.0)).clamp(
            -(level.arena - Vec2::splat(10.0)),
            level.arena - Vec2::splat(10.0),
        );
    }

    let car_positions: Vec<Vec2> = cars.iter().map(|t| plane(t.translation)).collect();

    for (transform, mut aim, mut intent, hero, downed, driving) in &mut players {
        *intent = Intent::default();
        if downed.is_some() {
            continue; // a downed hero does nothing
        }
        let ppos = plane(transform.translation);

        // Driving, for the proof video and because it is the honest way to
        // cross a town this size: the guitarist takes a car when the pitch it
        // is heading for is a long walk and one is parked to hand, and gets
        // out again once it arrives.
        if *hero == Hero::Guitarist {
            let goal = next_slot.filter(|_| stage_centres.is_empty());
            let far = goal.map_or(0.0, |g| g.distance(ppos));
            if driving.is_some() {
                if let Some(g) = goal {
                    intent.move_dir = (g - ppos).normalize_or_zero();
                    intent.interact = far < 11.0;
                } else {
                    intent.interact = true;
                }
                continue;
            }
            if far > 22.0
                && let Some(car) = car_positions.iter().copied().min_by(|a, b| {
                    a.distance_squared(ppos)
                        .total_cmp(&b.distance_squared(ppos))
                })
            {
                let to_car = car.distance(ppos);
                if to_car < super::drive::REACH - 0.5 {
                    intent.interact = true;
                    continue;
                }
                // Worth the detour only if the car is nearer than the pitch.
                if to_car < 30.0 && to_car < far * 0.85 {
                    intent.fire = true;
                    intent.move_dir = (car - ppos).normalize_or_zero();
                    continue;
                }
            }
        }
        let nearest = nearest_enemy(ppos, &enemies);
        let close = nearest.map_or(f32::MAX, |n| n.distance(ppos));

        if let Some(n) = nearest {
            let d = (n - ppos).normalize_or_zero();
            if d != Vec2::ZERO {
                aim.0 = ground(d, 0.0);
            }
        }

        // Both heroes rush and hold whatever build site is currently up, so
        // channels finish fast for the proof video.
        let my_site = nearest_site(ppos).filter(|s| s.distance(ppos) < 95.0);

        // Revive priority: if a team-mate is down, close on them.
        let seek = if let Some(mate) = downed_mate.filter(|m| m.distance(ppos) > 3.0) {
            (mate - ppos).normalize_or_zero()
        } else if let Some(site) = my_site {
            intent.fire = *hero == Hero::Guitarist;
            intent.attack = *hero == Hero::Drummer;
            if site.distance(ppos) > 3.0 {
                (site - ppos).normalize_or_zero()
            } else {
                Vec2::ZERO
            }
        } else {
            match hero {
                // Guitarist: walk the loudspeaker triangle, planting one at each
                // slot; once the network is up, strafe the fight.
                Hero::Guitarist => {
                    intent.fire = true;
                    intent.special = close < 22.0 && (t * 0.7).fract() < 0.02;
                    intent.dodge = close < 2.6 && (t * 1.7).fract() < 0.05;
                    if let Some(slot) = next_slot.filter(|_| stage_centres.is_empty()) {
                        let d = slot - ppos;
                        if d.length() < 2.4 {
                            intent.build = bank >= SPEAKER_COST;
                            Vec2::ZERO
                        } else {
                            d.normalize_or_zero()
                        }
                    } else {
                        let tangent = Vec2::new(-ang.sin(), ang.cos());
                        if close < 15.0 {
                            let away = nearest
                                .map(|n| (ppos - n).normalize_or_zero())
                                .unwrap_or(Vec2::ZERO);
                            (tangent * 0.9 + away * 0.5).normalize_or_zero()
                        } else {
                            (anchor - ppos).normalize_or_zero()
                        }
                    }
                }
                // Drummer: once the loudspeakers are linked, go to their centroid
                // and raise the stage; until then, press the fight near the
                // guitarist.
                Hero::Drummer => {
                    intent.attack = (t * 3.0).fract() < 0.5;
                    intent.special = close < 18.0 && (t * 0.9).fract() < 0.02;
                    intent.dodge = close < 1.8 && (t * 2.1).fract() < 0.04;
                    if linked >= SPEAKERS_TO_LINK
                        && stage_centres.is_empty()
                        && let Some(pitch) = stage_pitch
                    {
                        let d = pitch - ppos;
                        if d.length() < 2.4 {
                            intent.build = bank >= 60;
                            Vec2::ZERO
                        } else {
                            d.normalize_or_zero()
                        }
                    } else {
                        let tether = ppos.distance(guitarist_pos);
                        if tether > 16.0 {
                            (guitarist_pos - ppos).normalize_or_zero()
                        } else {
                            match nearest {
                                Some(n) if close > 4.5 => (n - ppos).normalize_or_zero(),
                                Some(n) => (ppos - n).normalize_or_zero() * 0.35,
                                None => (anchor - ppos).normalize_or_zero(),
                            }
                        }
                    }
                }
            }
        };

        // Bend the heading around any block dead ahead.
        let mut avoid = Vec2::ZERO;
        for (c, r) in &blocks {
            let to_c = *c - ppos;
            let dist = to_c.length();
            let reach = r + PLAYER_RADIUS + 6.0;
            if dist < 1e-3 || dist > reach {
                continue;
            }
            let ahead = to_c / dist;
            if ahead.dot(seek) <= 0.0 {
                continue;
            }
            let tan = Vec2::new(-ahead.y, ahead.x);
            let side = if tan.dot(seek) >= 0.0 { 1.0 } else { -1.0 };
            let s = (reach - dist) / reach;
            avoid += tan * side * s * 1.6 - ahead * s * 0.6;
        }
        intent.move_dir = (seek + avoid).normalize_or_zero();
    }
}

fn movement(
    time: Res<Time>,
    level: Res<CurrentLevel>,
    obstacles: Query<(&Transform, &super::Obstacle), Without<Player>>,
    mut players: Query<
        (
            &mut Transform,
            &mut Dodge,
            &Aim,
            &mut Moving,
            &Intent,
            &Hero,
            Option<&Downed>,
            Option<&super::drive::Driving>,
            Option<&super::fly::Flying>,
        ),
        With<Player>,
    >,
) {
    let dt = time.delta_secs();
    let blocks: Vec<(Vec2, f32)> = obstacles
        .iter()
        .map(|(t, o)| (plane(t.translation), o.radius))
        .collect();

    for (mut transform, mut dodge, aim, mut moving, intent, _hero, downed, driving, flying) in
        &mut players
    {
        dodge.cooldown.tick(time.delta());
        // A hero at the wheel — of a car or a plane — is moved by that vehicle,
        // not on foot.
        if driving.is_some() || flying.is_some() {
            moving.0 = false;
            continue;
        }
        if downed.is_some() {
            moving.0 = false;
            dodge.active = None;
            continue;
        }
        let ppos = plane(transform.translation);
        let input = intent.move_dir.clamp_length_max(1.0);

        if intent.dodge && !dodge.is_rolling() && dodge.cooldown.is_finished() {
            let dir = if input != Vec2::ZERO {
                ground(input.normalize(), 0.0)
            } else {
                aim.0
            };
            dodge.active = Some((Timer::from_seconds(ROLL_TIME, TimerMode::Once), dir));
            dodge.cooldown = Timer::from_seconds(ROLL_COOLDOWN, TimerMode::Once);
        }

        let mut roll_frac = None;
        let velocity = if let Some((timer, dir)) = dodge.active.as_mut() {
            timer.tick(time.delta());
            let dir = *dir;
            roll_frac = Some(timer.fraction());
            if timer.is_finished() {
                dodge.active = None;
            }
            dir * ROLL_SPEED
        } else {
            ground(input, 0.0) * MOVE_SPEED
        };

        let mut next = ppos + plane(velocity) * dt;
        next = super::resolve_obstacles(next, PLAYER_RADIUS, &blocks);
        let limit = level.arena - Vec2::splat(PLAYER_RADIUS);
        next = next.clamp(-limit, limit);
        transform.translation = ground(next, BODY_Y);

        moving.0 = dodge.active.is_none() && input != Vec2::ZERO;

        // Face the aim while acting, otherwise face travel; barrel-roll while
        // dodging.
        let acting = intent.fire || intent.attack;
        let facing = if acting && dodge.active.is_none() {
            aim.0
        } else if velocity.length_squared() > 0.01 {
            velocity.normalize()
        } else {
            aim.0
        };
        // The Quaternius mesh's front is its local +Z — same convention the
        // guitar-gun's barrel uses in `aim_guns` (no half-turn offset).
        let yaw = Quat::from_rotation_y(f32::atan2(facing.x, facing.z));
        if let Some(frac) = roll_frac {
            let axis = Vec3::new(facing.z, 0.0, -facing.x);
            transform.rotation = Quat::from_axis_angle(axis, frac * std::f32::consts::TAU) * yaw;
        } else {
            transform.rotation = yaw;
        }
    }
}
