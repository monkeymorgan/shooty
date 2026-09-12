//! **Spawning one thing out of an asset kit.** The three ways a level puts a
//! model on the ground, shared by every environment.
//!
//! Kenney's kits all arrive the same way — a glTF scene whose stock materials
//! render untextured white here — so all three of these spawn the scene under
//! a plain transform anchor and re-point its meshes at a material we control
//! once [`WorldInstanceReady`] fires. That indirection is the whole reason
//! these are functions rather than four lines at the call site, and it is why
//! adding a *new* kit to the game is a table of paths and a colormap handle
//! rather than any new code.

use bevy::image::{ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::world_serialization::{WorldAssetRoot, WorldInstanceReady};
use rand::Rng;

use crate::game::{Obstacle, RunEntity, ground};

/// A circle-collision reservation on the ground plane (centre, radius).
///
/// Levels keep a running list of these while they build so a scatter pass can
/// ask "is this ground already taken?" — it is how a house avoids landing on a
/// loudspeaker pitch, a gloom source, or another house.
pub type Solid = (Vec2, f32);

/// The two materials every nature model is routed to. Kenney's nature kit
/// bakes a minty `leafsGreen` and an orange `woodBark`, both `metallic: 1`,
/// which read wrong under our lighting — so [`nature`] sorts each stock
/// material into one of these by whether its base colour is green-dominant.
#[derive(Clone)]
pub struct NatureMats {
    pub foliage: Handle<StandardMaterial>,
    pub bark: Handle<StandardMaterial>,
}

/// Load a kit's `colormap.png` **point-sampled**. The colormaps are tiny atlases
/// of flat swatches; anything but nearest-neighbour bleeds one swatch into the
/// next and every model comes out muddy at the seams.
pub fn colormap(assets: &AssetServer, path: &'static str) -> Handle<Image> {
    assets
        .load_builder()
        .with_settings(|s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::nearest());
        })
        .load(path)
}

/// Spawn a colormap-kit building: an [`Obstacle`] anchor with the scaled scene
/// as a child, and an observer that re-points every mesh at `mat`. `lift` raises
/// the scene so its feet land on the ground — every kit measured so far stands
/// at y = 0, so it is 0 for all of them, but it stays a parameter because it is
/// the thing you reach for when a kit does not.
#[allow(clippy::too_many_arguments)]
pub fn building(
    commands: &mut Commands,
    assets: &AssetServer,
    model: &'static str,
    pos: Vec2,
    scale: f32,
    lift: f32,
    yaw: f32,
    radius: f32,
    mat: Handle<StandardMaterial>,
) {
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model));
    commands
        .spawn((
            Obstacle { radius },
            Transform::from_translation(ground(pos, 0.0)).with_rotation(Quat::from_rotation_y(yaw)),
            Visibility::default(),
            RunEntity,
        ))
        .with_children(|c| {
            c.spawn((
                WorldAssetRoot(scene),
                Transform::from_xyz(0.0, lift, 0.0).with_scale(Vec3::splat(scale)),
            ))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      meshed: Query<(), With<Mesh3d>>| {
                    for e in children.iter_descendants(ready.entity) {
                        if meshed.contains(e) {
                            commands.entity(e).insert(MeshMaterial3d(mat.clone()));
                        }
                    }
                },
            );
        });
}

/// Spawn a decorative kit prop (scene child scaled `scale`, every mesh recoloured
/// to `mat`). `obstacle` adds ground collision of that radius.
///
/// **`yaw` goes on the anchor, not on the mesh child.** It used to sit on the
/// child, which looks identical for anything that never moves — and was a real
/// bug for the one kind of prop that does. `drive::steer` reads and writes the
/// *anchor's* rotation and seats the driver off it, so a car spawned facing east
/// (yaw = π/2) drove north while pointing east: it appeared to swing around a
/// fulcrum somewhere off in the next street. Anything that will ever be moved,
/// aimed or sat in needs its facing on the entity the game code actually holds.
#[allow(clippy::too_many_arguments)]
pub fn prop(
    commands: &mut Commands,
    assets: &AssetServer,
    model: &'static str,
    pos: Vec2,
    y: f32,
    yaw: f32,
    scale: f32,
    mat: Handle<StandardMaterial>,
    obstacle: Option<f32>,
) -> Entity {
    prop_scaled(
        commands,
        assets,
        model,
        pos,
        y,
        yaw,
        Vec3::splat(scale),
        mat,
        obstacle,
    )
}

/// [`prop`], but scaled per axis — for the handful of models whose proportions
/// disagree with the real object's. See [`crate::game::scale::fit3`].
#[allow(clippy::too_many_arguments)]
pub fn prop_scaled(
    commands: &mut Commands,
    assets: &AssetServer,
    model: &'static str,
    pos: Vec2,
    y: f32,
    yaw: f32,
    scale: Vec3,
    mat: Handle<StandardMaterial>,
    obstacle: Option<f32>,
) -> Entity {
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(model));
    let mut e = commands.spawn((
        Transform::from_translation(ground(pos, y)).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RunEntity,
    ));
    if let Some(r) = obstacle {
        e.insert(Obstacle { radius: r });
    }
    e.with_children(|c| {
        c.spawn((WorldAssetRoot(scene), Transform::from_scale(scale)))
            .observe(
                move |ready: On<WorldInstanceReady>,
                      mut commands: Commands,
                      children: Query<&Children>,
                      meshed: Query<(), With<Mesh3d>>| {
                    for e in children.iter_descendants(ready.entity) {
                        if meshed.contains(e) {
                            commands.entity(e).insert(MeshMaterial3d(mat.clone()));
                        }
                    }
                },
            );
    });
    e.id()
}

/// Spawn a nature model — a tree, bush or rock — at a random yaw, hue-routing
/// its stock materials to [`NatureMats`].
///
/// `model` is (path, **the native dimension `metres` refers to**, trunk
/// collision radius at native scale). For a tree that dimension is its height;
/// for the flat, wide bushes and rocks it is their widest side, because fitting
/// those by height blows their footprint up several times over.
#[allow(clippy::too_many_arguments)]
pub fn nature(
    commands: &mut Commands,
    assets: &AssetServer,
    nature: &NatureMats,
    model: (&'static str, f32, f32),
    pos: Vec2,
    metres: f32,
    rng: &mut impl Rng,
    obstacle: bool,
) {
    let (path, native_h, trunk_r) = model;
    let scale = crate::game::scale::fit(native_h, metres);
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
    let yaw = rng.gen_range(0.0..std::f32::consts::TAU);
    let (foliage, bark) = (nature.foliage.clone(), nature.bark.clone());
    let mut e = commands.spawn((
        Transform::from_translation(ground(pos, 0.0)).with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RunEntity,
    ));
    if obstacle {
        e.insert(Obstacle {
            radius: (trunk_r * scale).max(0.7),
        });
    }
    e.with_children(|c| {
        c.spawn((
            WorldAssetRoot(scene),
            Transform::from_scale(Vec3::splat(scale)),
        ))
        .observe(
            move |ready: On<WorldInstanceReady>,
                  mut commands: Commands,
                  children: Query<&Children>,
                  mats: Query<&MeshMaterial3d<StandardMaterial>>,
                  assets: Res<Assets<StandardMaterial>>| {
                // Route each stock material to foliage or bark by whether its
                // base colour is green-dominant.
                for e in children.iter_descendants(ready.entity) {
                    let Ok(m) = mats.get(e) else { continue };
                    let leaf = assets.get(&m.0).is_some_and(|s| {
                        let c = s.base_color.to_linear();
                        c.green >= c.red && c.green >= c.blue
                    });
                    commands.entity(e).insert(MeshMaterial3d(if leaf {
                        foliage.clone()
                    } else {
                        bark.clone()
                    }));
                }
            },
        );
    });
}

/// Spawn a model **keeping its own materials**, feet on the ground.
///
/// The Kenney kits all arrive as untextured white and get re-pointed at a
/// colormap, which is what [`prop`] and [`building`] do. Models from elsewhere
/// — the CC-BY aircraft, say — arrive already painted, in several materials
/// each, and flattening them to one colour would throw away the model. So this
/// one leaves them alone.
///
/// `lift` is in **native units** and is the model's own lowest `y`: these do not
/// stand at y = 0 the way the kits do, so it is subtracted to put the feet (or
/// the wheels) on the ground.
#[allow(clippy::too_many_arguments)]
pub fn model(
    commands: &mut Commands,
    assets: &AssetServer,
    path: &'static str,
    pos: Vec2,
    altitude: f32,
    yaw: f32,
    scale: f32,
    lift: f32,
    obstacle: Option<f32>,
) -> Entity {
    let scene = assets.load(GltfAssetLabel::Scene(0).from_asset(path));
    let mut e = commands.spawn((
        Transform::from_translation(ground(pos, altitude))
            .with_rotation(Quat::from_rotation_y(yaw)),
        Visibility::default(),
        RunEntity,
    ));
    if let Some(r) = obstacle {
        e.insert(Obstacle { radius: r });
    }
    e.with_children(|c| {
        c.spawn((
            WorldAssetRoot(scene),
            Transform::from_xyz(0.0, -lift * scale, 0.0).with_scale(Vec3::splat(scale)),
        ));
    });
    e.id()
}
