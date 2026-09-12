//! Sound. Game systems fire [`AudioCue`] messages; `play_cues` turns each into a
//! one-shot [`AudioPlayer`].
//!
//! The music is the other half of this file, and it is not a track — it is a
//! **stack of stems**. `assets/audio/stems/` holds five loops of one 124 BPM
//! song (see `tools/gen_stems.py`), one per crowd genre, and each belongs to
//! one of the five miserable townspeople in [`gloom`]. Cheer someone up and
//! their part of the song comes back; cheer up all five and the whole thing is
//! playing. `stem_mix` is the entire mixer: ride each sink toward the target
//! [`gloom::Chorus`] publishes.
//!
//! **Why the stems are started together and never restarted.** They are all
//! exactly the same number of samples long, so five loops kicked off on the
//! same frame stay phase-locked for the length of a run — no scheduler, no
//! beat clock. What breaks that is starting them on *different* frames, which
//! is exactly what happens if you spawn five [`AudioPlayer`]s over handles that
//! finish loading at different times. So `start_stems` waits until every stem
//! has fully loaded and then spawns all five in one go, silent, and after that
//! the only thing that ever changes is volume.
//!
//! The old single `music_loop.ogg` bed that faded up with secured ground is
//! gone: two pieces of music fighting each other is worse than either, and the
//! stems say the same thing (the town is coming back) with far more resolution.

use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::prelude::*;
use rand::Rng;

use super::GameState;
use super::gloom::{self, Chorus};

/// A one-shot sound request. Fired by gameplay systems; `play_cues` spawns the
/// actual audio entity so callers never touch `AssetServer`.
#[derive(Message, Clone, Copy, PartialEq, Eq)]
pub enum AudioCue {
    /// Drumstick "1-2-3-4" — a stage channel begins, or someone cheers up and
    /// their stem counts itself in.
    CountIn,
    /// Big power chord — Encore blast, beat slam, a stage finishing.
    ChordStab,
    /// Soft click on the drummer's metronome beat.
    Beat,
    /// A structure is placed / starts building.
    Build,
    /// Scrap collected.
    Scrap,
    /// A bad vibe bursts. Fires on every enemy death, so it is mixed quiet,
    /// pitched randomly, and capped per frame — see `play_cues`.
    Pop,
}

#[derive(Resource)]
struct AudioBank {
    count_in: Handle<AudioSource>,
    chord: Handle<AudioSource>,
    beat: Handle<AudioSource>,
    build: Handle<AudioSource>,
    scrap: Handle<AudioSource>,
    pop: Handle<AudioSource>,
}

/// The five stem loops, loaded at startup so they are ready to be started as
/// one batch. `started` latches once they have been spawned for this run.
#[derive(Resource)]
struct StemBank {
    stems: [Handle<AudioSource>; gloom::STEMS.len()],
    started: bool,
}

/// Marks one playing stem, and says which of the five it is.
#[derive(Component)]
struct MusicStem(usize);

/// Master level for the music. Five stems at full sum well past unity on their
/// own, so this is the headroom that keeps the mix under the gunfire.
const STEM_GAIN: f32 = 0.42;

/// How many burst pops may sound in a single frame.
const MAX_POPS_PER_FRAME: u32 = 4;

pub struct AudioCuePlugin;

impl Plugin for AudioCuePlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<AudioCue>()
            .add_systems(OnEnter(GameState::Playing), setup_audio)
            .add_systems(Update, (play_cues, start_stems, stem_mix));
    }
}

/// Load both banks and clear any stems left over from the previous run.
///
/// The stem handles are created **here** rather than in `Startup`, which looks
/// like the more natural home for them but is not: Bevy runs the initial
/// `StateTransition` *before* `Startup`, so a binary that opens directly in
/// `Playing` — the capture bot does — fires this `OnEnter` first and would find
/// no `StemBank` to write to.
fn setup_audio(
    mut commands: Commands,
    assets: Res<AssetServer>,
    old: Query<Entity, With<MusicStem>>,
) {
    for e in &old {
        commands.entity(e).despawn();
    }
    commands.insert_resource(StemBank {
        stems: gloom::STEMS.map(|id| assets.load(format!("audio/stems/{id}.ogg"))),
        started: false,
    });
    commands.insert_resource(AudioBank {
        count_in: assets.load("audio/count_in.ogg"),
        chord: assets.load("audio/chord_stab.ogg"),
        beat: assets.load("audio/beat_click.ogg"),
        build: assets.load("audio/build.ogg"),
        scrap: assets.load("audio/scrap.ogg"),
        pop: assets.load("audio/pop.ogg"),
    });
}

/// Once every stem has finished loading, start all five at once and silent.
/// This single frame is what keeps them in phase for the rest of the run.
fn start_stems(
    mut commands: Commands,
    assets: Res<AssetServer>,
    bank: Option<ResMut<StemBank>>,
    state: Res<State<GameState>>,
) {
    let Some(mut bank) = bank else { return };
    if bank.started || *state.get() != GameState::Playing {
        return;
    }
    if !bank
        .stems
        .iter()
        .all(|h| assets.is_loaded_with_dependencies(h))
    {
        return;
    }
    for (i, h) in bank.stems.iter().enumerate() {
        commands.spawn((
            MusicStem(i),
            AudioPlayer(h.clone()),
            PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        ));
    }
    bank.started = true;
}

fn play_cues(
    mut commands: Commands,
    bank: Option<Res<AudioBank>>,
    mut cues: MessageReader<AudioCue>,
) {
    let Some(bank) = bank else {
        cues.clear();
        return;
    };
    // A drummer's beat slam can kill thirty vibes at once. Thirty identical
    // one-shots on one frame is not thirty times louder, it is a click — so
    // pops are capped, and each survivor is pitched slightly differently so a
    // burst sounds like popcorn rather than one flanged thud.
    let mut pops = 0;
    let mut rng = rand::thread_rng();
    for cue in cues.read() {
        let (src, vol, speed) = match cue {
            AudioCue::CountIn => (bank.count_in.clone(), 0.7, 1.0),
            AudioCue::ChordStab => (bank.chord.clone(), 0.85, 1.0),
            AudioCue::Beat => (bank.beat.clone(), 0.25, 1.0),
            AudioCue::Build => (bank.build.clone(), 0.6, 1.0),
            AudioCue::Scrap => (bank.scrap.clone(), 0.35, 1.0),
            AudioCue::Pop => {
                pops += 1;
                if pops > MAX_POPS_PER_FRAME {
                    continue;
                }
                (bank.pop.clone(), 0.30, rng.gen_range(0.84..1.24))
            }
        };
        commands.spawn((
            AudioPlayer(src),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(vol))
                .with_speed(speed),
        ));
    }
}

/// The mixer. Each stem's sink chases the level its townsperson has earned.
fn stem_mix(
    time: Res<Time>,
    chorus: Option<Res<Chorus>>,
    mut stems: Query<(&MusicStem, &mut AudioSink)>,
) {
    let levels = chorus.map(|c| c.level).unwrap_or([0.0; gloom::STEMS.len()]);
    // Fast enough that a cure is heard as an arrival, slow enough that it
    // swells in rather than switching on.
    let k = 1.0 - (-2.2 * time.delta_secs()).exp();
    for (stem, mut sink) in &mut stems {
        let target = levels.get(stem.0).copied().unwrap_or(0.0) * STEM_GAIN;
        let now = sink.volume().to_linear();
        sink.set_volume(Volume::Linear(now + (target - now) * k));
    }
}
