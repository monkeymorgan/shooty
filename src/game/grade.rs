//! M5 — the town lifts out of the gloom as the band reclaims it.
//!
//! A music-starved town opens overcast, dim and cold. Every loudspeaker zone
//! (and, at the end, the stage) raises [`SecuredZones::music_level`] — the same
//! signal the music bed rides. [`WorldGrade`] eases toward it and drives the
//! sky, ambient, distance fog and the key sun, so the place visibly warms and
//! brightens as the coverage grows. Finishing the stage leaves it in full
//! daylight for the win screen.

use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;

use crate::KeySun;

use super::build::SecuredZones;
use super::{CurrentLevel, GameState};

/// How far the town has come out of the gloom, `0.0` (music-starved) to `1.0`
/// (stage is live). Eased toward the target so a finished loudspeaker fades the
/// light up over a second or so rather than snapping it.
#[derive(Resource, Default)]
pub struct WorldGrade(pub f32);

/// A grade endpoint — every knob the lift touches, at one end of the range.
pub struct Look {
    pub sky: Vec3,
    pub ambient: Vec3,
    pub ambient_brightness: f32,
    pub fog: Vec3,
    pub fog_start: f32,
    pub fog_end: f32,
    pub sun: Vec3,
    pub sun_lux: f32,
}

/// Music-starved: a cold, flat overcast that closes in around you.
pub const GLOOM: Look = Look {
    sky: Vec3::new(0.13, 0.15, 0.20),
    ambient: Vec3::new(0.46, 0.52, 0.68),
    ambient_brightness: 70.0,
    fog: Vec3::new(0.13, 0.15, 0.20),
    fog_start: 18.0,
    fog_end: 95.0,
    sun: Vec3::new(0.60, 0.68, 0.90),
    sun_lux: 4_200.0,
};

/// Stage is live: the warm daylight the scene was originally lit for.
pub const LIT: Look = Look {
    sky: Vec3::new(0.55, 0.71, 0.92),
    ambient: Vec3::new(0.72, 0.80, 1.0),
    ambient_brightness: 380.0,
    fog: Vec3::new(0.55, 0.71, 0.92),
    fog_start: 95.0,
    fog_end: 210.0,
    sun: Vec3::new(1.0, 0.96, 0.88),
    sun_lux: 20_000.0,
};

fn mix(a: Vec3, b: Vec3, t: f32) -> Color {
    let v = a.lerp(b, t);
    Color::srgb(v.x, v.y, v.z)
}

pub struct GradePlugin;

impl Plugin for GradePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldGrade>()
            .add_systems(OnEnter(GameState::Playing), |mut g: ResMut<WorldGrade>| {
                g.0 = 0.0;
            })
            .add_systems(Update, lift_the_gloom.run_if(in_state(GameState::Playing)));
    }
}

#[allow(clippy::too_many_arguments)]
fn lift_the_gloom(
    time: Res<Time>,
    zones: Res<SecuredZones>,
    chorus: Option<Res<super::gloom::Chorus>>,
    level: Res<CurrentLevel>,
    mut grade: ResMut<WorldGrade>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<bevy::light::GlobalAmbientLight>,
    mut fog: Query<&mut DistanceFog>,
    mut sun: Query<&mut DirectionalLight, With<KeySun>>,
) {
    // A level with no gloom sources has no reclaim arc to grade against — it is
    // a sandbox, not a mission — so it simply sits in daylight. Without this it
    // would hold at maximum gloom forever, since nothing can ever cure.
    let target = if level.gloom_sites.is_empty() {
        1.0
    } else {
        // Two ways the world comes back: raising loudspeaker coverage / the
        // stage (`music_level` reaches ~0.76 when the stage goes live), and —
        // for a district map with no build yet — simply cheering up the five
        // gloom sources. Whichever is further along wins. (Global for now; M3
        // makes it per-district so a saved district brightens on its own.)
        let by_coverage = (zones.music_level() / 0.78).clamp(0.0, 1.0);
        let by_district = chorus
            .map(|c| c.cured as f32 / super::gloom::STEMS.len() as f32)
            .unwrap_or(0.0);
        by_coverage.max(by_district)
    };
    let k = 1.0 - (-0.8 * time.delta_secs()).exp();
    grade.0 += (target - grade.0) * k.clamp(0.0, 1.0);

    apply_grade(
        grade.0,
        level.fog_scale(),
        &mut clear,
        &mut ambient,
        fog.single_mut().ok().as_deref_mut(),
        sun.single_mut().ok().as_deref_mut(),
    );
}

/// Write one point on the gloom-to-daylight ramp into the scene.
///
/// Split out of [`lift_the_gloom`] so the asset lab (`src/bin/lab.rs`) can put
/// a character under the exact same light the game grades it with, instead of
/// keeping a second copy of these numbers that quietly drifts.
/// `fog_scale` stretches the fog distances for a bigger map — see
/// [`Level::fog_scale`](super::level::Level::fog_scale). Pass `1.0` for the
/// town-sized default.
pub fn apply_grade(
    grade: f32,
    fog_scale: f32,
    clear: &mut ClearColor,
    ambient: &mut bevy::light::GlobalAmbientLight,
    fog: Option<&mut DistanceFog>,
    sun: Option<&mut DirectionalLight>,
) {
    // Smootherstep so the mid-range doesn't feel linear.
    let t = grade.clamp(0.0, 1.0);
    let t = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);

    clear.0 = mix(GLOOM.sky, LIT.sky, t);
    ambient.color = mix(GLOOM.ambient, LIT.ambient, t);
    ambient.brightness =
        GLOOM.ambient_brightness + (LIT.ambient_brightness - GLOOM.ambient_brightness) * t;

    if let Some(fog) = fog {
        fog.color = mix(GLOOM.fog, LIT.fog, t);
        fog.falloff = FogFalloff::Linear {
            start: (GLOOM.fog_start + (LIT.fog_start - GLOOM.fog_start) * t) * fog_scale,
            end: (GLOOM.fog_end + (LIT.fog_end - GLOOM.fog_end) * t) * fog_scale,
        };
    }

    if let Some(sun) = sun {
        sun.color = mix(GLOOM.sun, LIT.sun, t);
        sun.illuminance = GLOOM.sun_lux + (LIT.sun_lux - GLOOM.sun_lux) * t;
    }
}
