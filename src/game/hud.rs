use bevy::prelude::*;

use super::build::{SPEAKERS_TO_LINK, Scrap, SpeakerNet};
use super::combat::Reviving;
use super::enemy::{Wave, WavePhase};
use super::{Downed, GameState, Health, Hero, Player, RunEntity, Score};

pub struct HudPlugin;

#[derive(Resource, Default)]
pub struct RunClock(pub f32);

#[derive(Component)]
struct HealthFill(Hero);

#[derive(Component)]
struct HeroLabel(Hero);

#[derive(Component)]
struct StatsText;

#[derive(Component)]
struct GameOverUi;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunClock>()
            .add_systems(OnEnter(GameState::Playing), spawn_hud)
            .add_systems(OnEnter(GameState::GameOver), spawn_game_over)
            .add_systems(OnEnter(GameState::Victory), spawn_victory)
            .add_systems(OnExit(GameState::GameOver), despawn_game_over)
            .add_systems(OnExit(GameState::Victory), despawn_game_over)
            .add_systems(
                Update,
                (tick_clock, update_hud).run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                restart
                    .run_if(in_state(GameState::GameOver).or_else(in_state(GameState::Victory)))
                    // On a net client the run is restarted by the host and
                    // followed via the snapshot's game state.
                    .run_if(super::net::authoritative),
            );
    }
}

/// One health bar + name, pinned to a screen corner.
fn health_bar(commands: &mut Commands, hero: Hero, right: bool) {
    let (name, fill_color) = match hero {
        Hero::Guitarist => ("GUITARIST", Color::srgb(0.30, 0.80, 0.95)),
        Hero::Drummer => ("DRUMMER", Color::srgb(1.0, 0.62, 0.22)),
    };
    let edge = px(16);
    let mut anchor = Node {
        position_type: PositionType::Absolute,
        top: px(16),
        width: px(240),
        flex_direction: FlexDirection::Column,
        row_gap: px(4),
        ..default()
    };
    if right {
        anchor.right = edge;
        anchor.align_items = AlignItems::End;
    } else {
        anchor.left = edge;
    }

    commands.spawn((anchor, RunEntity)).with_children(|c| {
        c.spawn((
            Text::new(name),
            TextFont {
                font_size: FontSize::Px(15.0),
                ..default()
            },
            TextColor(fill_color),
            HeroLabel(hero),
        ));
        c.spawn((
            Node {
                width: percent(100.0),
                height: px(20),
                border: UiRect::all(px(2)),
                ..default()
            },
            BorderColor::all(Color::srgb(0.8, 0.85, 0.9)),
            BackgroundColor(Color::srgb(0.12, 0.05, 0.05)),
        ))
        .with_children(|c| {
            c.spawn((
                Node {
                    width: percent(100.0),
                    height: percent(100.0),
                    ..default()
                },
                BackgroundColor(fill_color),
                HealthFill(hero),
            ));
        });
    });
}

fn spawn_hud(mut commands: Commands, party: Res<super::Party>) {
    health_bar(&mut commands, Hero::Guitarist, false);
    if *party == super::Party::Duo {
        health_bar(&mut commands, Hero::Drummer, true);
    }

    // Stats line, centred under the bars.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::srgb(0.9, 0.93, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            top: px(78),
            left: px(16),
            ..default()
        },
        StatsText,
        RunEntity,
    ));

    // Controls hint (bottom-left).
    let hint = if *party == super::Party::Duo {
        "P1  WASD / mouse / LMB fire / RMB encore / Space dodge / Q loudspeaker (marked pitch) / E drive / fly  \
         P2  pad or Arrows / [/] swing / [,] slam / [.] dodge / [;] stage (bandstand) / ['] drive"
    } else {
        "WASD move / mouse aim / LMB fire / RMB encore / Space dodge / E get in a car / \
         Q build — stand on a marked pitch (speakers), bandstand (stage)"
    };
    commands.spawn((
        Text::new(hint),
        TextFont {
            font_size: FontSize::Px(14.0),
            ..default()
        },
        TextColor(Color::srgb(0.6, 0.65, 0.75)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(14),
            left: px(16),
            ..default()
        },
        RunEntity,
    ));
}

fn tick_clock(time: Res<Time>, mut clock: ResMut<RunClock>) {
    clock.0 += time.delta_secs();
}

#[allow(clippy::type_complexity)]
fn update_hud(
    clock: Res<RunClock>,
    score: Res<Score>,
    players: Query<(&Hero, &Health, Option<&Downed>, Option<&Reviving>), With<Player>>,
    mut fills: Query<(&HealthFill, &mut Node), Without<HeroLabel>>,
    mut labels: Query<(&HeroLabel, &mut Text), Without<HealthFill>>,
    mut stats: Query<&mut Text, (With<StatsText>, Without<HeroLabel>)>,
    scrap: Option<Res<Scrap>>,
    wave: Option<Res<Wave>>,
    net: Option<Res<SpeakerNet>>,
    chorus: Option<Res<super::gloom::Chorus>>,
) {
    let scrap = scrap.map(|s| s.0).unwrap_or(0);
    for (fill, mut node) in &mut fills {
        if let Some((_, hp, ..)) = players.iter().find(|(h, ..)| **h == fill.0) {
            node.width = percent((hp.current / hp.max).clamp(0.0, 1.0) * 100.0);
        }
    }
    for (label, mut text) in &mut labels {
        let Some((_, _, downed, reviving)) = players.iter().find(|(h, ..)| **h == label.0) else {
            continue;
        };
        let name = match label.0 {
            Hero::Guitarist => "GUITARIST",
            Hero::Drummer => "DRUMMER",
        };
        text.0 = if downed.is_some() {
            let pct = reviving.map(|r| (r.0 / 2.2 * 100.0) as i32).unwrap_or(0);
            format!("{name}  — DOWN  (revive {pct}%)")
        } else {
            name.to_string()
        };
    }
    if let Ok(mut text) = stats.single_mut() {
        let linked = net.map(|n| n.largest).unwrap_or(0);
        let wave_str = match wave.as_deref() {
            Some(w) => match w.phase {
                WavePhase::Prep => {
                    format!("Wave {} -  BUILD  ({:.0}s)", w.number + 1, w.prep_left())
                }
                WavePhase::Spawning => format!("Wave {} -  INCOMING", w.number),
                WavePhase::Clearing => format!("Wave {} -  clear the streets", w.number),
            },
            None => String::new(),
        };
        // The band coming back together, as a row of stem lamps: one per
        // gloom source, lit when that person has cheered up. This is the
        // readout for the whole music mechanic — you should be able to see
        // what you are about to hear.
        let band = match chorus.as_deref() {
            Some(c) if c.complete() => "\nTHE BAND IS BACK  -  [#####]".to_string(),
            Some(c) => {
                let lamps: String = c
                    .level
                    .iter()
                    .map(|l| if *l > 0.05 { '#' } else { '.' })
                    .collect();
                format!("\nMusic {}/5  [{}]", c.cured, lamps)
            }
            None => String::new(),
        };
        text.0 = format!(
            "{wave_str}\nTime {:.0}s   Kills {}   Scrap {}   Speakers linked {}/{}{band}",
            clock.0, score.kills, scrap, linked, SPEAKERS_TO_LINK
        );
    }
}

fn spawn_victory(mut commands: Commands, clock: Res<RunClock>, score: Res<Score>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.10, 0.68)),
            GameOverUi,
        ))
        .with_children(|c| {
            c.spawn((
                Text::new("STAGE IS LIVE"),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.95, 1.0)),
            ));
            c.spawn((
                Text::new("The band reclaimed the town."),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            c.spawn((
                Text::new(format!(
                    "{:.0}s   -   {} bad vibes dispelled",
                    clock.0, score.kills
                )),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.75, 0.85)),
            ));
            c.spawn((
                Text::new("R  play again        M  character select"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.65, 0.75)),
            ));
        });
}

fn spawn_game_over(mut commands: Commands, clock: Res<RunClock>, score: Res<Score>) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GameOverUi,
        ))
        .with_children(|c| {
            c.spawn((
                Text::new("BAND DOWN"),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.3, 0.35)),
            ));
            c.spawn((
                Text::new(format!(
                    "Survived {:.0}s   -   {} kills",
                    clock.0, score.kills
                )),
                TextFont {
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            c.spawn((
                Text::new("R  restart        M  character select"),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.7, 0.75, 0.85)),
            ));
        });
}

fn despawn_game_over(mut commands: Commands, q: Query<Entity, With<GameOverUi>>) {
    for e in &q {
        commands.entity(e).despawn();
    }
}

fn restart(
    keys: Res<ButtonInput<KeyCode>>,
    mut clock: ResMut<RunClock>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        clock.0 = 0.0;
        next_state.set(GameState::Playing);
    } else if keys.just_pressed(KeyCode::KeyM) {
        // Back to the character-select lineup — which is also the main menu.
        // `select::clear_previous_run` tears the finished run down on entry.
        clock.0 = 0.0;
        next_state.set(GameState::Select);
    }
}
