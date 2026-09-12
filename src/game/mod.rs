use bevy::prelude::*;

mod audio;
mod build;
mod combat;
mod credits;
pub mod drive;
mod drummer;
pub mod enemy;
pub mod fly;
pub mod gloom;
pub mod grade;
pub mod hud;
pub mod level;
mod minimap;
pub mod net;
mod pickup;
pub mod player;
pub mod props;
pub mod roster;
pub mod scale;
pub mod select;
pub mod skins;
pub mod ui;
mod vfx;
pub mod weapon;

/// **Which environment this run is in.** Everything that used to read a
/// hard-coded `ARENA` — the camera clamp, the swarm's spawn ring, the wander
/// bounds — now reads `level.arena` off this resource instead, so a second
/// environment can simply be a different size.
pub use level::CurrentLevel;

/// The camera sits at this offset from the ground point it looks at, and
/// `follow_camera` keeps that point framed on the players (clamped to the
/// arena). With two co-op heroes the camera also eases *outward* along this
/// direction as they spread apart — see `follow_camera`.
pub const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 20.0, 12.0);

/// Extra pull-back per unit of distance between the two co-op players, and the
/// cap on that pull-back, so a split party stays on screen without the camera
/// lurching to the stratosphere.
const CAMERA_SPREAD_ZOOM: f32 = 0.34;
const CAMERA_SPREAD_ZOOM_MAX: f32 = 17.0;

/// Experimental camera rigs, cycled at runtime with **V** (windowed). The
/// capture bot scripts its own cycle so the proof video shows all three.
/// `Follow` is the shipping co-op framing; the other two are toys for now and
/// track the guitarist only.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    /// Angled top-down, framed on the party midpoint (the real game camera).
    #[default]
    Follow,
    /// Over-the-shoulder, near street level, looking down the guitarist's aim.
    Shoulder,
    /// First person from the guitarist's head down the aim line.
    FirstPerson,
}

impl CameraMode {
    fn next(self) -> Self {
        match self {
            CameraMode::Follow => CameraMode::Shoulder,
            CameraMode::Shoulder => CameraMode::FirstPerson,
            CameraMode::FirstPerson => CameraMode::Follow,
        }
    }
}

#[derive(States, Debug, Clone, Copy, Default, Eq, PartialEq, Hash)]
pub enum GameState {
    /// The character-select lineup (`select.rs`). The game opens here — you
    /// pick who you are before a town is built.
    #[default]
    Select,
    Playing,
    GameOver,
    /// The band raised a full stage on linked, secured ground — the level is
    /// won (see `build.rs`). Frozen behind a win screen until restart.
    Victory,
}

/// Marks entities that belong to a single run, torn down on restart.
#[derive(Component)]
pub struct RunEntity;

/// How many heroes are in play. `shooty` = `Solo` (guitarist only); `shooty
/// coop` / `host` / `join` = `Duo`. Defaults to `Duo` so the capture binary is
/// unaffected.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Party {
    Solo,
    #[default]
    Duo,
}

/// Marker on both co-op heroes. `Hero` says which one.
#[derive(Component)]
pub struct Player;

/// Which co-op hero this is. P1 (keyboard + mouse) plays the guitarist and its
/// ranged pick weapon; P2 (gamepad, or the keyboard fallback set) plays the
/// drummer with its beat-timed shockwaves and drumstick melee.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hero {
    Guitarist,
    Drummer,
}

impl Hero {
    pub fn name(self) -> &'static str {
        match self {
            Hero::Guitarist => "Guitarist",
            Hero::Drummer => "Drummer",
        }
    }
}

/// On a player whose health hit zero but the run isn't over — they're on the
/// ground playing dead until a team-mate revives them (`combat::revive_downed`)
/// or the last hero falls too. Systems that move / arm / aim a player skip
/// anyone carrying this.
#[derive(Component)]
pub struct Downed;

/// On an enemy that has wandered inside a completed stage's radius (see
/// `build`): the music has got to it. It deals no contact damage and drifts
/// back out of the zone until its `Lifetime` runs out.
#[derive(Component)]
pub struct Pacified;

/// Nearest living (non-[`Downed`]) player position to `from`, on the ground
/// plane — the aim point for enemies and the swarm's spawn focus. Falls back to
/// the nearest downed player, then `None`.
pub fn nearest_player(from: Vec2, players: impl IntoIterator<Item = (Vec2, bool)>) -> Option<Vec2> {
    let mut best_live: Option<(f32, Vec2)> = None;
    let mut best_any: Option<(f32, Vec2)> = None;
    for (p, downed) in players {
        let d = p.distance_squared(from);
        if best_any.is_none_or(|(bd, _)| d < bd) {
            best_any = Some((d, p));
        }
        if !downed && best_live.is_none_or(|(bd, _)| d < bd) {
            best_live = Some((d, p));
        }
    }
    best_live.or(best_any).map(|(_, p)| p)
}

/// A static piece of the environment (a city building). Actors collide with and
/// steer around it on the ground plane; treated as a circle of `radius` centred
/// on the entity's translation.
#[derive(Component)]
pub struct Obstacle {
    pub radius: f32,
}

/// Push a ground-plane position `p` (an actor of radius `r`) out of any
/// [`Obstacle`] it overlaps. Shared by the player and the enemies.
pub fn resolve_obstacles(p: Vec2, r: f32, obstacles: &[(Vec2, f32)]) -> Vec2 {
    let mut out = p;
    for (centre, radius) in obstacles {
        let away = out - *centre;
        let min = radius + r;
        let d = away.length();
        if d < min {
            out = if d > 1e-4 {
                *centre + away / d * min
            } else {
                *centre + Vec2::new(min, 0.0)
            };
        }
    }
    out
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub touch_damage: f32,
    pub speed: f32,
}

/// A wave is not monsters — it is other people's dark moods, spilling out of the
/// five miserable townsfolk in `gloom.rs`. Every tier is the boxy `base.glb`
/// rig wearing the venting citizen's own genre outfit, drained near-black: you
/// are fighting a shadow of the disco crowd, not the disco crowd.
#[derive(Clone, Copy, PartialEq)]
pub enum EnemyKind {
    /// A walking dark clone of the citizen. The bulk of a wave.
    Mood,
    /// Just the head, detached and drifting. Fast, fragile, swarms.
    Head,
    /// Hangs back at a standoff and throws a dark bad-vibe bolt at you.
    Heckler,
    /// An oversized dark clone — soaks fire, walks you down, hits hard.
    Sink,
}

impl EnemyKind {
    /// Round-trip helper for [`net`] replication (`self as u8` is the encode).
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => EnemyKind::Head,
            2 => EnemyKind::Heckler,
            3 => EnemyKind::Sink,
            _ => EnemyKind::Mood,
        }
    }
}

#[derive(Component)]
pub struct Bullet {
    pub velocity: Vec3,
    pub damage: f32,
}

/// A bad-vibe bolt thrown *at* the party by a [`EnemyKind::Heckler`]. Kept a
/// separate component from [`Bullet`] so the two never cross-hit — hero shots
/// only damage enemies, heckler bolts only damage heroes.
#[derive(Component)]
pub struct EnemyBullet {
    pub velocity: Vec3,
    pub damage: f32,
}

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// Ground-plane radius for the circle-overlap collision checks.
#[derive(Component)]
pub struct Hitbox(pub f32);

#[derive(Component)]
pub struct Lifetime(pub Timer);

#[derive(Resource, Default)]
pub struct Score {
    pub kills: u32,
}

/// Present only in the capture binary: scripted player + auto-fire.
#[derive(Resource)]
pub struct AutoPlay;

/// Ground-plane position `p` at height `y`. Screen "up" is `-Z`.
#[inline]
pub fn ground(p: Vec2, y: f32) -> Vec3 {
    Vec3::new(p.x, y, -p.y)
}

/// The ground-plane `(x, z→y)` coordinates of a world transform.
#[inline]
pub fn plane(t: Vec3) -> Vec2 {
    Vec2::new(t.x, -t.z)
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Score>()
            .init_resource::<net::NetRole>()
            .init_resource::<Party>()
            .init_resource::<CameraMode>()
            .init_resource::<roster::Roster>()
            .insert_resource(CurrentLevel(level::LevelId::from_env()))
            .add_systems(OnEnter(GameState::Playing), reset_run)
            .add_systems(
                Update,
                (
                    tick_lifetimes.run_if(in_state(GameState::Playing)),
                    cycle_camera_mode,
                    drive_camera,
                )
                    .chain(),
            )
            .add_plugins((
                player::PlayerPlugin,
                vfx::VfxPlugin,
                drummer::DrummerPlugin,
                drive::DrivePlugin,
                weapon::WeaponPlugin,
                enemy::EnemyPlugin,
                combat::CombatPlugin,
                pickup::PickupPlugin,
                build::BuildPlugin,
                gloom::GloomPlugin,
                audio::AudioCuePlugin,
                grade::GradePlugin,
                select::SelectPlugin,
                minimap::MinimapPlugin,
                hud::HudPlugin,
            ))
            // A second tuple: `add_plugins` takes at most 15 at a time.
            .add_plugins((
                level::sky::SkyPlugin,
                fly::FlyPlugin,
                credits::CreditsPlugin,
            ));
    }
}

fn reset_run(
    mut commands: Commands,
    assets: Res<AssetServer>,
    entities: Query<Entity, With<RunEntity>>,
    mut score: ResMut<Score>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    level: Res<CurrentLevel>,
) {
    for e in &entities {
        commands.entity(e).despawn();
    }
    score.kills = 0;
    level::build(
        &mut commands,
        &assets,
        &mut meshes,
        &mut materials,
        level.0.level(),
    );
}

/// **V** cycles the experimental camera rigs (windowed only — the capture bot
/// drives its own cycle).
fn cycle_camera_mode(keys: Res<ButtonInput<KeyCode>>, mut mode: ResMut<CameraMode>) {
    if keys.just_pressed(KeyCode::KeyV) {
        *mode = mode.next();
        info!("camera mode -> {:?}", *mode);
    }
}

/// Position the camera for the active [`CameraMode`]. `Follow` is the shipping
/// co-op framing; `Shoulder` / `FirstPerson` ride the guitarist down its aim.
fn drive_camera(
    time: Res<Time>,
    mode: Res<CameraMode>,
    level: Res<CurrentLevel>,
    players: Query<
        (&Transform, Option<&player::Aim>, Option<&Hero>),
        (With<Player>, Without<Camera3d>),
    >,
    airborne: Query<&fly::Plane>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut cam) = camera.single_mut() else {
        return;
    };
    let dt = time.delta_secs();

    // The guitarist (or, failing that, the first player) anchors the two toy rigs.
    let anchor = players
        .iter()
        .find(|(_, _, h)| matches!(h, Some(Hero::Guitarist)))
        .or_else(|| players.iter().next());

    match *mode {
        CameraMode::Follow => {
            let pts: Vec<Vec2> = players.iter().map(|(t, ..)| plane(t.translation)).collect();
            if pts.is_empty() {
                return;
            }
            let mid = pts.iter().copied().sum::<Vec2>() / pts.len() as f32;
            let spread = pts.iter().map(|p| p.distance(mid)).fold(0.0_f32, f32::max);
            let zoom = (spread * CAMERA_SPREAD_ZOOM).min(CAMERA_SPREAD_ZOOM_MAX);
            // Follow a pilot upstairs: without this the camera stays at its
            // ground offset and the plane flies straight out of frame.
            let ceiling = airborne.iter().map(|p| p.altitude).fold(0.0_f32, f32::max);
            let offset = CAMERA_OFFSET
                + CAMERA_OFFSET.normalize() * zoom
                + Vec3::Y * fly::camera_lift(ceiling);
            let margin = Vec2::new(12.0, 8.0) + Vec2::splat(zoom * 0.5);
            let slack = (level.arena - margin).max(Vec2::ZERO);
            let focus = mid.clamp(-slack, slack);
            let goal = ground(focus, ceiling) + offset;
            let k = (1.0 - (-7.0 * dt).exp()).clamp(0.0, 1.0);
            cam.translation = cam.translation.lerp(goal, k);
            cam.look_at(ground(focus, ceiling), Vec3::Y);
        }
        CameraMode::Shoulder | CameraMode::FirstPerson => {
            let Some((pt, aim, _)) = anchor else { return };
            let fwd = aim
                .map(|a| a.0)
                .filter(|v| v.length_squared() > 1e-4)
                .unwrap_or(Vec3::NEG_Z)
                .normalize();
            let (eye, look, k) = if *mode == CameraMode::Shoulder {
                (
                    pt.translation + Vec3::Y * 3.4 - fwd * 8.5 + fwd.cross(Vec3::Y) * 1.4,
                    pt.translation + Vec3::Y * 1.3 + fwd * 14.0,
                    1.0 - (-12.0 * dt).exp(),
                )
            } else {
                (
                    pt.translation + Vec3::Y * 2.5 + fwd * 0.35,
                    pt.translation + Vec3::Y * 2.5 + fwd * 20.0,
                    1.0 - (-22.0 * dt).exp(),
                )
            };
            cam.translation = cam.translation.lerp(eye, k.clamp(0.0, 1.0));
            cam.look_at(look, Vec3::Y);
        }
    }
}

fn tick_lifetimes(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Lifetime)>) {
    for (e, mut life) in &mut q {
        if life.0.tick(time.delta()).is_finished() {
            // `try_despawn`: a run-teardown may have already taken this entity
            // in the same frame (RunEntity + Lifetime both despawn it).
            commands.entity(e).try_despawn();
        }
    }
}
