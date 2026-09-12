//! **Asset lab** — a viewer for characters, skins, animations and props.
//!
//! ```text
//! cargo run --bin lab                    # interactive window
//! cargo run --bin lab -- shot <dir>      # offscreen contact sheet of the cast
//! cargo run --bin lab -- shot <dir> cmp  # just the boxy-vs-Quaternius compare
//! ```
//!
//! The point is to judge art without playing the game. It shows one subject at
//! a time under the game's own camera rig, lighting and gloom→daylight grade,
//! and applies the same per-material tints and face planes the game applies —
//! so what you sign off here is what the game draws.
//!
//! Nothing here is game code: no `GamePlugin`, no town, no waves. It borrows
//! the art functions (`player::hero_tint`, `enemy::gloom_tint`,
//! `enemy::face_placements`, `grade::apply_grade`, ...) rather than copying
//! their numbers, so the lab can't drift from the game.

use std::f32::consts::{FRAC_PI_2, PI};
use std::time::Duration;

use bevy::{
    animation::RepeatAnimation,
    app::{AppExit, ScheduleRunnerPlugin},
    camera::RenderTarget,
    gltf::{Gltf, GltfMaterialName},
    image::{ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler,
        ImageSamplerDescriptor},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
    time::TimeUpdateStrategy,
    window::ExitCondition,
    winit::WinitPlugin,
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};

use shooty::game::{
    EnemyKind,
    enemy::{FACE_STYLES, face_placements, stats, vibe_materials},
    grade::apply_grade,
    player::{face_in_head, face_material},
    props::{self, Carry, Prop, Rig},
    weapon::{ShotLook, shot_material, shot_mesh},
    skins::{self, Cast},
};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

// ---------------------------------------------------------------------------
// Subjects
// ---------------------------------------------------------------------------

#[derive(Clone)]
enum Subject {
    /// Any glTF under `assets/models`. `character` marks the Quaternius cast —
    /// the ones the skin roles and face planes apply to.
    Glb {
        path: String,
        label: String,
        character: bool,
    },
    /// A **boxy** character — the blocky base mesh in a painted atlas.
    /// Index into [`skins::BOXY`].
    Boxy(usize),
    /// A detached dark-mood head, in one of the `FACE_STYLES` expressions —
    /// built the way `enemy.rs` builds a `EnemyKind::Head`.
    Vibe(usize),
    /// One piece of music gear on its own, off any body. Index into
    /// [`Prop::ALL`].
    Gear(usize),
    /// A candidate projectile shape, off any weapon. Index into
    /// [`ShotLook::ALL`].
    Shot(usize),
}

impl Subject {
    fn label(&self) -> String {
        match self {
            Subject::Glb { label, .. } => label.clone(),
            Subject::Boxy(i) => format!("boxy / {}", skins::BOXY[*i].id),
            Subject::Vibe(k) => format!("vibe / {}", FACE_STYLES[*k].0),
            Subject::Gear(i) => format!("gear / {}", Prop::ALL[*i].name()),
            Subject::Shot(i) => format!("shot / {}", ShotLook::ALL[*i].name()),
        }
    }
    /// Which rig a character subject is built on — decides the mount bones and
    /// the scale every carry pose is expressed in.
    fn rig(&self) -> Rig {
        match self {
            Subject::Boxy(_) => Rig::Boxy,
            _ => Rig::Skeletal,
        }
    }
    fn is_character(&self) -> bool {
        matches!(
            self,
            Subject::Glb {
                character: true,
                ..
            }
        )
    }
}

/// Which set of art rules to dress a character in — "off the shelf", or one of
/// the palettes in [`skins::SKINS`]. Because the whole cast shares one skeleton
/// and one set of material names, any skin can be tried on any body; the skin's
/// own `model` is only where it was designed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    /// Straight off the shelf — no recolour, no face plane.
    Stock,
    /// Index into [`skins::SKINS`].
    Skin(usize),
}

impl Role {
    /// Stock, then every skin in table order.
    fn count() -> usize {
        skins::SKINS.len() + 1
    }
    fn at(i: usize) -> Role {
        match i {
            0 => Role::Stock,
            n => Role::Skin(n - 1),
        }
    }
    fn index(self) -> usize {
        match self {
            Role::Stock => 0,
            Role::Skin(i) => i + 1,
        }
    }
    fn skin(self) -> Option<&'static skins::Skin> {
        match self {
            Role::Stock => None,
            Role::Skin(i) => skins::SKINS.get(i),
        }
    }

    fn name(self) -> String {
        match self.skin() {
            None => "stock".into(),
            Some(sk) => {
                let cast = match sk.cast {
                    Cast::Band => "band",
                    Cast::Crowd => "crowd",
                    Cast::Gloom => "gloom",
                };
                format!("{} ({cast}) — {}", sk.name, sk.genre)
            }
        }
    }
    /// The scale the game spawns this role at.
    fn scale(self) -> f32 {
        self.skin().map_or(1.0, |sk| sk.model_scale())
    }
    fn tint(self, mat: &str) -> Option<Color> {
        self.skin().and_then(|sk| sk.tint(mat))
    }
    /// The face-plane texture this role wears on its `Head` bone.
    fn face(self) -> Option<&'static str> {
        self.skin().and_then(|sk| sk.face)
    }
}

/// The dark-mood head expressions, in `FACE_STYLES` order.
const VIBES: [usize; 3] = [0, 1, 2];

/// Walk `assets/models` for glTF files, Quaternius cast first.
fn discover_subjects() -> Vec<Subject> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "glb" || x == "gltf") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new("assets/models");
    let mut found = Vec::new();
    walk(root, &mut found);
    found.sort();

    let mut cast = Vec::new();
    let mut props = Vec::new();
    for p in found {
        let rel = p
            .strip_prefix("assets")
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        let stem = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let character = rel.contains("quaternius/");
        let group = p
            .parent()
            .and_then(|d| d.file_name())
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        let s = Subject::Glb {
            path: rel,
            label: if character {
                format!("cast / {stem}")
            } else {
                format!("{group} / {stem}")
            },
            character,
        };
        if character {
            cast.push(s)
        } else {
            props.push(s)
        }
    }

    // Put the three the game actually ships first, then the rest of the cast.
    let shipped = ["punk", "casual", "suit"];
    cast.sort_by_key(|s| {
        let l = s.label();
        shipped
            .iter()
            .position(|n| l.ends_with(n))
            .unwrap_or(shipped.len())
    });

    // The boxy cast leads: it is the style the art direction is aiming at, and
    // the whole point of the lab right now is holding it against the Quaternius
    // line that drifted off it.
    let mut out: Vec<Subject> = (0..skins::BOXY.len()).map(Subject::Boxy).collect();
    out.extend(cast);
    out.extend(VIBES.iter().map(|k| Subject::Vibe(*k)));
    out.extend((0..Prop::ALL.len()).map(Subject::Gear));
    out.extend((0..ShotLook::ALL.len()).map(Subject::Shot));
    out.extend(props);
    out
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nudge {
    Off,
    /// Live-tune the face quad's offset/scale on the `Head` bone.
    Face,
    /// Live-tune the mounted prop's fit on its bone.
    Prop,
}

/// Ground swatches — the backdrops a character has to read against in game.
const GROUNDS: [(&str, Color); 5] = [
    ("grass", Color::srgb(0.40, 0.49, 0.29)),
    ("road", Color::srgb(0.44, 0.45, 0.43)),
    ("dark", Color::srgb(0.06, 0.06, 0.08)),
    ("mid grey", Color::srgb(0.50, 0.50, 0.50)),
    ("paper", Color::srgb(0.86, 0.86, 0.84)),
];

#[derive(Resource)]
struct Lab {
    subjects: Vec<Subject>,
    idx: usize,
    role: Role,
    /// Extra multiplier on top of the role's shipped scale, for "is 1.15 right?".
    scale_mul: f32,
    speed: f32,
    paused: bool,
    looping: bool,
    clip: usize,
    /// Clips found on the current subject: (name, graph node, duration).
    clips: Vec<(String, AnimationNodeIndex, f32)>,
    yaw: f32,
    pitch: f32,
    dist: f32,
    target_y: f32,
    /// How much of the frame height the framed slice should fill when refitting.
    fill: f32,
    /// The slice of the subject's height to frame — 1.0 is all of it, small
    /// values close in on the head.
    frame: f32,
    /// Where to look, as a fraction of the subject's height.
    look: f32,
    /// Set by the `game` preset: a fixed distance instead of a fit.
    absolute: Option<f32>,
    /// Recompute `dist`/`target_y` from the measured bounds next frame.
    refit: bool,
    preset: &'static str,
    ground: usize,
    grade: f32,
    ruler: bool,
    /// The key-help block — off for contact sheets.
    help: bool,
    nudge: Nudge,
    face_off: Vec3,
    face_scale: Vec2,
    gun_off: Vec3,
    gun_scale: f32,
    /// Index into [`Prop::ALL`], or `None` for a character carrying nothing.
    prop: Option<usize>,
    /// How the mounted prop is worn.
    carry: Carry,
    /// Set to respawn the subject at the top of the next frame.
    dirty: bool,
    note: String,
}

impl Default for Lab {
    fn default() -> Self {
        let subjects = discover_subjects();
        let mut lab = Self {
            subjects,
            idx: 0,
            role: Role::at(1),
            scale_mul: 1.0,
            speed: 1.0,
            paused: false,
            looping: true,
            clip: 0,
            clips: Vec::new(),
            yaw: 0.0,
            pitch: 0.0,
            dist: 4.0,
            target_y: 1.0,
            fill: 0.82,
            frame: 1.0,
            look: 0.55,
            absolute: None,
            refit: true,
            preset: "3/4",
            ground: 0,
            grade: 1.0,
            ruler: true,
            help: true,
            nudge: Nudge::Off,
            prop: Some(0),
            carry: Carry::Slung,
            // The values `player.rs` / `enemy.rs` currently ship.
            face_off: face_in_head().translation,
            face_scale: face_in_head().scale.truncate(),
            gun_off: props::carry_transform(Carry::Slung, Prop::FlyingV, Rig::Skeletal, false)
                .translation,
            gun_scale: props::carry_transform(Carry::Slung, Prop::FlyingV, Rig::Skeletal, false)
                .scale
                .x,
            dirty: true,
            note: String::new(),
        };
        lab.preset("3/4");
        lab
    }
}

impl Lab {
    fn subject(&self) -> &Subject {
        &self.subjects[self.idx]
    }

    /// The subject index of the body a role's skin was designed against, so
    /// paging through skins lands on the intended character rather than
    /// dressing whatever happens to be on the turntable.
    fn home_body(&self, role: Role) -> Option<usize> {
        let want = role.skin()?.model;
        self.subjects.iter().position(|s| match s {
            Subject::Glb { path, .. } => path == want,
            _ => false,
        })
    }

    /// The exact angle and distance the in-game follow camera views a hero from
    /// (`CAMERA_OFFSET` above the character's ground origin). Deliberately not
    /// fitted to the subject — being tiny here *is* the finding.
    fn preset_game(&mut self) {
        let o = shooty::CAMERA_OFFSET;
        self.dist = o.length();
        self.pitch = (o.y / self.dist).asin();
        self.yaw = o.x.atan2(o.z);
        self.target_y = 0.0;
        self.absolute = Some(self.dist);
        self.refit = false;
        self.preset = "game";
    }

    /// Every other view frames the subject from its measured bounds, so a
    /// hydrant and a tower block both land in shot.
    fn preset(&mut self, name: &'static str) {
        // (yaw, pitch, height slice to frame, how much of screen it fills,
        //  look-at height fraction)
        let (yaw, pitch, frame, fill, look) = match name {
            "3/4" => (0.62, 0.22, 1.0, 0.80, 0.55),
            "front" => (0.0, 0.10, 1.0, 0.86, 0.52),
            "side" => (FRAC_PI_2, 0.10, 1.0, 0.86, 0.52),
            "back" => (PI, 0.10, 1.0, 0.86, 0.52),
            "top" => (0.0, 1.40, 1.0, 0.70, 0.30),
            // Close on the head — for the face planes and the hair blocks. A
            // Quaternius head is roughly the top eighth of the body.
            "face" => (0.0, 0.10, 0.30, 0.80, 0.84),
            _ => {
                self.preset_game();
                return;
            }
        };
        self.yaw = yaw;
        self.pitch = pitch;
        self.frame = frame;
        self.fill = fill;
        self.look = look;
        self.absolute = None;
        self.refit = true;
        self.preset = name;
    }

    fn scale(&self) -> f32 {
        self.role.scale() * self.scale_mul
    }

    /// A paste-ready line for whichever nudge mode is live.
    fn nudge_literal(&self) -> String {
        match self.nudge {
            Nudge::Off => String::new(),
            Nudge::Face => format!(
                "Transform::from_xyz({:.3}, {:.3}, {:.3}).with_scale(Vec3::new({:.3}, {:.3}, 1.0))",
                self.face_off.x,
                self.face_off.y,
                self.face_off.z,
                self.face_scale.x,
                self.face_scale.y
            ),
            Nudge::Prop => format!(
                "Transform::from_xyz(s * {:.3}, {:.3}, {:.3}).with_scale(Vec3::splat({:.3}))",
                self.gun_off.x.abs(),
                self.gun_off.y,
                self.gun_off.z,
                self.gun_scale
            ),
        }
    }
}

/// Everything that gets torn down when the subject changes.
#[derive(Component)]
struct SubjectRoot;

/// On the model root: what `on_ready` needs once the glTF scene instantiates.
#[derive(Component)]
struct LabModel {
    gltf: Handle<Gltf>,
    role: Role,
    /// Set for a [`Subject::Boxy`]: the painted atlas to put on every body
    /// part, instead of the per-material tints a Quaternius skin uses.
    atlas: Option<Handle<Image>>,
}

/// On the buried `AnimationPlayer`: the graph the lab drives.
#[derive(Component)]
struct LabAnim {
    playing: AnimationNodeIndex,
}

/// The face quad and the guitar-gun, so nudge mode can move them live.
#[derive(Component)]
struct FaceQuad;
#[derive(Component)]
struct LabProp;

#[derive(Resource)]
struct GroundMat(Handle<StandardMaterial>);

/// World-space extents of the current subject, measured once its meshes have a
/// propagated `GlobalTransform`. Everything but the `game` view frames off this,
/// so a 0.4 m hydrant and a 20 m tower are both filmed sensibly.
#[derive(Resource, Default)]
struct Bounds {
    min: Vec3,
    max: Vec3,
    measured: bool,
}

impl Bounds {
    fn height(&self) -> f32 {
        (self.max.y - self.min.y).max(0.05)
    }
}

#[derive(Component)]
struct LabCam;

#[derive(Component)]
struct LabHud;

#[derive(Component)]
struct LabHelp;

// ---------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let shot = args.get(1).map(String::as_str) == Some("shot");
    // The full sheet is 100-odd frames of every body in every skin. `cmp` is
    // the narrow one: the ten boxy characters against the Quaternius skins
    // wearing the same genre, which is the only question open right now.
    let compare = args.get(3).map(String::as_str) == Some("cmp");
    // `gear`: every prop in every carry pose, on one body of each rig.
    let gear = args.get(3).map(String::as_str) == Some("gear");

    let mut app = App::new();
    shooty::insert_scene_resources(&mut app);
    app.init_resource::<Lab>().init_resource::<Bounds>();

    if shot {
        let dir = args
            .get(2)
            .cloned()
            .unwrap_or_else(|| "screenshots/lab".into());
        std::fs::create_dir_all(&dir).expect("create out dir");
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
            1.0 / 30.0,
        )))
        .insert_resource(Shoot {
            dir,
            compare,
            gear,
            tick: 0,
            item: 0,
            plan: Vec::new(),
        })
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .disable::<WinitPlugin>(),
        )
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 120.0,
        )))
        .add_systems(Startup, (setup, setup_shot_target).chain())
        .add_systems(Update, run_shot);
    } else {
        app.add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Shooty — asset lab".into(),
                        resolution: (WIDTH, HEIGHT).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_systems(Startup, (setup, spawn_window_camera).chain())
        .add_systems(Update, (keys, orbit, shoot_window_screenshot));
    }

    app.add_systems(
        Update,
        (
            respawn_subject,
            measure_subject,
            apply_playback,
            apply_nudge,
            apply_ground,
            drive_camera,
            draw_reference,
            update_hud,
        )
            .chain(),
    )
    .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    shooty::spawn_key_light(&mut commands);

    let ground = materials.add(StandardMaterial {
        base_color: GROUNDS[0].1,
        perceptual_roughness: 0.95,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(80.0, 80.0))),
        MeshMaterial3d(ground.clone()),
    ));
    commands.insert_resource(GroundMat(ground));

    // Two read-outs: what you are looking at (top-left, on a plate so it stays
    // legible over a pale ground) and the key map (bottom-left, dimmer).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(12),
                left: px(12),
                padding: UiRect::all(px(8)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.06, 0.62)),
        ))
        .with_children(|c| {
            c.spawn((
                LabHud,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.97, 1.0)),
            ));
        });

    commands.spawn((
        LabHelp,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(12.0),
            ..default()
        },
        TextColor(Color::srgba(0.86, 0.89, 0.96, 0.72)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(10),
            left: px(12),
            ..default()
        },
    ));
}

fn spawn_window_camera(mut commands: Commands) {
    commands.spawn((LabCam, shooty::camera_bundle()));
}

// ---------------------------------------------------------------------------
// Subject spawning
// ---------------------------------------------------------------------------

/// Load a painted atlas with the sampler the Kenney meshes need.
///
/// Their UVs run outside [0,1] (V ~ 1.0-2.0) and only land on the right atlas
/// island under **Repeat** addressing — the default clamp sampler squashes
/// every body part onto the atlas edge, which reads as a character dipped in
/// one colour. Nearest keeps the painted texels crisp.
fn boxy_atlas(assets: &AssetServer, path: &str) -> Handle<Image> {
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

fn respawn_subject(
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    mut bounds: ResMut<Bounds>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    old: Query<Entity, With<SubjectRoot>>,
) {
    if !lab.dirty {
        return;
    }
    lab.dirty = false;
    bounds.measured = false;
    lab.refit = true;
    lab.clips.clear();
    lab.clip = 0;
    for e in &old {
        commands.entity(e).despawn();
    }

    let role = lab.role;
    let scale = lab.scale();
    match lab.subject().clone() {
        Subject::Glb {
            path, character, ..
        } => {
            let role = if character { role } else { Role::Stock };
            let scale = if character { scale } else { 1.0 };
            commands
                .spawn((
                    SubjectRoot,
                    WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(path.clone()))),
                    LabModel {
                        gltf: assets.load(path),
                        role,
                        atlas: None,
                    },
                    Transform::from_scale(Vec3::splat(scale)),
                    Visibility::default(),
                ))
                .observe(on_ready);
        }
        Subject::Boxy(i) => {
            let boxy = &skins::BOXY[i];
            let scale = boxy.model_scale() * lab.scale_mul;
            commands
                .spawn((
                    SubjectRoot,
                    WorldAssetRoot(
                        assets.load(GltfAssetLabel::Scene(0).from_asset(skins::BOXY_BASE)),
                    ),
                    LabModel {
                        gltf: assets.load(skins::BOXY_BASE),
                        role: Role::Stock,
                        atlas: Some(boxy_atlas(&assets, boxy.atlas)),
                    },
                    // `base.glb` has its origin at the hips, so stand it up.
                    Transform::from_xyz(0.0, skins::BOXY_LIFT * scale, 0.0)
                        .with_scale(Vec3::splat(scale)),
                    Visibility::default(),
                ))
                .observe(on_ready);
        }
        Subject::Vibe(style) => {
            let kind = EnemyKind::Head;
            let s = stats(kind);
            // Straight from the game's own builder, so the lab cannot preview
            // a look `enemy.rs` does not actually spawn.
            let (body, face) = vibe_materials(style, &assets, &mut materials);
            let quad = meshes.add(Rectangle::new(1.0, 1.0));
            let parent = commands
                .spawn((
                    SubjectRoot,
                    // Resting height + the cube's own half-height, as `enemy.rs`
                    // sits it on the ground.
                    Transform::from_xyz(0.0, s.hover + s.visual * 0.5, 0.0),
                    Visibility::default(),
                ))
                .id();
            commands.spawn((
                ChildOf(parent),
                Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
                MeshMaterial3d(body),
                Transform::from_scale(Vec3::splat(s.visual)),
            ));
            for mut t in face_placements(kind) {
                t.translation *= s.visual;
                t.scale *= s.visual;
                commands.spawn((
                    ChildOf(parent),
                    Mesh3d(quad.clone()),
                    MeshMaterial3d(face.clone()),
                    t,
                ));
            }
        }
        Subject::Shot(i) => {
            let look = ShotLook::ALL[i];
            commands.spawn((
                SubjectRoot,
                Mesh3d(shot_mesh(look, &mut meshes)),
                MeshMaterial3d(shot_material(look, &mut materials)),
                // Blown up off the in-flight scale so the shape is legible;
                // the `game` view shows the true size.
                Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::splat(1.6)),
            ));
        }
        Subject::Gear(i) => {
            let gear = props::spawn_prop(&mut commands, &mut meshes, &mut materials, Prop::ALL[i]);
            commands.entity(gear).insert((
                SubjectRoot,
                // Blown up off the hand scale so the shape is actually legible.
                Transform::from_xyz(0.0, 1.0, 0.0)
                    .with_rotation(Quat::from_rotation_y(-FRAC_PI_2))
                    .with_scale(Vec3::splat(1.6)),
            ));
        }
    }
}

/// Dress the freshly instantiated glTF the way the game dresses it, and collect
/// **every** baked clip into one graph so the lab can page through them.
fn on_ready(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    mut lab: ResMut<Lab>,
    assets: Res<AssetServer>,
    model: Query<&LabModel>,
    children: Query<&Children>,
    names: Query<&Name>,
    mat_names: Query<&GltfMaterialName>,
    mesh_mats: Query<&MeshMaterial3d<StandardMaterial>>,
    gltfs: Res<Assets<Gltf>>,
    clips: Res<Assets<AnimationClip>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut players: Query<&mut AnimationPlayer>,
) {
    use std::collections::HashMap;

    let root = ready.entity;
    let Ok(LabModel { gltf, role, atlas }) = model.get(root) else {
        return;
    };
    let role = *role;

    // A boxy character wears one painted atlas on every body part — no tints,
    // no face plane, because the face is in the texture.
    if let Some(atlas) = atlas {
        let skinned = materials.add(StandardMaterial {
            base_color_texture: Some(atlas.clone()),
            perceptual_roughness: 0.82,
            metallic: 0.0,
            ..default()
        });
        for e in children.iter_descendants(root) {
            if mesh_mats.contains(e) {
                commands.entity(e).insert(MeshMaterial3d(skinned.clone()));
            }
        }
    }

    let mut recoloured: HashMap<AssetId<StandardMaterial>, Handle<StandardMaterial>> =
        HashMap::new();
    let mut head = None;
    let mut hand = None;
    // The bone the current carry pose hangs its prop off, for this rig.
    let rig = lab.subject().rig();
    let want_bone = lab.carry.bone(rig, false);
    let mut mount = None;
    for e in children.iter_descendants(root) {
        if let (Ok(mat_name), Ok(cur)) = (mat_names.get(e), mesh_mats.get(e))
            && let Some(tint) = role.tint(mat_name.0.as_str())
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
            if name.as_str() == want_bone {
                mount = Some(e);
            }
            match name.as_str() {
                "Head" => head = Some(e),
                "Wrist.R" => hand = Some(e),
                // Some Quaternius characters ship carrying stock kit — a
                // pistol (`suit`, `swat`), a bedroll (`adventurer`). No role in
                // this game uses either, so the game hides them too.
                "Pistol" | "Backpack" if role != Role::Stock => {
                    commands.entity(e).insert(Visibility::Hidden);
                }
                _ => {}
            }
        }
    }

    // Face plane on the `Head` bone — the same quad `player.rs` / `enemy.rs`
    // parent there, at the offsets nudge mode is editing.
    if let (Some(head), Some(face)) = (head, role.face()) {
        commands.spawn((
            FaceQuad,
            ChildOf(head),
            Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial3d(materials.add(face_material(assets.load(face)))),
            Transform::from_translation(lab.face_off).with_scale(lab.face_scale.extend(1.0)),
        ));
    }

    // Whatever gear the lab is currently reviewing, mounted the way the game
    // mounts it: `props::carry_transform` is the single source for the fit, so
    // a number dialled in here is the number the game uses.
    if let (Some(mount), Some(prop)) = (mount, lab.prop.map(|i| Prop::ALL[i])) {
        let gear = props::spawn_prop(&mut commands, &mut meshes, &mut materials, prop);
        commands.entity(gear).insert((
            LabProp,
            ChildOf(mount),
            props::carry_transform(lab.carry, prop, rig, false),
        ));
    }
    let _ = hand;

    let Some(gltf) = gltfs.get(gltf) else {
        return;
    };
    let Some(anim_entity) = children
        .iter_descendants(root)
        .find(|e| players.contains(*e))
    else {
        // Plenty of props are static — that's fine, there's just nothing to play.
        return;
    };

    let mut named: Vec<(String, Handle<AnimationClip>)> = gltf
        .named_animations
        .iter()
        .map(|(n, h)| (n.to_string(), h.clone()))
        .collect();
    named.sort_by(|a, b| a.0.cmp(&b.0));
    if named.is_empty() {
        return;
    }

    let (graph, idx) = AnimationGraph::from_clips(named.iter().map(|(_, h)| h.clone()));
    let graph = graphs.add(graph);

    lab.clips = named
        .iter()
        .zip(idx.iter())
        .map(|((name, handle), node)| {
            let dur = clips.get(handle).map(|c| c.duration()).unwrap_or(0.0);
            (name.clone(), *node, dur)
        })
        .collect();
    // Open on a resting pose when the model has one. Quaternius names it
    // `Idle`, Kenney `idle` — without the fold the boxy cast opened mid-kick.
    lab.clip = lab
        .clips
        .iter()
        .position(|(n, _, _)| n.eq_ignore_ascii_case("idle"))
        .unwrap_or(0);

    let playing = lab.clips[lab.clip].1;
    let mut player = players.get_mut(anim_entity).unwrap();
    player.play(playing).repeat();

    commands
        .entity(anim_entity)
        .insert((AnimationGraphHandle(graph), LabAnim { playing }));
}

// ---------------------------------------------------------------------------
// Playback
// ---------------------------------------------------------------------------

/// Push the lab's transport state onto the live `AnimationPlayer` each frame.
fn apply_playback(lab: Res<Lab>, mut anim: Query<(&mut AnimationPlayer, &mut LabAnim)>) {
    let Ok((mut player, mut state)) = anim.single_mut() else {
        return;
    };
    let Some(&(_, want, _)) = lab.clips.get(lab.clip) else {
        return;
    };

    if state.playing != want {
        player.stop_all();
        player.play(want);
        state.playing = want;
    }
    if let Some(active) = player.animation_mut(want) {
        active.set_speed(lab.speed);
        active.set_repeat(if lab.looping {
            RepeatAnimation::Forever
        } else {
            RepeatAnimation::Never
        });
        if lab.paused {
            active.pause();
        } else {
            active.resume();
        }
    }
}

/// Keep the face quad and the mounted prop on the numbers nudge mode is
/// editing, so the change is visible the instant a key is pressed.
///
/// Only the *active* nudge target is written. This used to overwrite both
/// unconditionally, which meant the nudge defaults silently replaced whatever
/// `props::carry_transform` had just placed — a guitar that should have hung
/// across the chest turned up shrunk, down by the character's knee.
fn apply_nudge(
    lab: Res<Lab>,
    mut face: Query<&mut Transform, (With<FaceQuad>, Without<LabProp>)>,
    mut gun: Query<&mut Transform, (With<LabProp>, Without<FaceQuad>)>,
) {
    if !lab.is_changed() {
        return;
    }
    if lab.nudge == Nudge::Face {
        for mut t in &mut face {
            t.translation = lab.face_off;
            t.scale = lab.face_scale.extend(1.0);
        }
    }
    if lab.nudge == Nudge::Prop {
        for mut t in &mut gun {
            t.translation = lab.gun_off;
            t.scale = Vec3::splat(lab.gun_scale);
        }
    }
}

fn apply_ground(
    lab: Res<Lab>,
    mat: Res<GroundMat>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<bevy::light::GlobalAmbientLight>,
    mut fog: Query<&mut bevy::pbr::DistanceFog>,
    mut sun: Query<&mut DirectionalLight, With<shooty::KeySun>>,
) {
    if !lab.is_changed() {
        return;
    }
    if let Some(mut m) = materials.get_mut(&mat.0) {
        m.base_color = GROUNDS[lab.ground].1;
    }
    apply_grade(
        lab.grade,
        // The lab shows one character on a turntable, so it always wants the
        // town's fog distances whatever level is loaded.
        1.0,
        &mut clear,
        &mut ambient,
        fog.single_mut().ok().as_deref_mut(),
        sun.single_mut().ok().as_deref_mut(),
    );
}

/// Take the subject's world-space extents on the first frame they exist, and
/// hold them — measuring every frame would make the camera breathe with the
/// animation.
fn measure_subject(
    mut bounds: ResMut<Bounds>,
    roots: Query<Entity, With<SubjectRoot>>,
    children: Query<&Children>,
    boxes: Query<(&bevy::camera::primitives::Aabb, &GlobalTransform)>,
) {
    if bounds.measured {
        return;
    }
    let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut any = false;
    for root in &roots {
        for e in std::iter::once(root).chain(children.iter_descendants(root)) {
            let Ok((aabb, gt)) = boxes.get(e) else {
                continue;
            };
            let c: Vec3 = aabb.center.into();
            let h: Vec3 = aabb.half_extents.into();
            // Eight corners through the world transform — cheap and exact
            // enough for framing.
            for i in 0..8 {
                let corner = c + Vec3::new(
                    if i & 1 == 0 { -h.x } else { h.x },
                    if i & 2 == 0 { -h.y } else { h.y },
                    if i & 4 == 0 { -h.z } else { h.z },
                );
                let w = gt.transform_point(corner);
                min = min.min(w);
                max = max.max(w);
                any = true;
            }
        }
    }
    if any {
        bounds.min = min;
        bounds.max = max;
        bounds.measured = true;
    }
}

fn drive_camera(
    mut lab: ResMut<Lab>,
    bounds: Res<Bounds>,
    mut cam: Query<(&mut Transform, &Projection), With<LabCam>>,
) {
    let Ok((mut t, projection)) = cam.single_mut() else {
        return;
    };

    // The game view is an absolute distance — a respawn's refit must not eat it.
    if let Some(d) = lab.absolute {
        lab.dist = d;
        lab.target_y = 0.0;
        lab.refit = false;
    } else if lab.refit && bounds.measured {
        let h = bounds.height();
        let half_fov = match projection {
            Projection::Perspective(p) => p.fov * 0.5,
            _ => std::f32::consts::FRAC_PI_8,
        };
        lab.dist = (h * lab.frame / (2.0 * lab.fill)) / half_fov.tan();
        lab.target_y = bounds.min.y + h * lab.look;
        lab.refit = false;
    }

    let target = Vec3::new(0.0, lab.target_y, 0.0);
    let (sy, cy) = lab.yaw.sin_cos();
    let dir = Vec3::new(sy * lab.pitch.cos(), lab.pitch.sin(), cy * lab.pitch.cos());
    t.translation = target + dir * lab.dist;
    t.look_at(target, Vec3::Y);
}

/// A metre grid and a 1.8 m human-height ruler stood beside the subject — the
/// scale question ("is this person-sized against the town?") answered instead
/// of guessed.
fn draw_reference(lab: Res<Lab>, bounds: Res<Bounds>, mut gizmos: Gizmos) {
    if !lab.ruler {
        return;
    }
    gizmos.grid(
        Isometry3d::new(Vec3::ZERO, Quat::from_rotation_x(-FRAC_PI_2)),
        UVec2::splat(24),
        Vec2::splat(1.0),
        Color::srgba(1.0, 1.0, 1.0, 0.10),
    );
    // Stand the post just clear of the subject so it never hides inside it.
    let x = if bounds.measured {
        bounds.max.x + 0.25
    } else {
        1.1
    };
    let tick = (bounds.height() * 0.12).clamp(0.08, 1.2);
    for i in 0..9 {
        let (a, b) = (i as f32 * 0.2, (i as f32 + 1.0) * 0.2);
        let c = if i % 2 == 0 {
            Color::srgb(0.95, 0.95, 1.0)
        } else {
            Color::srgb(0.15, 0.15, 0.2)
        };
        gizmos.line(Vec3::new(x, a, 0.0), Vec3::new(x, b, 0.0), c);
    }
    // 1 m and 1.8 m ticks — average adult eyeline is the one that matters.
    for (y, c) in [
        (1.0, Color::srgb(0.3, 0.9, 1.0)),
        (1.8, Color::srgb(1.0, 0.7, 0.2)),
    ] {
        gizmos.line(Vec3::new(x - tick, y, 0.0), Vec3::new(x + tick, y, 0.0), c);
    }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

fn keys(keys: Res<ButtonInput<KeyCode>>, mut lab: ResMut<Lab>) {
    use KeyCode::*;
    let shift = keys.pressed(ShiftLeft) || keys.pressed(ShiftRight);
    let step = if shift { 0.05 } else { 0.005 };

    // --- nudge modes take the arrow keys ---
    if lab.nudge != Nudge::Off {
        let mut d = Vec3::ZERO;
        if keys.pressed(ArrowLeft) {
            d.x -= step
        }
        if keys.pressed(ArrowRight) {
            d.x += step
        }
        if keys.pressed(ArrowUp) {
            d.y += step
        }
        if keys.pressed(ArrowDown) {
            d.y -= step
        }
        if keys.pressed(PageUp) {
            d.z += step
        }
        if keys.pressed(PageDown) {
            d.z -= step
        }
        let s = if keys.pressed(KeyO) {
            step
        } else if keys.pressed(KeyI) {
            -step
        } else {
            0.0
        };
        if d != Vec3::ZERO || s != 0.0 {
            match lab.nudge {
                Nudge::Face => {
                    lab.face_off += d;
                    lab.face_scale += Vec2::splat(s);
                }
                Nudge::Prop => {
                    lab.gun_off += d;
                    lab.gun_scale = (lab.gun_scale + s).max(0.01);
                }
                Nudge::Off => {}
            }
        }
    }

    let mut dirty = false;
    if keys.just_pressed(BracketRight) {
        lab.idx = (lab.idx + 1) % lab.subjects.len();
        dirty = true;
    }
    if keys.just_pressed(BracketLeft) {
        lab.idx = (lab.idx + lab.subjects.len() - 1) % lab.subjects.len();
        dirty = true;
    }
    // Gear review: what is carried, and how it is worn.
    if keys.just_pressed(KeyB) {
        lab.prop = match lab.prop {
            None => Some(0),
            Some(i) if i + 1 < Prop::ALL.len() => Some(i + 1),
            Some(_) => None,
        };
        dirty = true;
    }
    if keys.just_pressed(KeyM) {
        let i = Carry::ALL.iter().position(|c| *c == lab.carry).unwrap_or(0);
        lab.carry = Carry::ALL[(i + 1) % Carry::ALL.len()];
        dirty = true;
    }
    if keys.just_pressed(Tab) {
        let n = Role::count();
        let step = if shift { n - 1 } else { 1 };
        lab.role = Role::at((lab.role.index() + step) % n);
        // Follow the skin to the body it was cut for; `[` / `]` still move the
        // body freely afterwards, keeping the skin on.
        if let Some(i) = lab.home_body(lab.role) {
            lab.idx = i;
        }
        dirty = true;
    }
    if keys.just_pressed(Semicolon) {
        lab.scale_mul = (lab.scale_mul - 0.02).max(0.1);
        dirty = true;
    }
    if keys.just_pressed(Quote) {
        lab.scale_mul += 0.02;
        dirty = true;
    }
    if keys.just_pressed(KeyC) {
        lab.scale_mul = 1.0;
        dirty = true;
    }
    if dirty {
        lab.dirty = true;
    }

    // --- transport ---
    if keys.just_pressed(Period) && !lab.clips.is_empty() {
        lab.clip = (lab.clip + 1) % lab.clips.len();
    }
    if keys.just_pressed(Comma) && !lab.clips.is_empty() {
        lab.clip = (lab.clip + lab.clips.len() - 1) % lab.clips.len();
    }
    if keys.just_pressed(Space) {
        lab.paused = !lab.paused;
    }
    if keys.just_pressed(KeyL) {
        lab.looping = !lab.looping;
    }
    if keys.just_pressed(Minus) {
        lab.speed = (lab.speed - 0.1).max(0.05);
    }
    if keys.just_pressed(Equal) {
        lab.speed = (lab.speed + 0.1).min(3.0);
    }
    if keys.just_pressed(Digit0) {
        lab.speed = 1.0;
    }

    // --- views ---
    for (k, name) in [
        (Digit1, "3/4"),
        (Digit2, "front"),
        (Digit3, "side"),
        (Digit4, "back"),
        (Digit5, "top"),
        (Digit6, "face"),
        (Digit7, "game"),
    ] {
        if keys.just_pressed(k) {
            lab.preset(name);
        }
    }

    // --- scene ---
    if keys.just_pressed(KeyG) {
        lab.ground = (lab.ground + 1) % GROUNDS.len();
    }
    if keys.just_pressed(KeyN) {
        lab.grade = if lab.grade > 0.5 { 0.0 } else { 1.0 };
    }
    if keys.just_pressed(KeyR) {
        lab.ruler = !lab.ruler;
    }
    if keys.just_pressed(Slash) {
        lab.help = !lab.help;
    }
    if keys.just_pressed(KeyF) {
        lab.nudge = if lab.nudge == Nudge::Face {
            Nudge::Off
        } else {
            Nudge::Face
        };
    }
    if keys.just_pressed(KeyH) {
        lab.nudge = if lab.nudge == Nudge::Prop {
            Nudge::Off
        } else {
            // Seed the editable numbers from the pose on screen, so nudging
            // starts where the carry left off instead of jumping.
            if let Some(prop) = lab.prop.map(|i| Prop::ALL[i]) {
                let t = props::carry_transform(lab.carry, prop, lab.subject().rig(), false);
                lab.gun_off = t.translation;
                lab.gun_scale = t.scale.x;
            }
            Nudge::Prop
        };
    }
    if keys.just_pressed(KeyP) {
        let line = lab.nudge_literal();
        let msg = if line.is_empty() {
            format!(
                "[lab] {} · {} · clip {} · scale {:.3}",
                lab.subject().label(),
                lab.role.name(),
                lab.clips.get(lab.clip).map(|c| c.0.as_str()).unwrap_or("-"),
                lab.scale()
            )
        } else {
            format!("[lab] {line}")
        };
        println!("{msg}");
        lab.note = msg;
    }
}

fn orbit(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut lab: ResMut<Lab>,
) {
    let mut dyaw = 0.0;
    let mut dpitch = 0.0;
    if mouse.pressed(MouseButton::Left) {
        dyaw -= motion.delta.x * 0.006;
        dpitch += motion.delta.y * 0.006;
    }
    // Nudge mode owns the arrows, so orbiting also lives on A/D/W/S.
    let k = time.delta_secs() * 1.6;
    if keys.pressed(KeyCode::KeyA) {
        dyaw -= k
    }
    if keys.pressed(KeyCode::KeyD) {
        dyaw += k
    }
    if keys.pressed(KeyCode::KeyW) {
        dpitch += k
    }
    if keys.pressed(KeyCode::KeyS) {
        dpitch -= k
    }

    let dz = -scroll.delta.y * 0.4
        + if keys.pressed(KeyCode::KeyQ) {
            k * 4.0
        } else if keys.pressed(KeyCode::KeyE) {
            -k * 4.0
        } else {
            0.0
        };

    if dyaw != 0.0 || dpitch != 0.0 || dz != 0.0 {
        lab.yaw += dyaw;
        lab.pitch = (lab.pitch + dpitch).clamp(-0.3, 1.5);
        lab.dist = (lab.dist + dz * lab.dist * 0.12).clamp(0.15, 200.0);
        lab.absolute = None;
        lab.refit = false;
        lab.preset = "free";
    }
}

fn shoot_window_screenshot(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    lab: Res<Lab>,
    mut n: Local<u32>,
) {
    if !keys.just_pressed(KeyCode::Enter) {
        return;
    }
    std::fs::create_dir_all("screenshots/lab").ok();
    let slug: String = lab
        .subject()
        .label()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    *n += 1;
    let path = format!("screenshots/lab/{slug}_{}_{:02}.png", lab.role.name(), *n);
    println!("[lab] saving {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
}

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

fn update_hud(
    lab: Res<Lab>,
    anim: Query<&AnimationPlayer>,
    mut hud: Query<&mut Text, (With<LabHud>, Without<LabHelp>)>,
    mut help: Query<(&mut Text, &mut Visibility), With<LabHelp>>,
) {
    let Ok(mut text) = hud.single_mut() else {
        return;
    };
    let (clip, dur, at) = match lab.clips.get(lab.clip) {
        Some((name, node, dur)) => {
            let at = anim
                .iter()
                .find_map(|p| p.animation(*node).map(|a| a.seek_time()))
                .unwrap_or(0.0);
            (name.as_str(), *dur, at)
        }
        None => ("- static -", 0.0, 0.0),
    };

    let mut s = format!(
        "{}   [{}/{}]\n\
         skin  {}\n\
         scale {:.3}  ({:.2} shipped x {:.2})   [{}/{}]\n\
         clip  {}  ({}/{})   {:.2}s / {:.2}s   {:.2}x  {}  {}\n\
         gear  {}  ·  {}\n\
         view  {}   ground {}   {}",
        lab.subject().label(),
        lab.idx + 1,
        lab.subjects.len(),
        lab.role.name(),
        lab.scale(),
        lab.role.scale(),
        lab.scale_mul,
        lab.role.index() + 1,
        Role::count(),
        clip,
        (lab.clip + 1).min(lab.clips.len().max(1)),
        lab.clips.len(),
        at,
        dur,
        lab.speed,
        if lab.paused { "PAUSED" } else { "playing" },
        if lab.looping { "loop" } else { "once" },
        lab.prop.map_or("- none -", |i| Prop::ALL[i].name()),
        lab.carry.name(),
        lab.preset,
        GROUNDS[lab.ground].0,
        if lab.grade > 0.5 { "daylight" } else { "GLOOM" },
    );

    if lab.nudge != Nudge::Off {
        s.push_str(&format!(
            "\n\nNUDGE {} - arrows XY, PgUp/PgDn Z, I/O scale, Shift coarse\n  {}",
            match lab.nudge {
                Nudge::Face => "face plane",
                Nudge::Prop => "prop on bone",
                Nudge::Off => "",
            },
            lab.nudge_literal(),
        ));
    }
    if !lab.note.is_empty() {
        s.push_str(&format!("\n\n{}", lab.note));
    }
    text.0 = s;

    if let Ok((mut help, mut vis)) = help.single_mut() {
        *vis = if lab.help {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if help.0.is_empty() {
            help.0 = "[ ] body   Tab / Shift-Tab skin (jumps to its body)   ; ' scale   C reset scale\n\
                 , . clip   Space pause   - = speed   0 reset   L loop\n\
                 1-7 views 3/4 front side back top face GAME   ADWS/QE orbit   drag+scroll\n\
                 G ground   N gloom   R ruler   F face-nudge   H gun-nudge   P print   / help   Enter screenshot"
                .into();
        }
    }
}

// ---------------------------------------------------------------------------
// Offscreen contact sheet
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Shoot {
    dir: String,
    /// Shoot only the boxy-vs-Quaternius comparison, not the whole cast.
    compare: bool,
    /// Shoot only the gear review: props x carry poses.
    gear: bool,
    tick: u32,
    item: usize,
    plan: Vec<Shot>,
}

/// One frame of a contact sheet.
#[derive(Clone, Copy)]
struct Shot {
    subject: usize,
    role: Role,
    view: &'static str,
    /// Gear to mount, or `None` to leave the character empty-handed.
    prop: Option<usize>,
    carry: Carry,
    /// Substring of the clip to hold this frame on, case-insensitive. A gun
    /// pose only makes sense over a shooting animation.
    clip: Option<&'static str>,
}

impl Shot {
    fn new(subject: usize, role: Role, view: &'static str) -> Self {
        Self { subject, role, view, prop: None, carry: Carry::Slung, clip: None }
    }
    fn gear(mut self, prop: usize, carry: Carry) -> Self {
        self.prop = Some(prop);
        self.carry = carry;
        // Show each pose from where it can actually be judged: gear on the
        // back needs the back view, and gear in the hand needs the hand up.
        match carry {
            Carry::Back => self.view = "back",
            Carry::Aim => self.clip = Some("shoot"),
            Carry::Slung => {}
        }
        self
    }
}

#[derive(Resource)]
struct ShotTarget(Handle<Image>);

fn setup_shot_target(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut shoot: ResMut<Shoot>,
    lab: Res<Lab>,
) {
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = images.add(image);
    commands.spawn((
        LabCam,
        shooty::camera_bundle(),
        RenderTarget::Image(handle.clone().into()),
    ));
    commands.insert_resource(ShotTarget(handle));

    // Every skin on the body it was cut for: a 3/4 turnaround and a face
    // close-up each, plus the true in-play framing for the three the game
    // actually ships. Then the vibe cubes, the gun, and any body no skin
    // claims, shown stock so nothing in `assets/models` goes unlooked-at.
    if shoot.gear {
        // One body per rig, every prop, every carry pose — the sheet that
        // answers "what does a snare on the back actually look like".
        let boxy = lab
            .subjects
            .iter()
            .position(|s| matches!(s, Subject::Boxy(0)))
            .unwrap_or(0);
        let quat = skins::SKINS
            .iter()
            .position(|sk| sk.id == "guitarist")
            .and_then(|i| lab.home_body(Role::Skin(i)));
        // Does worn gear read from the camera the game is actually played at?
        // A guitar flat against the chest can be invisible from above.
        for rig_subject in [Some(boxy), quat] {
            let Some(sub) = rig_subject else { continue };
            let role = if sub == boxy {
                Role::Stock
            } else {
                Role::Skin(skins::SKINS.iter().position(|sk| sk.id == "guitarist").unwrap())
            };
            for carry in [Carry::Slung, Carry::Back] {
                let mut shot = Shot::new(sub, role, "3/4").gear(0, carry);
                shot.view = "game";
                shoot.plan.push(shot);
            }
        }
        for prop in 0..Prop::ALL.len() {
            for carry in Carry::ALL {
                shoot
                    .plan
                    .push(Shot::new(boxy, Role::Stock, "3/4").gear(prop, carry));
                if let Some(q) = quat {
                    let role = Role::Skin(
                        skins::SKINS.iter().position(|sk| sk.id == "guitarist").unwrap(),
                    );
                    shoot.plan.push(Shot::new(q, role, "3/4").gear(prop, carry));
                }
            }
        }
        // Each piece of gear, and each candidate projectile, on its own — so
        // the modelling can be judged apart from the fit.
        for (i, s) in lab.subjects.iter().enumerate() {
            match s {
                Subject::Gear(_) => shoot.plan.push(Shot::new(i, Role::Stock, "3/4")),
                // A projectile has to be judged at the distance it is actually
                // seen from, so each candidate gets the true in-play framing
                // as well as a legible close-up.
                Subject::Shot(_) => {
                    shoot.plan.push(Shot::new(i, Role::Stock, "3/4"));
                    shoot.plan.push(Shot::new(i, Role::Stock, "game"));
                }
                _ => {}
            }
        }
        return;
    }

    // The boxy cast, always: a 3/4 turnaround and the true in-play framing for
    // each of the ten. These lead the sheet.
    for (i, s) in lab.subjects.iter().enumerate() {
        if let Subject::Boxy(_) = s {
            shoot.plan.push(Shot::new(i, Role::Stock, "3/4"));
            shoot.plan.push(Shot::new(i, Role::Stock, "game"));
        }
    }
    if shoot.compare {
        // ... and the Quaternius skin of the same genre next to it, so the two
        // lines can be held against each other frame for frame.
        for id in ["metal", "grunge", "disco", "techno", "hiphop", "country", "jpop"] {
            let Some(i) = skins::SKINS.iter().position(|sk| sk.id == id) else {
                continue;
            };
            let role = Role::Skin(i);
            if let Some(body) = lab.home_body(role) {
                shoot.plan.push(Shot::new(body, role, "3/4"));
                shoot.plan.push(Shot::new(body, role, "game"));
            }
        }
        return;
    }

    let mut shot_bodies = std::collections::HashSet::new();
    for i in 0..skins::SKINS.len() {
        let role = Role::Skin(i);
        let Some(body) = lab.home_body(role) else {
            continue;
        };
        shot_bodies.insert(body);
        shoot.plan.push(Shot::new(body, role, "3/4"));
        shoot.plan.push(Shot::new(body, role, "face"));
        if matches!(skins::SKINS[i].id, "guitarist" | "drummer" | "gloom") {
            shoot.plan.push(Shot::new(body, role, "game"));
        }
    }
    for (i, s) in lab.subjects.iter().enumerate() {
        match s {
            Subject::Glb { .. } if s.is_character() && !shot_bodies.contains(&i) => {
                shoot.plan.push(Shot::new(i, Role::Stock, "3/4"));
            }
            Subject::Vibe(_) => {
                shoot.plan.push(Shot::new(i, Role::Stock, "3/4"));
                shoot.plan.push(Shot::new(i, Role::Stock, "game"));
            }
            Subject::Gear(_) | Subject::Shot(_) => {
                shoot.plan.push(Shot::new(i, Role::Stock, "3/4"))
            }
            _ => {}
        }
    }
}

/// Per item: settle (the glTF has to load and instantiate), grab a frame, move
/// on. Deliberately dumb frame budgets — a lab sheet isn't timing-critical.
const SETTLE: u32 = 110;
const HOLD: u32 = 150;

fn run_shot(
    mut commands: Commands,
    mut shoot: ResMut<Shoot>,
    mut lab: ResMut<Lab>,
    target: Res<ShotTarget>,
    mut exit: MessageWriter<AppExit>,
    mut strat: ResMut<TimeUpdateStrategy>,
) {
    if shoot.item >= shoot.plan.len() {
        exit.write(AppExit::Success);
        return;
    }
    let shot = shoot.plan[shoot.item];
    let (idx, role, view) = (shot.subject, shot.role, shot.view);
    // The clip list only exists once the glTF has instantiated, so the pick
    // happens a few ticks in rather than at tick 0.
    if shoot.tick == SETTLE / 2
        && let Some(want) = shot.clip
        && let Some(i) = lab
            .clips
            .iter()
            .position(|(n, _, _)| n.to_ascii_lowercase().contains(want))
    {
        lab.clip = i;
    }

    if shoot.tick == 0 {
        lab.idx = idx;
        lab.role = role;
        lab.prop = shot.prop;
        lab.carry = shot.carry;
        lab.preset(shot.view);
        lab.dirty = true;
        lab.help = false;
    }
    // Freeze time while the model loads so the clip starts from its first frame.
    *strat = TimeUpdateStrategy::ManualDuration(if shoot.tick < SETTLE {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(1.0 / 30.0)
    });

    shoot.tick += 1;
    if shoot.tick == SETTLE + 20 {
        let slug: String = lab
            .subject()
            .label()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        // Gear frames name what is being worn and how, so a sheet can be
        // globbed by prop or by pose.
        let gear = match shot.prop {
            Some(i) => format!(
                "_{}_{}",
                Prop::ALL[i].name().replace(' ', ""),
                match shot.carry {
                    Carry::Slung => "slung",
                    Carry::Aim => "aim",
                    Carry::Back => "back",
                }
            ),
            None => String::new(),
        };
        let path = format!(
            "{}/{:02}_{}_{}_{}{}.png",
            shoot.dir,
            shoot.item,
            slug,
            role.skin().map_or("stock", |sk| sk.id),
            view.replace('/', ""),
            gear
        );
        println!("[lab] {path}");
        commands
            .spawn(Screenshot::image(target.0.clone()))
            .observe(save_to_disk(path))
            .observe(|_: On<ScreenshotCaptured>| {});
    }
    if shoot.tick >= HOLD {
        shoot.tick = 0;
        shoot.item += 1;
    }
}
