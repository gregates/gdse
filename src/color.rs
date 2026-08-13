//! The color model: maps a tag's inferred Kind + Rarity to the `{^X}` color
//! letter to bake into its text, trimmed to gdse's locked v1 scope (see
//! scope-decision memory).
//!
//! Only item & affix rarity is colored, and only the three rarities that can
//! carry a name-altering affix: Common=White, Magical=Yellow, Rare=Green.
//! Higher rarities keep native game colors by default, but can be overridden.
//! rarity color. Faction / Set / Skill / Quality / Style get no special cue
//! either, matching Full Rainbow. Damage-type Property colors live separately in
//! `property.rs`. Colors are the single-letter codes from gd-colorcodes.json.

use crate::infer::TagInfo;
use crate::keywords::{Kind, Rarity};
use crate::palette::{GREEN, WHITE, YELLOW};
use crate::user_palette::UserPalette;

/// The color letter to bake into the tag's value, or `None` if it's left
/// untouched. Base item names are only colored when they can take a name-
/// altering affix (otherwise the engine's native rarity color is fine and there
/// is no bleed to guard against); affix tags are always colored by their rarity.
pub fn color_for(info: &TagInfo, user_palette: &UserPalette) -> Option<char> {
    if info.kind == Kind::Item && !info.affixable {
        return None;
    }
    match info.rarity {
        Rarity::Common => Some(user_palette.rarity(Rarity::Common).unwrap_or(WHITE)),
        Rarity::Magical => Some(user_palette.rarity(Rarity::Magical).unwrap_or(YELLOW)),
        Rarity::Rare => Some(user_palette.rarity(Rarity::Rare).unwrap_or(GREEN)),
        // Optional override key: rarity.epic
        Rarity::Epic => user_palette.rarity(Rarity::Epic),
        // Optional override key: rarity.legendary
        Rarity::Legendary => user_palette.rarity(Rarity::Legendary),
    }
}
