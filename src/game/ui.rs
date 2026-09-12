//! The UI font.
//!
//! Bevy's built-in default font covers little more than ASCII. The select
//! screen had been quietly drawing `·` and `←` `→` as empty boxes because of
//! it, and the credits screen cannot use it at all: "Vojtěch Balák" would come
//! out mangled, and a mangled name is a **failed attribution**, not a cosmetic
//! bug — see `assets/models/aircraft/LICENSE-cc-by.txt`.
//!
//! DejaVu Sans Mono is monospace (matching the HUD's look), covers Latin
//! Extended-A and the arrows, and is freely redistributable — its licence ships
//! beside it in `assets/fonts/`.

use bevy::prelude::*;

const UI_FONT: &str = "fonts/DejaVuSansMono.ttf";

/// The font every piece of UI text should use, ready to drop into
/// [`TextFont::font`].
pub fn font(assets: &AssetServer) -> bevy::text::FontSource {
    assets.load::<Font>(UI_FONT).into()
}
