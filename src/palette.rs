//! The Grim Dawn `{^X}` color letters, named by color.
//!
//! Names and hex values are the game's own, taken from `gd-colorcodes.json`.
//! These say only what a letter *looks like* — what each one is used for is
//! decided in `color.rs` (rarity) and `property.rs` (damage types), so a color
//! can be repurposed there without its name going stale.
//!
//! Only the letters gdse actually bakes are listed. The rest of the palette is
//! either reserved by the engine — `e` Brown (body text), `h` Grayish Orange
//! (highlight), `s` Silver (the dynamic "inactive bonus" cue), `b` Blue and `i`
//! Indigo (Epic/Legendary item names), `q` Grayish Magenta (lore items) — too
//! dark to read on the tooltip background (`d` Dark Gray, `x` Dark Green), or
//! disabled outright in `gd-colorcodes.json` (`j`, `n`, `u`, `v`). That leaves
//! `t` Teal (`00FFD2`) as the only unused slot still worth spending.

/// `FFFFFF`
pub const WHITE: char = 'w';
/// `FFF62C`
pub const YELLOW: char = 'y';
/// `10EB5D`
pub const GREEN: char = 'g';
/// `F1E78C`
pub const KHAKI: char = 'k';
/// `FF69B5`
pub const FUSHIA: char = 'f';
/// `FF4200`
pub const RED: char = 'r';
/// `F3A44D`
pub const ORANGE: char = 'o';
/// `00FFFF`
pub const CYAN: char = 'c';
/// `6A91E0`
pub const COBALT: char = 'z';
/// `92CC00`
pub const OLIVE: char = 'l';
/// `800000`
pub const MAROON: char = 'm';
/// `80FFD5`
pub const AQUA: char = 'a';
/// `BD94C6`
pub const PURPLE: char = 'p';
/// Grayish Orange (engine highlight color)
pub const HIGHLIGHT_ORANGE: char = 'h';
/// `00FFD2`
pub const TEAL: char = 't';
