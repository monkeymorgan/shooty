//! **Who you are playing.** The band has two lines of characters, drawn on two
//! different rigs, and a run can be played in either:
//!
//! * the **boxy** cast ([`skins::BOXY`]) — Kenney's six-box rig in a painted
//!   atlas. This is the house style (ART.md "The look"), and it leads.
//! * the **skeletal** cast ([`skins::SKINS`]) — the Quaternius mesh with
//!   per-material recolours and a face plane. Realistically proportioned, kept
//!   because it is what the game shipped and it is worth being able to put the
//!   two side by side in play, not only in a contact sheet.
//!
//! [`Pick`] is the one type that hides that difference. Everything a spawner
//! needs — the model, how to dress it, how big it stands, what its shots are
//! made of — comes off a `Pick`, so `player.rs` has one code path and the
//! character-select screen can list both casts in one row.

use bevy::prelude::*;

use super::Hero;
use super::props::Rig;
use super::skins::{self, Cast, Sound};

/// One playable character, from either cast.
///
/// Also a [`Component`], sat on the hero entity, so any system that needs to
/// know which rig it is dealing with — the animation driver, the gun mount,
/// the weapon — can just ask the hero rather than the roster.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    /// Index into [`skins::BOXY`].
    Boxy(usize),
    /// Index into [`skins::SKINS`].
    Skeletal(usize),
}

impl Pick {
    /// Every character you are allowed to play: the boxy band first (the house
    /// style leads), then the skeletal band.
    ///
    /// The crowd and gloom skins are deliberately absent — they are what you
    /// fight, and a wave of them should not be something you can also be.
    pub fn playable() -> Vec<Pick> {
        let boxy = skins::BOXY
            .iter()
            .enumerate()
            .filter(|(_, b)| b.cast == Cast::Band)
            .map(|(i, _)| Pick::Boxy(i));
        let skeletal = skins::SKINS
            .iter()
            .enumerate()
            .filter(|(_, s)| s.cast == Cast::Band)
            .map(|(i, _)| Pick::Skeletal(i));
        boxy.chain(skeletal).collect()
    }

    pub fn id(self) -> &'static str {
        match self {
            Pick::Boxy(i) => skins::BOXY[i].id,
            Pick::Skeletal(i) => skins::SKINS[i].id,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pick::Boxy(i) => skins::BOXY[i].name,
            Pick::Skeletal(i) => skins::SKINS[i].name,
        }
    }

    pub fn genre(self) -> &'static str {
        match self {
            Pick::Boxy(i) => skins::BOXY[i].genre,
            Pick::Skeletal(i) => skins::SKINS[i].genre,
        }
    }

    /// Which line this character is drawn on — the name the select screen
    /// shows so it is never a mystery which style you are looking at.
    pub fn line(self) -> &'static str {
        match self {
            Pick::Boxy(_) => "blocky",
            Pick::Skeletal(_) => "skeletal",
        }
    }

    pub fn rig(self) -> Rig {
        match self {
            Pick::Boxy(_) => Rig::Boxy,
            Pick::Skeletal(_) => Rig::Skeletal,
        }
    }

    /// The glTF scene to spawn.
    pub fn model(self) -> &'static str {
        match self {
            Pick::Boxy(_) => skins::BOXY_BASE,
            Pick::Skeletal(i) => skins::SKINS[i].model,
        }
    }

    /// The painted atlas to put on every part — boxy characters only, where
    /// hair, face, collar and boot are all in the one texture.
    pub fn atlas(self) -> Option<&'static str> {
        match self {
            Pick::Boxy(i) => Some(skins::BOXY[i].atlas),
            Pick::Skeletal(_) => None,
        }
    }

    /// The face-plane texture — skeletal characters only; a boxy character's
    /// face is painted into [`Pick::atlas`].
    pub fn face(self) -> Option<&'static str> {
        match self {
            Pick::Boxy(_) => None,
            Pick::Skeletal(i) => skins::SKINS[i].face,
        }
    }

    /// The replacement colour for a glTF material of this name, if any.
    pub fn tint(self, material: &str) -> Option<Color> {
        match self {
            Pick::Boxy(_) => None,
            Pick::Skeletal(i) => skins::SKINS[i].tint(material),
        }
    }

    /// Uniform model scale, from the height in metres the character declares.
    pub fn model_scale(self) -> f32 {
        match self {
            Pick::Boxy(i) => skins::BOXY[i].model_scale(),
            Pick::Skeletal(i) => skins::SKINS[i].model_scale(),
        }
    }

    /// Ground lift for the model root.
    pub fn lift(self) -> f32 {
        match self {
            Pick::Boxy(_) => skins::BOXY_LIFT * self.model_scale(),
            Pick::Skeletal(_) => 0.0,
        }
    }

    /// What this character's shots are made of.
    pub fn sound(self) -> Sound {
        skins::sound(self.id())
    }

    /// The clip name for one gait on this character's rig.
    ///
    /// The two rigs name their animations completely differently — Quaternius
    /// ships `Idle_Gun` / `Walk` / `Run`, Kenney ships `holding-right` /
    /// `walk` / `sprint` — so this is the whole of what the animation driver
    /// needs to know about which rig it is on.
    pub fn clip(self, gait: Gait, armed: bool) -> &'static str {
        match (self.rig(), gait) {
            (Rig::Skeletal, Gait::Idle) => {
                if armed {
                    "Idle_Gun"
                } else {
                    "Idle"
                }
            }
            (Rig::Skeletal, Gait::Walk) => "Walk",
            (Rig::Skeletal, Gait::Sprint) => "Run",
            (Rig::Skeletal, Gait::Shoot) => "Idle_Gun_Shoot",
            (Rig::Skeletal, Gait::Melee) => "Sword_Slash",
            (Rig::Skeletal, Gait::Die) => "Death",
            (Rig::Skeletal, Gait::Drive) => "Idle",
            (Rig::Boxy, Gait::Idle) => {
                if armed {
                    "holding-right"
                } else {
                    "idle"
                }
            }
            (Rig::Boxy, Gait::Walk) => "walk",
            (Rig::Boxy, Gait::Sprint) => "sprint",
            (Rig::Boxy, Gait::Shoot) => "holding-right-shoot",
            (Rig::Boxy, Gait::Melee) => "attack-melee-right",
            (Rig::Boxy, Gait::Die) => "die",
            // The one clip the blocky rig has and the skeletal one does not —
            // which is why the cars are theirs to drive.
            (Rig::Boxy, Gait::Drive) => "drive",
        }
    }

    /// Stock kit a character ships wearing that no role here uses.
    pub fn hides(self, node: &str) -> bool {
        matches!(self.rig(), Rig::Skeletal) && matches!(node, "Pistol" | "Backpack")
    }
}

/// The states a hero's body can be in, as far as animation is concerned.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Gait {
    Idle,
    Walk,
    Sprint,
    Shoot,
    Melee,
    Die,
    /// Sat in a car.
    Drive,
}

impl Gait {
    pub const ALL: [Gait; 7] = [
        Gait::Idle,
        Gait::Walk,
        Gait::Sprint,
        Gait::Shoot,
        Gait::Melee,
        Gait::Die,
        Gait::Drive,
    ];
}

/// Who each hero slot is being played as. Set on the character-select screen
/// and read by `player::spawn_hero` at the start of every run.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Roster {
    pub guitarist: Pick,
    pub drummer: Pick,
}

impl Default for Roster {
    fn default() -> Self {
        // The boxy punk and the boxy grunge drummer: the house style, and a
        // pair that reads as one band. `Pick::playable()` puts the boxy band
        // first, so these are also indices 0 and 2 of the select lineup.
        Self {
            guitarist: Pick::Boxy(0),
            drummer: Pick::Boxy(2),
        }
    }
}

impl Roster {
    pub fn of(&self, hero: Hero) -> Pick {
        match hero {
            Hero::Guitarist => self.guitarist,
            Hero::Drummer => self.drummer,
        }
    }

    pub fn set(&mut self, hero: Hero, pick: Pick) {
        match hero {
            Hero::Guitarist => self.guitarist = pick,
            Hero::Drummer => self.drummer = pick,
        }
    }
}
