//! M5 — a corner minimap: the town outline, the ground the band has secured,
//! and live blips for the two heroes, the swarm, the loudspeakers and the
//! stage. The secured-zone discs are the coverage read — they grow and brighten
//! as you push the frontier back, the same story the [`grade`](super::grade)
//! light-lift tells in the world itself.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use super::build::{BuildSite, Pitch, SecuredZones, Speaker, Stage, StructureKind};
use super::level::RoadNet;
use super::{CurrentLevel, Enemy, GameState, Hero, Player, RunEntity, plane};

/// Minimap texture size. ~1.39:1, to match the town's arena (100:72).
const MW: usize = 192;
const MH: usize = 138;

/// Handle to the CPU-side image the minimap redraws each frame.
#[derive(Resource)]
struct MinimapTex(Handle<Image>);

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), setup_minimap)
            .add_systems(Update, draw_minimap.run_if(in_state(GameState::Playing)));
    }
}

fn setup_minimap(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    existing: Option<Res<MinimapTex>>,
    level: Res<CurrentLevel>,
) {
    // Reuse the texture across runs; only build it once.
    let handle = if let Some(tex) = existing {
        tex.0.clone()
    } else {
        let mut image = Image::new(
            Extent3d {
                width: MW as u32,
                height: MH as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![0; MW * MH * 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        );
        image.sampler = ImageSampler::nearest();
        let handle = images.add(image);
        commands.insert_resource(MinimapTex(handle.clone()));
        handle
    };

    // The framed panel, bottom-right.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: px(16),
                bottom: px(16),
                padding: UiRect::all(px(3)),
                border: UiRect::all(px(2)),
                flex_direction: FlexDirection::Column,
                row_gap: px(3),
                ..default()
            },
            BorderColor::all(Color::srgb(0.35, 0.62, 0.72)),
            BackgroundColor(Color::srgba(0.02, 0.04, 0.06, 0.72)),
            RunEntity,
        ))
        .with_children(|c| {
            c.spawn((
                Text::new(level.name.to_uppercase()),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.72, 0.8)),
            ));
            c.spawn((
                ImageNode::new(handle),
                Node {
                    width: px(MW as i32),
                    height: px(MH as i32),
                    ..default()
                },
            ));
        });
}

/// Ground-plane point → pixel in the minimap image, for a level of half-extents
/// `arena`.
fn to_px(p: Vec2, arena: Vec2) -> (i32, i32) {
    let u = (p.x / arena.x) * 0.5 + 0.5;
    let v = 0.5 - (p.y / arena.y) * 0.5;
    ((u * MW as f32) as i32, (v * MH as f32) as i32)
}

fn draw_minimap(
    time: Res<Time>,
    level: Res<CurrentLevel>,
    net: Option<Res<RoadNet>>,
    tex: Option<Res<MinimapTex>>,
    mut images: ResMut<Assets<Image>>,
    zones: Res<SecuredZones>,
    players: Query<(&Transform, &Hero, Option<&super::Downed>), With<Player>>,
    enemies: Query<&Transform, With<Enemy>>,
    speakers: Query<&Transform, With<Speaker>>,
    stage: Query<&Transform, With<Stage>>,
    sites: Query<&Transform, With<BuildSite>>,
    pitches: Query<&Pitch>,
    gloom: Query<(&super::gloom::GloomSource, &Transform)>,
) {
    let Some(tex) = tex else { return };
    let Some(mut image) = images.get_mut(&tex.0) else {
        return;
    };
    let Some(buf) = image.data.as_mut() else {
        return;
    };

    let mut blend = |x: i32, y: i32, rgb: [f32; 3], a: f32| {
        if x < 0 || y < 0 || x >= MW as i32 || y >= MH as i32 {
            return;
        }
        let i = (y as usize * MW + x as usize) * 4;
        for k in 0..3 {
            let src = buf[i + k] as f32 / 255.0;
            let out = src * (1.0 - a) + rgb[k] * a;
            buf[i + k] = (out.clamp(0.0, 1.0) * 255.0) as u8;
        }
        buf[i + 3] = 255;
    };

    let arena = level.arena;

    // ---- Base: grass ground, faintly darker on the roads for orientation.
    for y in 0..MH as i32 {
        for x in 0..MW as i32 {
            let wx = ((x as f32 + 0.5) / MW as f32 - 0.5) * 2.0 * arena.x;
            let wy = (0.5 - (y as f32 + 0.5) / MH as f32) * 2.0 * arena.y;
            let edge = x < 1 || y < 1 || x >= MW as i32 - 1 || y >= MH as i32 - 1;
            let rgb = if edge {
                [0.28, 0.44, 0.5]
            } else if net.as_ref().is_some_and(|n| n.0.on_road(Vec2::new(wx, wy))) {
                [0.10, 0.11, 0.13]
            } else {
                [0.13, 0.17, 0.13]
            };
            blend(x, y, rgb, 1.0);
        }
    }

    // ---- Reclaimed districts: a soft warm wash around the home of every gloom
    // source that has been cheered up. This is the "what have I saved" read —
    // it fills in as you clear the map district by district.
    for (g, _) in &gloom {
        if !matches!(
            g.mood,
            super::gloom::Mood::Marching | super::gloom::Mood::Happy
        ) {
            continue;
        }
        let (cx, cy) = to_px(g.home, arena);
        // A generous radius — a district, not a point.
        let rr = 44.0_f32;
        let rx = (rr / arena.x * 0.5 * MW as f32).ceil() as i32;
        let ry = (rr / arena.y * 0.5 * MH as f32).ceil() as i32;
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let f = (dx as f32 / rx as f32).powi(2) + (dy as f32 / ry as f32).powi(2);
                if f > 1.0 {
                    continue;
                }
                blend(cx + dx, cy + dy, [1.0, 0.82, 0.42], (1.0 - f) * 0.26);
            }
        }
    }

    // ---- Secured zones: additive glow discs — the coverage read.
    let lvl = zones.music_level();
    for z in &zones.0 {
        let (cx, cy) = to_px(z.center, arena);
        let rx = (z.radius / arena.x * 0.5 * MW as f32).ceil() as i32;
        let ry = (z.radius / arena.y * 0.5 * MH as f32).ceil() as i32;
        for dy in -ry..=ry {
            for dx in -rx..=rx {
                let f = (dx as f32 / rx as f32).powi(2) + (dy as f32 / ry as f32).powi(2);
                if f > 1.0 {
                    continue;
                }
                // Brighter at the centre, and brighter overall as the town lifts.
                let a = (1.0 - f) * (0.16 + 0.3 * lvl);
                blend(cx + dx, cy + dy, [0.35, 0.95, 0.85], a.min(0.62));
            }
        }
    }

    let dot = |blend: &mut dyn FnMut(i32, i32, [f32; 3], f32),
               p: Vec2,
               r: i32,
               rgb: [f32; 3],
               outline: bool| {
        let (cx, cy) = to_px(p, arena);
        if outline {
            for dy in -(r + 1)..=(r + 1) {
                for dx in -(r + 1)..=(r + 1) {
                    blend(cx + dx, cy + dy, [0.02, 0.02, 0.03], 0.8);
                }
            }
        }
        for dy in -r..=r {
            for dx in -r..=r {
                blend(cx + dx, cy + dy, rgb, 1.0);
            }
        }
    };

    // ---- Swarm.
    for t in &enemies {
        dot(
            &mut blend,
            plane(t.translation),
            0,
            [0.95, 0.25, 0.22],
            false,
        );
    }

    // ---- Free pitches: hollow markers, so the route between them is legible
    // from the minimap alone. This is the whole reason the pitches are fixed.
    let pulse = 0.5 + 0.5 * (time.elapsed_secs() * 6.0).sin();
    for pitch in &pitches {
        let rgb = match pitch.kind {
            StructureKind::Speaker => [0.30, 0.62 + 0.20 * pulse, 0.80],
            StructureKind::Stage => [0.85, 0.55 + 0.20 * pulse, 0.22],
        };
        dot(&mut blend, pitch.at, 1, rgb, false);
    }

    // ---- Build sites: pulsing amber.
    for t in &sites {
        dot(
            &mut blend,
            plane(t.translation),
            1,
            [1.0, 0.7 + 0.3 * pulse, 0.2],
            true,
        );
    }

    // ---- Loudspeakers + the stage.
    for t in &speakers {
        dot(&mut blend, plane(t.translation), 1, [0.4, 0.95, 1.0], true);
    }
    for t in &stage {
        dot(&mut blend, plane(t.translation), 2, [0.7, 1.0, 1.0], true);
        dot(&mut blend, plane(t.translation), 1, [1.0, 0.5, 0.95], false);
    }

    // ---- The five. A purple ring while they are still miserable (this is how
    // you find the source of a wave in a town this size), warm and solid once
    // they have come round and joined the crowd at the bandstand.
    for (g, t) in &gloom {
        let p = plane(t.translation);
        match g.mood {
            super::gloom::Mood::Dormant => dot(&mut blend, p, 1, [0.34, 0.30, 0.46], true),
            super::gloom::Mood::Brooding => {
                dot(&mut blend, p, 2, [0.05, 0.03, 0.10], true);
                dot(&mut blend, p, 1, [0.62, 0.36 + 0.34 * pulse, 0.95], false);
            }
            super::gloom::Mood::Marching | super::gloom::Mood::Happy => {
                dot(&mut blend, p, 1, [1.0, 0.86, 0.42], true)
            }
        }
    }

    // ---- Heroes, on top.
    for (t, hero, downed) in &players {
        let rgb = match hero {
            Hero::Guitarist => [0.35, 0.85, 1.0],
            Hero::Drummer => [1.0, 0.62, 0.22],
        };
        let rgb = if downed.is_some() {
            [rgb[0] * 0.4, rgb[1] * 0.4, rgb[2] * 0.4]
        } else {
            rgb
        };
        dot(&mut blend, plane(t.translation), 2, rgb, true);
    }
}
