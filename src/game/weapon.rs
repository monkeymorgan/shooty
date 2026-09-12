use bevy::math::primitives::{Extrusion, Triangle2d};
use bevy::prelude::*;
use rand::Rng;

use std::collections::HashMap;

use super::audio::AudioCue;
use super::pickup::{DualWield, RapidFire};
use super::player::{Aim, Dodge, Intent, Muzzle};
use super::skins::{self, Sound, Waveform};
use super::{Bullet, GameState, Hero, Hitbox, Lifetime, Player, RunEntity};

const FIRE_RATE: f32 = 0.16;
const BULLET_SPEED: f32 = 40.0;
const BULLET_DAMAGE: f32 = 20.0;
/// Fallback origin height if no muzzle is found (should not happen in practice).
const MUZZLE_Y: f32 = 2.0;

#[derive(Resource)]
pub struct FireCooldown(Timer);

impl Default for FireCooldown {
    fn default() -> Self {
        Self(Timer::from_seconds(FIRE_RATE, TimerMode::Once))
    }
}

/// A tumbling projectile.
///
/// Only [`ShotLook::Pick`] uses this now: a shot that is *made of sound* has an
/// orientation that means something (the bar lies flat, the note stands up), and
/// tumbling it destroys the read.
#[derive(Component)]
pub struct Spin(Vec3);

/// What the guitarist's shots look like.
///
/// The original plectrum span end over end and read as a slice of pizza rather
/// than a guitar pick, so the alternatives here all take the other route: the
/// shot is the **music**, not the thing that plucked it. Selectable at runtime
/// so the call can be made from the game rather than from a still.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ShotLook {
    /// **A sound wave.** A run of chunky cubes strung along the flight line,
    /// swung off it by the shooter's own waveform (see [`skins::Sound`]) — and
    /// the waveform *travels through the shot while it flies*, so what you see
    /// is an oscillation rather than a fixed squiggle.
    ///
    /// This is the one look that says "music" without needing a symbol for it:
    /// an EQ bar or a quaver is a *picture of* music, whereas a wave is the
    /// shape sound actually has. It also carries character — the waveform,
    /// pitch, amplitude and colour all come from who fired it, so a metal
    /// bassist's slow wide saw and a J-Pop idol's tight bright sine are
    /// visibly different weapons before a note has been recorded.
    #[default]
    Wave,
    /// One bar of a graphic EQ — a wide flat slab flying broadside, lying in
    /// the ground plane so the top-down camera sees its full width.
    Bar,
    /// Three stacked bars of falling width: a slice of a spectrogram. Reads as
    /// a chord where [`ShotLook::Bar`] reads as a note.
    Chord,
    /// A chunky quaver — notehead, stem and flag. The most literal "music"
    /// read, and the one most at risk of turning to mush at speed.
    Note,
    /// A flat expanding-looking ring: the drum's beat rather than a pitch.
    Pulse,
    /// The original spinning plectrum, kept so the change can be judged
    /// against what it replaced.
    Pick,
}

impl ShotLook {
    pub const ALL: [ShotLook; 6] = [
        ShotLook::Wave,
        ShotLook::Bar,
        ShotLook::Chord,
        ShotLook::Note,
        ShotLook::Pulse,
        ShotLook::Pick,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ShotLook::Wave => "sound wave",
            ShotLook::Bar => "EQ bar",
            ShotLook::Chord => "chord stack",
            ShotLook::Note => "quaver",
            ShotLook::Pulse => "beat ring",
            ShotLook::Pick => "pick (old)",
        }
    }

    /// Body colour and emissive punch. Warm gold for the pitched shapes, a
    /// cooler cyan for the drum pulse so the two instruments read apart.
    fn colours(self) -> (Color, LinearRgba) {
        match self {
            ShotLook::Pulse => (Color::srgb(0.55, 0.95, 1.0), LinearRgba::rgb(0.5, 2.6, 3.2)),
            // The wave's own colour comes from whoever fired it
            // (`skins::sound`); this is only the fallback tint.
            _ => (
                Color::srgb(1.0, 0.82, 0.28),
                LinearRgba::rgb(1.6, 1.0, 0.15),
            ),
        }
    }

    /// Does this shape mean anything when it tumbles?
    fn tumbles(self) -> bool {
        self == ShotLook::Pick
    }
}

/// Build the mesh for one look. Shots travel down **+Z**, so anything with a
/// broadside (the bar, the chord) is widest across X.
pub fn shot_mesh(look: ShotLook, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    match look {
        // A still of the house waveform. In play the mesh is swapped every
        // frame for the next phase (`oscillate_waves`); this is the one a
        // preview or a fallback gets.
        ShotLook::Wave => wave_mesh(skins::sound(""), 0.0, meshes),
        ShotLook::Bar => meshes.add(Cuboid::new(0.95, 0.11, 0.3)),
        // Built as one mesh rather than three entities: a bullet is spawned
        // several times a second and its children would double the entity
        // churn for no visual gain.
        ShotLook::Chord => {
            let mut m: Mesh = Cuboid::new(0.95, 0.1, 0.3).into();
            for (w, y) in [(0.66_f32, 0.17_f32), (0.4, 0.34)] {
                let mut bar: Mesh = Cuboid::new(w, 0.1, 0.3).into();
                bar = bar.translated_by(Vec3::new(0.0, y, 0.0));
                m.merge(&bar).ok();
            }
            meshes.add(m)
        }
        ShotLook::Note => {
            let mut m: Mesh = Cuboid::new(0.38, 0.28, 0.24).into();
            let stem: Mesh = Cuboid::new(0.09, 0.6, 0.12)
                .mesh()
                .build()
                .translated_by(Vec3::new(0.15, 0.42, 0.0));
            let flag: Mesh = Cuboid::new(0.26, 0.12, 0.12)
                .mesh()
                .build()
                .translated_by(Vec3::new(0.31, 0.66, 0.0));
            m.merge(&stem).ok();
            m.merge(&flag).ok();
            meshes.add(m)
        }
        ShotLook::Pulse => meshes.add(Torus::new(0.3, 0.42).mesh().major_resolution(14).build()),
        ShotLook::Pick => meshes.add(Extrusion::new(
            Triangle2d::new(
                Vec2::new(0.0, 0.42),
                Vec2::new(-0.34, -0.26),
                Vec2::new(0.34, -0.26),
            ),
            0.12,
        )),
    }
}

pub fn shot_material(
    look: ShotLook,
    materials: &mut Assets<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let (base_color, emissive) = look.colours();
    materials.add(StandardMaterial {
        base_color,
        emissive,
        ..default()
    })
}

// ---------------------------------------------------------------------------
// The sound wave
// ---------------------------------------------------------------------------

/// How many phases of a waveform get baked as meshes.
///
/// The oscillation is done by **swapping between pre-built meshes**, not by
/// rewriting vertices: a shot is spawned several times a second and lives just
/// over a second, so there can be forty of them in the air, and rebuilding a
/// mesh per bullet per frame would be forty mesh uploads a frame for a thing
/// the size of a matchbox. Sixteen phases is enough that a 6 Hz wave looks
/// continuous and cheap enough to bake the moment a character first fires.
const WAVE_PHASES: usize = 16;
/// How long a shot is along its flight line.
const WAVE_LEN: f32 = 1.9;
/// How many points the waveform is sampled at. One box is built **between**
/// consecutive samples, so this is segments + 1.
const WAVE_SEGS: usize = 17;
/// Thickness of the trace.
const WAVE_THICK: f32 = 0.17;

/// One phase of a character's waveform, drawn as a chain of boxes.
///
/// Each box spans from one sample to the next rather than sitting *on* a
/// sample, which is the difference between a waveform and a dotted line: a
/// square wave's jumps become the vertical risers you would draw by hand, and
/// nothing ever has a gap in it however hard the wave turns.
///
/// Boxes, not an extruded ribbon, because the whole game is built out of boxes
/// (ART.md "The look") — a smooth swept tube would be the one thing on screen
/// that isn't.
pub fn wave_mesh(sound: Sound, phase: f32, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    // Sample the wave into the ground plane: it swings across X and travels
    // down +Z, so the top-down camera sees the swing at full width while the
    // flight axis is the foreshortened one.
    let at = |i: usize| {
        let f = i as f32 / (WAVE_SEGS - 1) as f32;
        Vec3::new(
            sound.amp * sound.wave.sample(f * sound.cycles + phase),
            0.0,
            (f - 0.5) * WAVE_LEN,
        )
    };

    let mut acc: Option<Mesh> = None;
    for i in 0..WAVE_SEGS - 1 {
        let (a, b) = (at(i), at(i + 1));
        let span = b - a;
        let len = span.length();
        if len < 1e-4 {
            continue;
        }
        // Overlap each joint by the trace thickness so corners stay solid.
        let box_mesh = Cuboid::new(WAVE_THICK, WAVE_THICK, len + WAVE_THICK)
            .mesh()
            .build()
            .rotated_by(Quat::from_rotation_y(f32::atan2(span.x, span.z)))
            .translated_by(a + span * 0.5);
        match &mut acc {
            Some(m) => {
                m.merge(&box_mesh).ok();
            }
            None => acc = Some(box_mesh),
        }
    }

    // A blunt head on the leading end, so the shot has a direction and lands
    // with something rather than tapering into nothing.
    let head = Cuboid::from_length(WAVE_THICK * 1.9)
        .mesh()
        .build()
        .translated_by(at(WAVE_SEGS - 1));
    match &mut acc {
        Some(m) => {
            m.merge(&head).ok();
        }
        None => acc = Some(head),
    }

    meshes.add(acc.expect("WAVE_SEGS > 1"))
}

/// A [`Sound`] reduced to something hashable, so two characters with the same
/// signal share one set of baked phases.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundKey(u8, u16, u16);

fn sound_key(s: Sound) -> SoundKey {
    SoundKey(
        Waveform::ALL.iter().position(|w| *w == s.wave).unwrap_or(0) as u8,
        (s.cycles * 100.0) as u16,
        (s.amp * 100.0) as u16,
    )
}

/// Baked waveform phases and glow materials, one set per distinct [`Sound`].
#[derive(Resource, Default)]
struct WaveAssets {
    phases: HashMap<SoundKey, Vec<Handle<Mesh>>>,
    materials: HashMap<SoundKey, Handle<StandardMaterial>>,
}

impl WaveAssets {
    /// Bake `sound` if it hasn't been seen before, and hand back what a shot
    /// of it needs.
    fn ensure(
        &mut self,
        sound: Sound,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> (SoundKey, Handle<StandardMaterial>) {
        let key = sound_key(sound);
        self.phases.entry(key).or_insert_with(|| {
            (0..WAVE_PHASES)
                .map(|i| wave_mesh(sound, i as f32 / WAVE_PHASES as f32, meshes))
                .collect()
        });
        let mat = self
            .materials
            .entry(key)
            .or_insert_with(|| {
                let l = sound.color.to_linear();
                materials.add(StandardMaterial {
                    base_color: sound.color,
                    emissive: LinearRgba::rgb(l.red * 3.4, l.green * 3.4, l.blue * 3.4),
                    ..default()
                })
            })
            .clone();
        (key, mat)
    }
}

/// On a [`ShotLook::Wave`] shot: which baked phase set it draws from, how fast
/// the wave runs through it, and where in the cycle it started.
#[derive(Component)]
struct Wave {
    key: SoundKey,
    hz: f32,
    phase0: f32,
}

/// Run each shot's waveform forward. The phase *decreases* so the pattern
/// travels out along the flight line rather than back down it.
fn oscillate_waves(
    time: Res<Time>,
    assets: Res<WaveAssets>,
    mut shots: Query<(&Wave, &mut Mesh3d)>,
) {
    let t = time.elapsed_secs();
    for (wave, mut mesh) in &mut shots {
        let Some(phases) = assets.phases.get(&wave.key) else {
            continue;
        };
        let p = (wave.phase0 - t * wave.hz).rem_euclid(1.0);
        let i = ((p * WAVE_PHASES as f32) as usize).min(WAVE_PHASES - 1);
        if mesh.0 != phases[i] {
            mesh.0 = phases[i].clone();
        }
    }
}

#[derive(Resource)]
struct PickAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    flash: Handle<Mesh>,
    flash_mat: Handle<StandardMaterial>,
}

pub struct WeaponPlugin;

impl Plugin for WeaponPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FireCooldown>()
            .init_resource::<ShotLook>()
            .init_resource::<WaveAssets>()
            .add_systems(Startup, load_pick_assets)
            // The oscillation is cosmetic, so it runs on a client too — a net
            // ghost's shot should wobble like the host's.
            .add_systems(Update, oscillate_waves.run_if(in_state(GameState::Playing)))
            .add_systems(
                Update,
                (fire, move_bullets, spin_projectiles)
                    .run_if(in_state(GameState::Playing))
                    .run_if(super::net::authoritative),
            );
    }
}

fn load_pick_assets(
    mut commands: Commands,
    look: Res<ShotLook>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(PickAssets {
        mesh: shot_mesh(*look, &mut meshes),
        material: shot_material(*look, &mut materials),
        flash: meshes.add(Sphere::new(0.35)),
        flash_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.9, 0.6),
            emissive: LinearRgba::rgb(6.0, 4.2, 1.4),
            unlit: true,
            ..default()
        }),
    });
}

#[allow(clippy::type_complexity)]
fn fire(
    mut commands: Commands,
    time: Res<Time>,
    mut cooldown: ResMut<FireCooldown>,
    mut shot: Local<usize>,
    look: Res<ShotLook>,
    shot_assets: Res<PickAssets>,
    mut wave_assets: ResMut<WaveAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cues: MessageWriter<AudioCue>,
    player: Query<
        (
            &Aim,
            &Dodge,
            &Intent,
            &Hero,
            &super::roster::Pick,
            Option<&DualWield>,
            Option<&RapidFire>,
        ),
        With<Player>,
    >,
    muzzles: Query<(&GlobalTransform, &ChildOf), With<Muzzle>>,
    guns: Query<&super::player::GuitarGun>,
) {
    cooldown.0.tick(time.delta());
    let Some((aim, dodge, intent, _, pick, dual, rapid)) =
        player.iter().find(|t| *t.3 == Hero::Guitarist)
    else {
        return;
    };
    let firing = intent.fire;
    // Dodge-rolling is still the one thing that stops a shot; the car used
    // to as well, back when the car was the only weapon in the driving
    // seat — now you can shoot from it too.
    if dodge.is_rolling() || !firing || !cooldown.0.is_finished() {
        return;
    }
    // Synth "Arpeggio" pickup: crank the fire rate for its duration.
    let rate = if rapid.is_some() {
        FIRE_RATE * 0.38
    } else {
        FIRE_RATE
    };
    cooldown
        .0
        .set_duration(std::time::Duration::from_secs_f32(rate));
    cooldown.0.reset();

    let dir = aim.0.normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }

    // Which muzzles fire this shot: both hands when dual-wielding, else the
    // right hand only. Muzzles are parented to a gun entity that knows its side.
    let dual = dual.is_some();
    let mut origins: Vec<Vec3> = muzzles
        .iter()
        .filter(|(_, parent)| dual || guns.get(parent.parent()).map(|g| !g.left).unwrap_or(true))
        .map(|(gt, _)| gt.translation())
        .collect();
    if origins.is_empty() {
        // Model not loaded yet — fall back to a point in front of the player.
        origins.push(Vec3::new(dir.x, MUZZLE_Y, dir.z));
    }

    // A wave shot is made of the shooter's own signal, so its mesh set and
    // colour are looked up (and baked, once) per character rather than shared.
    let sound = pick.sound();
    let wave =
        (*look == ShotLook::Wave).then(|| wave_assets.ensure(sound, &mut meshes, &mut materials));

    let mut rng = rand::thread_rng();
    for origin in origins {
        *shot += 1;
        cues.write(AudioCue::Shoot);
        let (mesh, material) = match &wave {
            Some((key, mat)) => (wave_assets.phases[key][0].clone(), mat.clone()),
            None => (shot_assets.mesh.clone(), shot_assets.material.clone()),
        };
        let bullet = commands
            .spawn((
                Bullet {
                    velocity: dir * BULLET_SPEED,
                    damage: BULLET_DAMAGE,
                },
                Hitbox(0.55),
                Lifetime(Timer::from_seconds(1.3, TimerMode::Once)),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                // Face the shot down its own travel. A sound-shaped projectile has
                // a meaningful orientation — the bar lies flat across the flight,
                // the quaver stands upright — so only the pick is allowed to tumble.
                Transform::from_translation(origin)
                    .with_rotation(Quat::from_rotation_y(f32::atan2(dir.x, dir.z))),
                RunEntity,
            ))
            .id();
        if let Some((key, _)) = wave {
            commands.entity(bullet).insert(Wave {
                key,
                hz: sound.hz,
                // Stagger the start so consecutive shots aren't one long
                // continuous wave train.
                phase0: (*shot as f32 * 0.37) % 1.0,
            });
        }
        if look.tumbles() {
            commands.entity(bullet).insert(Spin(
                Vec3::new(
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                    rng.gen_range(-1.0..1.0),
                )
                .normalize_or_zero()
                    * rng.gen_range(18.0..28.0),
            ));
        }
        // Muzzle flash.
        commands.spawn((
            Mesh3d(shot_assets.flash.clone()),
            MeshMaterial3d(shot_assets.flash_mat.clone()),
            Transform::from_translation(origin).with_scale(Vec3::splat(0.9)),
            Lifetime(Timer::from_seconds(0.05, TimerMode::Once)),
            RunEntity,
        ));
    }
}

fn move_bullets(time: Res<Time>, mut bullets: Query<(&Bullet, &mut Transform)>) {
    let dt = time.delta_secs();
    for (bullet, mut transform) in &mut bullets {
        transform.translation += bullet.velocity * dt;
    }
}

fn spin_projectiles(time: Res<Time>, mut q: Query<(&Spin, &mut Transform)>) {
    let dt = time.delta_secs();
    for (spin, mut t) in &mut q {
        if spin.0 != Vec3::ZERO {
            t.rotate(Quat::from_scaled_axis(spin.0 * dt));
        }
    }
}
