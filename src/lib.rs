use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::light::CascadeShadowConfigBuilder;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;

pub mod game;

pub use game::{CAMERA_OFFSET, GamePlugin, GameState};

/// Initial camera pose (before `follow_camera` takes over), reused by the
/// capture binary. Sits `CAMERA_OFFSET` from the point it looks at.
pub const CAMERA_LOOK: Vec3 = Vec3::ZERO;
pub const CAMERA_POS: Vec3 = CAMERA_OFFSET;

/// Daytime sky — also the fog colour, so the far edge of the town fades into it.
pub const SKY: Color = Color::srgb(0.55, 0.71, 0.92);

/// The warm key sun (there's also a dim cool fill light). Tagged so the M5
/// world grade (`game::grade`) can lift it out of the gloom as the band
/// reclaims the town.
#[derive(Component)]
pub struct KeySun;

/// Scene resources shared by the windowed game and the offscreen capture.
pub fn insert_scene_resources(app: &mut App) {
    app.insert_resource(ClearColor(SKY))
        .insert_resource(bevy::light::GlobalAmbientLight {
            // Low, cool bounce — the key light does the shaping now, so the
            // town blocks and characters read with real contrast instead of
            // the old flat pastel wash.
            color: Color::srgb(0.72, 0.80, 1.0),
            brightness: 380.0,
            ..default()
        });
}

pub fn build_app(app: &mut App) {
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Shooty".into(),
                    resolution: (1280u32, 800u32).into(),
                    ..default()
                }),
                ..default()
            })
            // Crisp texel edges on the blocky skins + face-cube textures.
            .set(ImagePlugin::default_nearest()),
    );
    insert_scene_resources(app);
    app.init_state::<GameState>()
        .add_systems(Startup, spawn_camera_and_light)
        .add_plugins(game::GamePlugin);
}

/// The shared camera rig: HDR + filmic tonemapping + bloom so every emissive in
/// the game (hero ring, muzzle flash, pickup panels, the Encore wave) actually
/// glows, plus distance fog for depth across the bigger town map.
pub fn camera_bundle() -> impl Bundle {
    (
        Camera3d::default(),
        bevy::camera::Hdr,
        Tonemapping::TonyMcMapface,
        Bloom {
            intensity: 0.22,
            low_frequency_boost: 0.8,
            ..Bloom::NATURAL
        },
        DistanceFog {
            color: SKY,
            falloff: FogFalloff::Linear {
                start: 95.0,
                end: 210.0,
            },
            ..default()
        },
        Transform::from_translation(CAMERA_POS).looking_at(CAMERA_LOOK, Vec3::Y),
        bevy::ui::IsDefaultUiCamera,
    )
}

/// Warm key sun + a dim cool sky-fill so shadowed faces keep some colour.
pub fn spawn_key_light(commands: &mut Commands) {
    commands.spawn((
        KeySun,
        DirectionalLight {
            illuminance: 20_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(30.0, 44.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder {
            num_cascades: 4,
            first_cascade_far_bound: 22.0,
            maximum_distance: 170.0,
            ..default()
        }
        .build(),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 3_200.0,
            color: Color::srgb(0.55, 0.68, 1.0),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-24.0, 26.0, -16.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

pub fn spawn_camera_and_light(mut commands: Commands) {
    commands.spawn(camera_bundle());
    spawn_key_light(&mut commands);
}

pub fn run() {
    use game::{Party, net::Launch};

    let mut app = App::new();
    build_app(&mut app);
    match game::net::launch_from_args() {
        Launch::Solo => {
            app.insert_resource(Party::Solo);
        }
        Launch::Coop => {
            app.insert_resource(Party::Duo);
        }
        Launch::Host(port) => {
            app.insert_resource(Party::Duo)
                .add_plugins(game::net::NetPlugin::host(port));
        }
        Launch::Client(addr) => {
            app.insert_resource(Party::Duo)
                .add_plugins(game::net::NetPlugin::client(addr));
        }
    }
    app.run();
}
