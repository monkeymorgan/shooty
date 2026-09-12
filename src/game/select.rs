//! **Character select** — the lineup you pick a rocker from before a run.
//!
//! It is a real 3D lineup rather than a grid of cards, for one reason: the
//! thing you are choosing between is how a character *looks and moves*, and
//! half the point of this screen is being able to put the blocky cast and the
//! skeletal cast next to each other and see the difference. A portrait would
//! hide exactly what you came to compare.
//!
//! Each character stands on the stage in their idle animation with their own
//! **waveform** turning beside them — the shape their shots are made of (see
//! [`skins::sound`]). Picking a character picks a weapon.

use std::time::Duration;

use bevy::gltf::{Gltf, GltfMaterialName};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};

use super::player::{boxy_atlas, face_in_head, face_material};
use super::roster::{Gait, Pick, Roster};
use super::weapon::wave_mesh;
use super::{GameState, Hero, Party, RunEntity, skins};

/// How far apart the characters stand.
const SPACING: f32 = 2.6;
/// How far the off-centre characters fall back, per step from the middle.
const RECEDE: f32 = 0.55;
/// Scale applied to whoever is currently under the cursor, over their own.
const FOCUS_SWELL: f32 = 1.14;
/// Phases baked for the turning waveform beside each character.
const WAVE_PHASES: usize = 16;

pub struct SelectPlugin;

impl Plugin for SelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Choice>()
            .add_systems(
                OnEnter(GameState::Select),
                (clear_previous_run, open_stage, spawn_ui).chain(),
            )
            .add_systems(OnExit(GameState::Select), close_stage)
            .add_systems(
                Update,
                (
                    choose,
                    lay_out_lineup,
                    turn_waveforms,
                    update_ui,
                    frame_camera,
                )
                    .chain()
                    .run_if(in_state(GameState::Select)),
            );
    }
}

/// Present in the capture binary's `select` mode: walk the cursor along the
/// lineup on a timer instead of reading input, so the screen can be recorded.
#[derive(Resource)]
pub struct BrowseCast;

/// Where the cursor is and which hero slot it is filling.
#[derive(Resource)]
struct Choice {
    cursor: usize,
    /// Which hero the cursor is currently choosing for. Only ever
    /// [`Hero::Drummer`] in a two-player party.
    slot: Hero,
    /// Eased cursor, so the lineup slides rather than snapping.
    slide: f32,
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            cursor: 0,
            slot: Hero::Guitarist,
            slide: 0.0,
        }
    }
}

/// Everything the select screen spawns, torn down when it closes.
#[derive(Component)]
struct SelectEntity;

/// One character standing in the lineup.
#[derive(Component)]
struct LineupSlot {
    index: usize,
}

/// The turning waveform beside a character.
#[derive(Component)]
struct WavePreview {
    phases: Vec<Handle<Mesh>>,
    hz: f32,
}

/// Carried on a model root until its glTF scene instantiates.
#[derive(Component)]
struct Dressing {
    pick: Pick,
    gltf: Handle<Gltf>,
}

// ---------------------------------------------------------------------------
// The stage
// ---------------------------------------------------------------------------

fn open_stage(
    mut commands: Commands,
    mut choice: ResMut<Choice>,
    roster: Res<Roster>,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Open on whoever is already in the slot, so coming back from a run puts
    // the cursor where you left it.
    let cast = Pick::playable();
    choice.slot = Hero::Guitarist;
    choice.cursor = cast
        .iter()
        .position(|p| *p == roster.guitarist)
        .unwrap_or(0);
    choice.slide = choice.cursor as f32;

    // A dark stage floor, so the characters are lit against something rather
    // than floating in fog.
    commands.spawn((
        SelectEntity,
        Mesh3d(meshes.add(Cylinder::new(28.0, 0.4))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.07, 0.07, 0.10),
            perceptual_roughness: 0.85,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.2, 0.0),
    ));
    // Warm key from the front, cool rim from behind: a stage wash, not the
    // town's daylight.
    commands.spawn((
        SelectEntity,
        DirectionalLight {
            illuminance: 12_000.0,
            color: Color::srgb(1.0, 0.94, 0.86),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 12.0).looking_at(Vec3::Y, Vec3::Y),
    ));
    commands.spawn((
        SelectEntity,
        DirectionalLight {
            illuminance: 6_000.0,
            color: Color::srgb(0.45, 0.62, 1.0),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-6.0, 6.0, -10.0).looking_at(Vec3::Y, Vec3::Y),
    ));

    for (index, pick) in cast.into_iter().enumerate() {
        let sound = pick.sound();
        let slot = commands
            .spawn((
                SelectEntity,
                LineupSlot { index },
                Transform::default(),
                Visibility::default(),
            ))
            .id();

        // A ground ring in the character's own sound colour — the same
        // find-me ring they wear in play, and a swatch of what they fire.
        commands.spawn((
            ChildOf(slot),
            Mesh3d(meshes.add(Torus {
                minor_radius: 0.05,
                major_radius: 0.95,
            })),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: sound.color,
                emissive: {
                    let l = sound.color.to_linear();
                    LinearRgba::rgb(l.red * 1.8, l.green * 1.8, l.blue * 1.8)
                },
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(0.0, 0.05, 0.0),
        ));

        // The character. Faces +Z, which is toward the camera.
        commands
            .spawn((
                ChildOf(slot),
                WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(pick.model()))),
                Dressing {
                    pick,
                    gltf: assets.load(pick.model()),
                },
                Transform::from_xyz(0.0, pick.lift(), 0.0)
                    .with_scale(Vec3::splat(pick.model_scale())),
            ))
            .observe(dress_for_the_lineup);

        // Their waveform, turning above their head — the shape their shots are
        // made of, shown before you commit to firing it. Only the character
        // under the cursor shows theirs: twelve at once is a hedge, not a
        // lineup.
        let phases: Vec<Handle<Mesh>> = (0..WAVE_PHASES)
            .map(|i| wave_mesh(sound, i as f32 / WAVE_PHASES as f32, &mut meshes))
            .collect();
        let l = sound.color.to_linear();
        commands.spawn((
            ChildOf(slot),
            WavePreview {
                phases: phases.clone(),
                hz: sound.hz * 0.35,
            },
            Mesh3d(phases[0].clone()),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: sound.color,
                emissive: LinearRgba::rgb(l.red * 3.0, l.green * 3.0, l.blue * 3.0),
                ..default()
            })),
            // Stood up and turned side-on to the camera, so it reads the way a
            // waveform is drawn: running left to right, swinging up and down.
            // The mesh is built running down +Z and swinging across X, so Z
            // goes to screen-X and X to screen-Y.
            Transform::from_xyz(0.0, 3.05, 0.0)
                .with_rotation(
                    Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)
                        * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                )
                .with_scale(Vec3::splat(0.95)),
            Visibility::Hidden,
        ));
    }
}

/// Tear down a finished run when we come back to the lineup from the game-over
/// or victory screen (M on those screens). On the app's first Select there is
/// nothing tagged [`RunEntity`] yet, so this is a no-op then.
fn clear_previous_run(mut commands: Commands, run: Query<Entity, With<RunEntity>>) {
    for e in &run {
        commands.entity(e).despawn();
    }
}

fn close_stage(mut commands: Commands, q: Query<Entity, With<SelectEntity>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

/// Put the atlas / tints / face on a lineup character and start it idling.
///
/// The same two dressing routes as `player::on_model_ready`, minus the gait
/// machine: a lineup character only ever idles.
fn dress_for_the_lineup(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    dressing: Query<&Dressing>,
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
    let Ok(Dressing { pick, gltf }) = dressing.get(root) else {
        return;
    };
    let pick = *pick;

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
            if name.as_str() == "Head" {
                head = Some(e);
            }
            if pick.hides(name.as_str()) {
                commands.entity(e).insert(Visibility::Hidden);
            }
        }
    }
    if let (Some(head), Some(face)) = (head, pick.face()) {
        commands.spawn((
            ChildOf(head),
            Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
            MeshMaterial3d(materials.add(face_material(assets.load(face)))),
            face_in_head(),
        ));
    }

    let Some(gltf) = gltfs.get(gltf) else {
        return;
    };
    let Some(anim_entity) = children
        .iter_descendants(root)
        .find(|e| players.contains(*e))
    else {
        return;
    };
    // Empty-handed idle, not the armed one: the armed pose holds a weapon that
    // isn't there, which on the blocky rig is both arms straight out in front.
    let Some(clip) = gltf.named_animations.get(pick.clip(Gait::Idle, false)) else {
        return;
    };
    let (graph, idx) = AnimationGraph::from_clips([clip.clone()]);
    let mut anim = players.get_mut(anim_entity).unwrap();
    let mut transitions = AnimationTransitions::new();
    // Desync the lineup so twelve characters don't breathe in lockstep.
    transitions
        .play(&mut anim, idx[0], Duration::ZERO)
        .repeat()
        .seek_to((root.to_bits() % 97) as f32 * 0.01);
    commands
        .entity(anim_entity)
        .insert((AnimationGraphHandle(graphs.add(graph)), transitions));
}

// ---------------------------------------------------------------------------
// Choosing
// ---------------------------------------------------------------------------

fn choose(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    party: Res<Party>,
    autoplay: Option<Res<super::AutoPlay>>,
    browse: Option<Res<BrowseCast>>,
    mut choice: ResMut<Choice>,
    mut roster: ResMut<Roster>,
    mut next: ResMut<NextState<GameState>>,
    mut dwell: Local<f32>,
) {
    // The capture bot has no hands: it takes the roster as it stands.
    if autoplay.is_some() {
        next.set(GameState::Playing);
        return;
    }

    let cast = Pick::playable();

    // Recording the lineup: walk it, don't wait for a key that isn't coming.
    if browse.is_some() {
        *dwell += time.delta_secs();
        if *dwell > 0.9 {
            *dwell = 0.0;
            choice.cursor = (choice.cursor + 1) % cast.len();
        }
        roster.set(choice.slot, cast[choice.cursor]);
        return;
    }

    let pad = pads.iter().next();
    let pressed = |k: [KeyCode; 2], b: GamepadButton| {
        keys.any_just_pressed(k) || pad.is_some_and(|p| p.just_pressed(b))
    };

    let mut moved = 0i32;
    if pressed([KeyCode::ArrowLeft, KeyCode::KeyA], GamepadButton::DPadLeft) {
        moved -= 1;
    }
    if pressed(
        [KeyCode::ArrowRight, KeyCode::KeyD],
        GamepadButton::DPadRight,
    ) {
        moved += 1;
    }
    if moved != 0 {
        let n = cast.len() as i32;
        choice.cursor = ((choice.cursor as i32 + moved).rem_euclid(n)) as usize;
    }

    // Two-player: Tab hands the cursor to the drummer, and confirming with the
    // drummer still selected starts the run.
    if *party == Party::Duo && keys.just_pressed(KeyCode::Tab) {
        choice.slot = match choice.slot {
            Hero::Guitarist => Hero::Drummer,
            Hero::Drummer => Hero::Guitarist,
        };
        choice.cursor = cast
            .iter()
            .position(|p| *p == roster.of(choice.slot))
            .unwrap_or(choice.cursor);
    }

    // Keep the roster live as the cursor moves, so the panel below the lineup
    // is always describing the character you would actually get.
    roster.set(choice.slot, cast[choice.cursor]);

    if pressed([KeyCode::Enter, KeyCode::Space], GamepadButton::South) {
        if *party == Party::Duo && choice.slot == Hero::Guitarist {
            choice.slot = Hero::Drummer;
            choice.cursor = cast
                .iter()
                .position(|p| *p == roster.drummer)
                .unwrap_or(choice.cursor);
        } else {
            next.set(GameState::Playing);
        }
    }
}

/// Slide the lineup so the cursor's character is centre stage, and swell them.
#[allow(clippy::type_complexity)]
fn lay_out_lineup(
    time: Res<Time>,
    mut choice: ResMut<Choice>,
    mut slots: Query<(&LineupSlot, &mut Transform, &Children)>,
    mut waves: Query<&mut Visibility, With<WavePreview>>,
) {
    let dt = time.delta_secs();
    let k = (1.0 - (-11.0 * dt).exp()).clamp(0.0, 1.0);
    // Take the shortest way round the carousel rather than unwinding the whole
    // row when the cursor wraps from the last character to the first.
    let n = slots.iter().len().max(1) as f32;
    let target = choice.cursor as f32;
    let mut delta = target - choice.slide;
    if delta > n * 0.5 {
        delta -= n;
    } else if delta < -n * 0.5 {
        delta += n;
    }
    choice.slide += delta * k;

    for (slot, mut t, kids) in &mut slots {
        // Each character's offset from centre, wrapped so the row is a ring.
        let mut off = slot.index as f32 - choice.slide;
        if off > n * 0.5 {
            off -= n;
        } else if off < -n * 0.5 {
            off += n;
        }
        let focus = (1.0 - off.abs()).max(0.0);
        t.translation = Vec3::new(off * SPACING, 0.0, -off.abs() * RECEDE);
        t.scale = Vec3::splat(1.0 + (FOCUS_SWELL - 1.0) * focus);
        // The one under the cursor turns slowly on the spot; the rest stay
        // square to the camera.
        t.rotation = Quat::from_rotation_y(time.elapsed_secs() * 0.6 * focus);

        for kid in kids.iter() {
            if let Ok(mut vis) = waves.get_mut(kid) {
                *vis = if focus > 0.6 {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
}

fn turn_waveforms(time: Res<Time>, mut waves: Query<(&WavePreview, &mut Mesh3d)>) {
    let t = time.elapsed_secs();
    for (wave, mut mesh) in &mut waves {
        let p = (-t * wave.hz).rem_euclid(1.0);
        let i = ((p * WAVE_PHASES as f32) as usize).min(WAVE_PHASES - 1);
        if mesh.0 != wave.phases[i] {
            mesh.0 = wave.phases[i].clone();
        }
    }
}

/// Point the camera at the middle of the lineup.
fn frame_camera(mut cam: Query<&mut Transform, With<Camera3d>>) {
    let Ok(mut cam) = cam.single_mut() else {
        return;
    };
    *cam = Transform::from_xyz(0.0, 2.9, 7.4).looking_at(Vec3::new(0.0, 1.35, 0.0), Vec3::Y);
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

#[derive(Component)]
struct NameText;
#[derive(Component)]
struct GenreText;
#[derive(Component)]
struct SoundText;
#[derive(Component)]
struct SlotText;

fn spawn_ui(mut commands: Commands, assets: Res<AssetServer>, party: Res<Party>) {
    // The default Bevy font has no `\u{b7}` and no arrows — this screen was
    // drawing them as empty boxes. See `ui::font`.
    let font = super::ui::font(&assets);
    let px = bevy::ui::Val::Px;
    commands
        .spawn((
            SelectEntity,
            Node {
                position_type: PositionType::Absolute,
                top: px(0.0),
                left: px(0.0),
                width: bevy::ui::Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(4.0),
                padding: bevy::ui::UiRect::top(px(26.0)),
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                Text::new("GET THE BAND BACK TOGETHER"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(30.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.42)),
            ));
            c.spawn((
                SlotText,
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.62, 0.78, 0.92)),
            ));
        });

    commands
        .spawn((
            SelectEntity,
            Node {
                position_type: PositionType::Absolute,
                bottom: px(0.0),
                left: px(0.0),
                width: bevy::ui::Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6.0),
                padding: bevy::ui::UiRect::bottom(px(26.0)),
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                NameText,
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(34.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            c.spawn((
                GenreText,
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(Color::srgb(0.80, 0.82, 0.88)),
            ));
            c.spawn((
                SoundText,
                Text::new(""),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(15.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.78, 0.34)),
            ));
            c.spawn((
                Text::new(if *party == Party::Duo {
                    "\u{2190} \u{2192} choose    Tab other player    Enter confirm    C credits"
                } else {
                    "\u{2190} \u{2192} choose    Enter start    C credits"
                }),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(14.0),
                    ..default()
                },
                TextColor(Color::srgb(0.58, 0.62, 0.70)),
            ));
        });
}

#[allow(clippy::type_complexity)]
fn update_ui(
    choice: Res<Choice>,
    party: Res<Party>,
    roster: Res<Roster>,
    mut texts: ParamSet<(
        Query<&mut Text, With<NameText>>,
        Query<&mut Text, With<GenreText>>,
        Query<&mut Text, With<SoundText>>,
        Query<&mut Text, With<SlotText>>,
    )>,
    mut colors: Query<&mut TextColor, With<SoundText>>,
) {
    let cast = Pick::playable();
    let Some(pick) = cast.get(choice.cursor).copied() else {
        return;
    };
    let sound = pick.sound();

    if let Ok(mut t) = texts.p0().single_mut() {
        **t = pick.name().to_string();
    }
    if let Ok(mut t) = texts.p1().single_mut() {
        **t = format!("{}  \u{00b7}  {} cast", pick.genre(), pick.line());
    }
    if let Ok(mut t) = texts.p2().single_mut() {
        // The shot description is the weapon description: waveform is timbre,
        // cycles is pitch, amplitude is how wide it swings.
        **t = format!(
            "shots: {} wave  \u{00b7}  {:.1} cycles  \u{00b7}  swing {:.2}",
            sound.wave.name(),
            sound.cycles,
            sound.amp
        );
    }
    if let Ok(mut c) = colors.single_mut() {
        c.0 = sound.color;
    }
    if let Ok(mut t) = texts.p3().single_mut() {
        **t = if *party == Party::Duo {
            format!(
                "P1 {}   \u{00b7}   P2 {}   \u{2014} choosing for {}",
                roster.guitarist.name(),
                roster.drummer.name(),
                choice.slot.name()
            )
        } else {
            String::new()
        };
    }
    let _ = skins::SKINS;
}
