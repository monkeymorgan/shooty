use bevy::prelude::*;

use super::pickup::EnemyDied;
use super::vfx::Explosion;
use super::{
    AutoPlay, Bullet, Downed, Enemy, EnemyBullet, EnemyKind, GameState, Health, Hitbox, Obstacle,
    Pacified, Player, Score, plane,
};
use crate::game::player::Dodge;

/// Standing next to a downed team-mate for this long brings them back.
const REVIVE_SECS: f32 = 2.2;
const REVIVE_REACH: f32 = 3.0;
/// Fraction of max health a revived hero comes back with.
const REVIVE_HEALTH: f32 = 0.45;

/// On a [`Downed`] player: how long a living team-mate has been reviving them.
#[derive(Component, Default)]
pub struct Reviving(pub f32);

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                bullets_hit_enemies,
                enemy_bullets_hit_player,
                enemies_touch_player,
                check_death,
                revive_downed,
            )
                .chain()
                .run_if(in_state(GameState::Playing))
                .run_if(super::net::authoritative),
        );
    }
}

fn overlap(a: Vec2, ra: f32, b: Vec2, rb: f32) -> bool {
    a.distance_squared(b) <= (ra + rb) * (ra + rb)
}

fn bullets_hit_enemies(
    mut commands: Commands,
    mut cues: MessageWriter<super::audio::AudioCue>,
    bullets: Query<(Entity, &Transform, &Bullet, &Hitbox)>,
    obstacles: Query<(&Transform, &Obstacle)>,
    mut enemies: Query<(&Transform, &Hitbox, &mut Health), With<Enemy>>,
) {
    for (bullet_e, bt, bullet, bhb) in &bullets {
        let bp = plane(bt.translation);

        // Picks thunk into the city blocks and stop.
        if obstacles
            .iter()
            .any(|(t, o)| overlap(bp, bhb.0, plane(t.translation), o.radius))
        {
            commands.entity(bullet_e).despawn();
            continue;
        }

        for (et, ehb, mut hp) in &mut enemies {
            if overlap(bp, bhb.0, plane(et.translation), ehb.0) {
                hp.current -= bullet.damage;
                cues.write(super::audio::AudioCue::Hit);
                commands.entity(bullet_e).despawn();
                break;
            }
        }
    }
}

/// A bad-vibe bolt from a Heckler that lands on a hero. Mirrors
/// [`bullets_hit_enemies`]: stopped by city blocks, ignored while a hero is
/// dodge-rolling or driving.
#[allow(clippy::type_complexity)]
fn enemy_bullets_hit_player(
    mut commands: Commands,
    mut cues: MessageWriter<super::audio::AudioCue>,
    bolts: Query<(Entity, &Transform, &EnemyBullet, &Hitbox)>,
    obstacles: Query<(&Transform, &Obstacle)>,
    mut players: Query<
        (
            &Transform,
            &Hitbox,
            &mut Health,
            &Dodge,
            Option<&super::drive::Driving>,
        ),
        (With<Player>, Without<Downed>),
    >,
) {
    for (bolt_e, bt, bolt, bhb) in &bolts {
        let bp = plane(bt.translation);
        if obstacles
            .iter()
            .any(|(t, o)| overlap(bp, bhb.0, plane(t.translation), o.radius))
        {
            commands.entity(bolt_e).despawn();
            continue;
        }
        for (pt, phb, mut hp, dodge, driving) in &mut players {
            if dodge.is_rolling() || driving.is_some() {
                continue;
            }
            if overlap(bp, bhb.0, plane(pt.translation), phb.0) {
                hp.current -= bolt.damage;
                cues.write(super::audio::AudioCue::HeroHurt);
                commands.entity(bolt_e).despawn();
                break;
            }
        }
    }
}

/// Contact damage to every living hero the swarm is touching.
///
/// This is continuous rather than a discrete event — a hero can stand in the
/// swarm for seconds at a time — so the hurt cue is throttled by `hurt_cd`
/// rather than fired every frame it happens, which would just be a drone.
fn enemies_touch_player(
    time: Res<Time>,
    autoplay: Option<Res<AutoPlay>>,
    mut cues: MessageWriter<super::audio::AudioCue>,
    mut hurt_cd: Local<f32>,
    mut players: Query<
        (
            &Transform,
            &Hitbox,
            &mut Health,
            &Dodge,
            Option<&super::drive::Driving>,
        ),
        (With<Player>, Without<Downed>),
    >,
    enemies: Query<(&Transform, &Hitbox, &Enemy), Without<Pacified>>,
) {
    // The scripted capture bot takes reduced contact damage so the proof video
    // shows sustained action rather than the bot's (lack of) skill.
    let scale = if autoplay.is_some() { 0.22 } else { 1.0 };
    *hurt_cd = (*hurt_cd - time.delta_secs()).max(0.0);
    let mut touched = false;
    for (pt, phb, mut hp, dodge, driving) in &mut players {
        // I-frames while rolling; a car's bodywork while driving. The car pays
        // for that by not being able to build (see `drive.rs`).
        if dodge.is_rolling() || driving.is_some() {
            continue;
        }
        let pp = plane(pt.translation);
        for (et, ehb, enemy) in &enemies {
            if overlap(pp, phb.0, plane(et.translation), ehb.0) {
                hp.current -= enemy.touch_damage * scale * time.delta_secs();
                touched = true;
            }
        }
    }
    if touched && *hurt_cd <= 0.0 {
        cues.write(super::audio::AudioCue::HeroHurt);
        *hurt_cd = 0.35;
    }
}

#[allow(clippy::type_complexity)]
fn check_death(
    mut commands: Commands,
    mut score: ResMut<Score>,
    mut next_state: ResMut<NextState<GameState>>,
    mut died: MessageWriter<EnemyDied>,
    mut boom: MessageWriter<Explosion>,
    mut cues: MessageWriter<super::audio::AudioCue>,
    enemies: Query<(Entity, &Health, &Transform, &Enemy)>,
    players: Query<(Entity, &Health, Option<&Downed>), With<Player>>,
) {
    for (e, hp, transform, enemy) in &enemies {
        if hp.current <= 0.0 {
            score.kills += 1;
            died.write(EnemyDied {
                pos: transform.translation,
                kind: enemy.kind,
            });
            boom.write(Explosion {
                pos: transform.translation,
                kind: enemy.kind,
                power: match enemy.kind {
                    EnemyKind::Sink => 1.9,
                    EnemyKind::Head => 0.7,
                    _ => 1.0,
                },
            });
            cues.write(super::audio::AudioCue::Pop);
            commands.entity(e).despawn();
        }
    }

    // A hero at zero health goes down but the run continues while a team-mate
    // stands; only when every hero is down is it game over.
    let mut all_down = true;
    for (e, hp, downed) in &players {
        if downed.is_none() && hp.current <= 0.0 {
            commands.entity(e).insert((Downed, Reviving::default()));
        }
        all_down &= downed.is_some() || hp.current <= 0.0;
    }
    if all_down && !players.is_empty() {
        next_state.set(GameState::GameOver);
    }
}

/// A living hero standing over a [`Downed`] team-mate revives them.
fn revive_downed(
    mut commands: Commands,
    time: Res<Time>,
    living: Query<&Transform, (With<Player>, Without<Downed>)>,
    mut downed: Query<
        (Entity, &Transform, &mut Health, &mut Reviving),
        (With<Player>, With<Downed>),
    >,
) {
    for (e, dt, mut hp, mut prog) in &mut downed {
        let dp = plane(dt.translation);
        let helped = living
            .iter()
            .any(|t| plane(t.translation).distance(dp) <= REVIVE_REACH);
        if helped {
            prog.0 += time.delta_secs();
            if prog.0 >= REVIVE_SECS {
                hp.current = hp.max * REVIVE_HEALTH;
                commands.entity(e).remove::<Downed>().remove::<Reviving>();
            }
        } else {
            prog.0 = (prog.0 - time.delta_secs() * 0.6).max(0.0);
        }
    }
}
