//! **Credits.** `C` on the character-select screen; `C` or `Escape` closes it.
//!
//! This exists because of a licence, not because it is nice to have. Almost
//! everything in the game is CC0 and needs no acknowledgement at all, but the
//! three aircraft are **CC-BY**, and CC-BY is only satisfied if the credit
//! reaches the person playing — a line in a text file next to the model does
//! not discharge it. So the aircraft block here is an obligation, and the rest
//! is courtesy.
//!
//! See `assets/models/aircraft/LICENSE-cc-by.txt` for the same attribution in
//! the form the licence asks for.

use bevy::prelude::*;

use super::GameState;

/// Marks everything the credits overlay spawns.
#[derive(Component)]
struct CreditsEntity;

/// Whether the overlay is up.
#[derive(Resource, Default)]
struct Open(bool);

/// One block of the roll: a heading and its lines.
struct Section {
    heading: &'static str,
    /// `true` for the block that discharges a licence obligation, which is
    /// drawn brighter — it is the part that legally has to be read.
    required: bool,
    lines: &'static [&'static str],
}

const ROLL: &[Section] = &[
    Section {
        heading: "AIRCRAFT — Creative Commons Attribution (CC-BY 3.0)",
        required: true,
        lines: &[
            "\"plane 2\" by Jake Blakeley",
            "\"Small Airplane\" by Vojt\u{011b}ch Bal\u{e1}k",
            "\"Blimp\" by Poly by Google",
            "all via Poly Pizza  \u{2014}  poly.pizza",
        ],
    },
    Section {
        heading: "ENVIRONMENT & VEHICLES — Kenney (CC0)",
        required: false,
        lines: &[
            "City Kit: Commercial, Suburban, Industrial, Roads",
            "Car Kit  \u{b7}  Nature Kit  \u{b7}  Furniture Kit",
            "Blocky Characters  \u{2014}  the base mesh the band is painted onto",
            "kenney.nl",
        ],
    },
    Section {
        heading: "FARM BUILDINGS & CHARACTERS — Quaternius (CC0)",
        required: false,
        lines: &[
            "Farm Buildings Pack",
            "Ultimate Modular Men",
            "quaternius.com",
        ],
    },
    Section {
        heading: "MADE FOR THIS GAME",
        required: false,
        lines: &[
            "Loudspeaker, bad-vibe heads  \u{2014}  Tripo3D, from xAI Grok references",
            "Character skins, enemy faces, music stems  \u{2014}  generated in tools/",
        ],
    },
];

pub struct CreditsPlugin;

impl Plugin for CreditsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Open>()
            .add_systems(Update, toggle.run_if(in_state(GameState::Select)))
            .add_systems(OnExit(GameState::Select), close);
    }
}

/// `C` opens it, `C` or `Escape` closes it.
fn toggle(
    mut commands: Commands,
    assets: Res<AssetServer>,
    keys: Res<ButtonInput<KeyCode>>,
    mut open: ResMut<Open>,
    existing: Query<Entity, With<CreditsEntity>>,
) {
    let wants_open = keys.just_pressed(KeyCode::KeyC);
    let wants_close = keys.just_pressed(KeyCode::Escape) || (open.0 && wants_open);

    if open.0 && wants_close {
        open.0 = false;
        for e in &existing {
            commands.entity(e).despawn();
        }
    } else if !open.0 && wants_open {
        open.0 = true;
        spawn(&mut commands, super::ui::font(&assets));
    }
}

fn close(
    mut commands: Commands,
    mut open: ResMut<Open>,
    existing: Query<Entity, With<CreditsEntity>>,
) {
    open.0 = false;
    for e in &existing {
        commands.entity(e).despawn();
    }
}

fn spawn(commands: &mut Commands, font: bevy::text::FontSource) {
    let px = bevy::ui::Val::Px;
    commands
        .spawn((
            CreditsEntity,
            Node {
                position_type: PositionType::Absolute,
                top: px(0.0),
                left: px(0.0),
                width: bevy::ui::Val::Percent(100.0),
                height: bevy::ui::Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: px(3.0),
                ..default()
            },
            // Nearly opaque: the lineup behind it is busy, and an attribution
            // you have to squint through is not much of an attribution.
            BackgroundColor(Color::srgba(0.01, 0.02, 0.04, 0.975)),
        ))
        .with_children(|c| {
            c.spawn((
                Text::new("CREDITS"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.86, 0.42)),
                Node {
                    margin: bevy::ui::UiRect::bottom(px(14.0)),
                    ..default()
                },
            ));

            for section in ROLL {
                c.spawn((
                    Text::new(section.heading),
                    TextFont {
                        font: font.clone(),
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(if section.required {
                        Color::srgb(1.0, 0.78, 0.34)
                    } else {
                        Color::srgb(0.45, 0.72, 0.82)
                    }),
                    Node {
                        margin: bevy::ui::UiRect::top(px(12.0)),
                        ..default()
                    },
                ));
                for line in section.lines {
                    c.spawn((
                        Text::new(*line),
                        TextFont {
                            font: font.clone(),
                            font_size: FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(if section.required {
                            Color::srgb(0.94, 0.94, 0.96)
                        } else {
                            Color::srgb(0.70, 0.74, 0.80)
                        }),
                    ));
                }
            }

            c.spawn((
                Text::new("C or Escape to close"),
                TextFont {
                    font: font.clone(),
                    font_size: FontSize::Px(13.0),
                    ..default()
                },
                TextColor(Color::srgb(0.52, 0.56, 0.64)),
                Node {
                    margin: bevy::ui::UiRect::top(px(20.0)),
                    ..default()
                },
            ));
        });
}
