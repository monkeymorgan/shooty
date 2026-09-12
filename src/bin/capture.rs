//! Offscreen capture binary — renders the real game to an image target with a
//! scripted bot driving the player, and saves PNG frames for a proof video.
//!
//!   cargo run --bin capture -- frames screenshots/run 450
//!   cargo run --bin capture -- select screenshots/select 60   # the lineup
//!   cargo run --bin capture -- gloom  screenshots/gloom  600  # the stem loop

use std::time::Duration;

use bevy::{
    app::{AppExit, ScheduleRunnerPlugin},
    camera::RenderTarget,
    prelude::*,
    render::{
        render_resource::{TextureFormat, TextureUsages},
        view::screenshot::{Screenshot, ScreenshotCaptured, save_to_disk},
    },
    time::TimeUpdateStrategy,
    window::ExitCondition,
    winit::WinitPlugin,
};
use shooty::game::gloom::{GloomSource, Mood};
use shooty::{GamePlugin, GameState, game::AutoPlay};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;
const PRE_ROLL: u32 = 160;

#[derive(Resource)]
struct Capture {
    out_dir: String,
    frame: u32,
    target: u32,
    saved: u32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `select` parks on the character-select lineup instead of playing a run,
    // so the screen can be reviewed the same way everything else here is.
    let on_select = args.get(1).is_some_and(|m| m == "select");
    // `gloom` plays a normal run but points the camera at the *mechanic* rather
    // than at the hero: whichever of the five is currently pouring bad vibes,
    // and then whoever is walking to the bandstand having been cheered up. The
    // bot fights wherever the swarm chases it to, which is the wrong place to
    // be standing if what you want to see is where the swarm came from.
    let on_gloom = args.get(1).is_some_and(|m| m == "gloom");
    // `map` looks straight down from high above and slowly orbits, so a whole
    // level can be reviewed as a level — districts, road network, where the
    // empty ground is. The play camera sits 20 units up and frames two people;
    // it is the wrong instrument entirely for judging a 470 m world.
    let on_map = args.get(1).is_some_and(|m| m == "map");
    // `fly` scripts a take-off: walk the guitarist to the aircraft, climb in,
    // throttle up, rotate, and circle. The play bot has no idea aircraft exist,
    // so without this there is no way to see whether flight actually works.
    let on_fly = args.get(1).is_some_and(|m| m == "fly");
    // `credits` parks on the select screen and opens the credits overlay, so
    // the attribution can be reviewed the same way everything else here is —
    // in particular whether the font actually draws the diacritics in
    // "Vojtěch Balák", which a licence depends on.
    let on_credits = args.get(1).is_some_and(|m| m == "credits");
    let out_dir = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "screenshots/run".into());
    let target: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(450);
    // Optional 4th arg: which candidate projectile to fire, so the shot-shape
    // call can be made from footage rather than from a still of one sitting on
    // the ground. Matched on the display name, e.g. "bar", "quaver", "ring".
    let look = args.get(4).and_then(|want| {
        let want = want.to_ascii_lowercase();
        shooty::game::weapon::ShotLook::ALL
            .into_iter()
            .find(|l| l.name().to_ascii_lowercase().contains(&want))
    });
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let mut app = App::new();
    shooty::insert_scene_resources(&mut app);
    if let Some(look) = look {
        println!("[capture] shots: {}", look.name());
        app.insert_resource(look);
    }
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1.0 / 30.0,
    )))
    .insert_resource(Capture {
        out_dir,
        frame: 0,
        target,
        saved: 0,
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
        1.0 / 90.0,
    )))
    // Straight into a run by default: the bot has no hands for the lineup, and
    // the roster's default is the one the proof video should show.
    .insert_state(if on_select || on_credits {
        GameState::Select
    } else {
        GameState::Playing
    })
    .add_plugins(GamePlugin)
    .add_systems(Startup, setup_target)
    .add_systems(
        Update,
        (pace, cycle_cam, auto_restart, grab_frames, finish).chain(),
    );
    if on_gloom {
        // Hold the shipping framing (no rig cycling) and re-aim it in
        // `PostUpdate`, after `GamePlugin`'s own `drive_camera` has had its say.
        app.insert_resource(GloomCam)
            .add_systems(PostUpdate, gloom_cam);
    }
    if on_map {
        app.insert_resource(MapCam).add_systems(PostUpdate, map_cam);
    }
    if on_credits {
        // In `PreUpdate`, *after* the input system clears last frame's presses
        // — press it in `Update` and `just_pressed` may already have been
        // wiped before the credits toggle runs.
        app.add_systems(PreUpdate, open_credits.after(bevy::input::InputSystems));
    }
    if on_fly {
        app.insert_resource(FlyDemo::default())
            .add_systems(Update, fly_demo)
            .add_systems(PostUpdate, fly_cam);
    }
    if on_select || on_credits {
        app.insert_resource(shooty::game::select::BrowseCast);
    } else {
        app.insert_resource(AutoPlay);
    }
    app.run();
}

#[derive(Resource)]
struct TargetImage(Handle<Image>);

fn setup_target(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_target_texture(WIDTH, HEIGHT, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let handle = images.add(image);

    commands.spawn((
        shooty::camera_bundle(),
        RenderTarget::Image(handle.clone().into()),
    ));
    shooty::spawn_key_light(&mut commands);
    commands.insert_resource(TargetImage(handle));
}

/// Freeze the simulation until the pre-roll is over, so glTF models have time
/// to load without the spawner racing ahead.
fn pace(cap: Res<Capture>, mut strat: ResMut<TimeUpdateStrategy>) {
    *strat = if cap.frame < PRE_ROLL {
        TimeUpdateStrategy::ManualDuration(Duration::ZERO)
    } else {
        TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 30.0))
    };
}

fn grab_frames(mut commands: Commands, target: Res<TargetImage>, mut cap: ResMut<Capture>) {
    cap.frame += 1;
    if cap.frame <= PRE_ROLL || cap.saved >= cap.target {
        return;
    }
    let n = cap.frame - PRE_ROLL;
    if n > cap.target {
        return;
    }
    let path = format!("{}/frame{:05}.png", cap.out_dir, n);
    commands
        .spawn(Screenshot::image(target.0.clone()))
        .observe(save_to_disk(path))
        .observe(|_: On<ScreenshotCaptured>, mut cap: ResMut<Capture>| {
            cap.saved += 1;
        });
}

/// Rotate through the experimental camera rigs so the proof video shows all
/// three: a stretch of the shipping Follow cam, then Shoulder, then FirstPerson.
fn cycle_cam(
    time: Res<Time>,
    gloom: Option<Res<GloomCam>>,
    map: Option<Res<MapCam>>,
    fly: Option<Res<FlyDemo>>,
    mut mode: ResMut<shooty::game::CameraMode>,
    mut elapsed: Local<f32>,
) {
    if gloom.is_some() || map.is_some() || fly.is_some() {
        return;
    }
    *elapsed += time.delta_secs();
    let want = match ((*elapsed / 6.0) as u32) % 3 {
        1 => shooty::game::CameraMode::Shoulder,
        2 => shooty::game::CameraMode::FirstPerson,
        _ => shooty::game::CameraMode::Follow,
    };
    if *mode != want {
        *mode = want;
    }
}

/// Hold the win screen briefly, then kick off another loop so a long capture
/// keeps showing the game rather than a frozen banner.
fn auto_restart(
    time: Res<Time>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut held: Local<f32>,
) {
    if *state.get() == GameState::Victory {
        *held += time.delta_secs();
        if *held > 3.5 {
            *held = 0.0;
            next.set(GameState::Playing);
        }
    } else {
        *held = 0.0;
    }
}

fn finish(cap: Res<Capture>, mut exit: MessageWriter<AppExit>) {
    // Stop once every requested frame has been written, with a hard ceiling in
    // case a save is dropped.
    if cap.saved >= cap.target || cap.frame > PRE_ROLL + cap.target + 240 {
        exit.write(AppExit::Success);
    }
}

/// Present only in the `fly` capture mode: a scripted take-off.
#[derive(Resource, Default)]
struct FlyDemo {
    t: f32,
    boarded: bool,
}

/// Put the guitarist beside the aircraft, get in, and fly it. Drives `Intent`
/// directly — the same channel a keyboard writes to — so this exercises the
/// real flight code rather than a special path through it.
fn fly_demo(
    time: Res<Time>,
    mut demo: ResMut<FlyDemo>,
    mut heroes: Query<
        (
            &mut Transform,
            &mut shooty::game::player::Intent,
            Option<&shooty::game::fly::Flying>,
        ),
        (
            With<shooty::game::Player>,
            Without<shooty::game::fly::Plane>,
        ),
    >,
    craft: Query<&Transform, With<shooty::game::fly::Plane>>,
) {
    demo.t += time.delta_secs();
    let Ok(target) = craft.single().map(|t| t.translation) else {
        return;
    };
    for (mut transform, mut intent, flying) in &mut heroes {
        if flying.is_none() {
            if demo.boarded {
                continue;
            }
            // Stand next to it and press interact once it has settled.
            transform.translation = target + Vec3::new(4.0, 0.0, 0.0);
            if demo.t > 1.0 {
                intent.interact = true;
                demo.boarded = true;
            }
            continue;
        }
        intent.interact = false;
        // Full throttle, then rotate and hold a gentle left-hand circuit.
        intent.move_dir = Vec2::new(if demo.t > 7.0 { -0.75 } else { 0.0 }, 1.0);
        intent.climb = if demo.t > 5.0 { 1.0 } else { 0.0 };
    }
}

/// Press `C` once, a moment in, to raise the credits overlay.
fn open_credits(time: Res<Time>, mut keys: ResMut<ButtonInput<KeyCode>>, mut done: Local<bool>) {
    if !*done && time.elapsed_secs() > 0.6 {
        keys.press(KeyCode::KeyC);
        *done = true;
    } else {
        keys.release(KeyCode::KeyC);
    }
}

/// Chase the aircraft, so the take-off is actually in the footage. The play
/// camera frames the *party*, and the drummer stays on the apron, so without
/// this the shot is of a stationary hero while the plane leaves.
fn fly_cam(
    _demo: Res<FlyDemo>,
    time: Res<Time>,
    craft: Query<&Transform, (With<shooty::game::fly::Plane>, Without<Camera3d>)>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok(mut cam), Ok(craft)) = (cam.single_mut(), craft.single()) else {
        return;
    };
    let yaw = craft.rotation.to_euler(EulerRot::YXZ).0;
    let behind = Vec3::new(-yaw.sin(), 0.0, yaw.cos()) * 34.0;
    let goal = craft.translation + behind + Vec3::Y * 14.0;
    let k = (1.0 - (-3.0 * time.delta_secs()).exp()).clamp(0.0, 1.0);
    cam.translation = cam.translation.lerp(goal, k);
    cam.look_at(craft.translation + Vec3::Y * 2.0, Vec3::Y);
}

/// Present only in the `map` capture mode.
#[derive(Resource)]
struct MapCam;

/// Look down on the whole level from high up, drifting slowly so the shot is
/// not a still. The height is derived from the level's own arena, so this frames
/// any level without being told how big it is.
fn map_cam(
    _mode: Res<MapCam>,
    time: Res<Time>,
    level: Res<shooty::game::CurrentLevel>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
    mut fog: Query<&mut bevy::pbr::DistanceFog>,
) {
    let Ok(mut cam) = cam.single_mut() else {
        return;
    };
    let arena = level.arena;

    // From up here the level is further away than any playable fog setting
    // allows for, so push the fog past the far corner. Runs in `PostUpdate`,
    // after `grade::lift_the_gloom` has written its own value.
    if let Ok(mut fog) = fog.single_mut() {
        let reach = arena.length() * 4.0;
        fog.falloff = bevy::pbr::FogFalloff::Linear {
            start: reach * 0.75,
            end: reach,
        };
    }
    // Far enough back that the long axis fits the frame with room to spare.
    let height = arena.x.max(arena.y) * 1.9;
    let t = time.elapsed_secs() * 0.08;
    let drift = Vec2::new(t.cos(), t.sin()) * (arena * 0.18);
    let focus = shooty::game::ground(drift, 0.0);
    cam.translation = focus + Vec3::new(0.0, height, height * 0.55);
    cam.look_at(focus, Vec3::Y);
}

/// Present only in the `gloom` capture mode.
#[derive(Resource)]
struct GloomCam;

/// Frame the bad-vibe loop: the source currently venting, or — the moment
/// someone snaps out of it — the one walking to the bandstand, which is the
/// half of the mechanic a hero-follow camera never shows.
fn gloom_cam(
    _mode: Res<GloomCam>,
    time: Res<Time>,
    level: Res<shooty::game::CurrentLevel>,
    sources: Query<(&Transform, &GloomSource), Without<Camera3d>>,
    mut cam: Query<&mut Transform, With<Camera3d>>,
    mut hold: Local<f32>,
    mut last_cured: Local<usize>,
) {
    let Ok(mut cam) = cam.single_mut() else {
        return;
    };

    // Hold on the bandstand for a few seconds after each cure, so the payoff —
    // somebody in full colour arriving and nodding along — is actually in the
    // footage. Without this the camera snaps straight back to the next vent
    // and the half of the loop that is the *reward* never gets filmed.
    let cured = sources
        .iter()
        .filter(|(_, g)| g.mood != Mood::Dormant && g.mood != Mood::Brooding)
        .count();
    if cured > *last_cured {
        *last_cured = cured;
        *hold = 7.0;
    }
    *hold = (*hold - time.delta_secs()).max(0.0);

    let marching = sources
        .iter()
        .find(|(_, g)| g.mood == Mood::Marching)
        .map(|(t, _)| t.translation);
    let venting = sources
        .iter()
        .filter(|(_, g)| g.mood == Mood::Brooding)
        .max_by_key(|(_, g)| g.idx)
        .map(|(t, _)| t.translation);
    let stage = shooty::game::ground(level.stage_site, 0.0);
    let focus = if *hold > 0.0 {
        marching.unwrap_or(stage)
    } else {
        venting.or(marching).unwrap_or(stage)
    };

    // Closer than the play camera — these are person-sized beats.
    let offset = Vec3::new(0.0, 15.0, 13.0);
    let goal = focus + offset;
    let k = (1.0 - (-3.5 * time.delta_secs()).exp()).clamp(0.0, 1.0);
    cam.translation = cam.translation.lerp(goal, k);
    cam.look_at(focus + Vec3::Y * 1.2, Vec3::Y);
}
