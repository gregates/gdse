use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::keywords::Rarity;

#[derive(Debug, Clone)]
pub struct UserPalette {
    values: HashMap<String, char>,
}

impl UserPalette {
    pub fn empty() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Option<char> {
        self.values.get(key).copied()
    }

    pub fn rarity(&self, rarity: Rarity) -> Option<char> {
        match rarity {
            Rarity::Common => self.get("rarity.common"),
            Rarity::Magical => self.get("rarity.magical"),
            Rarity::Rare => self.get("rarity.rare"),
            Rarity::Epic => self.get("rarity.epic"),
            Rarity::Legendary => self.get("rarity.legendary"),
        }
    }

    pub fn damage(&self, token: &str) -> Option<char> {
        match token {
            "Physical" => self.get("damage.physical"),
            "Pierce" => self.get("damage.pierce"),
            "Bleeding" => self.get("damage.bleeding"),
            "Fire" => self.get("damage.fire"),
            "Cold" => self.get("damage.cold"),
            "Lightning" => self.get("damage.lightning"),
            "Poison" => self.get("damage.poison"),
            "Vitality" => self.get("damage.vitality"),
            "Life" => self.get("damage.life"),
            "Aether" => self.get("damage.aether"),
            "Chaos" => self.get("damage.chaos"),
            "Elemental" => self.get("damage.elemental"),
            _ => None,
        }
    }

    pub fn non_damage(&self, key: &str) -> Option<char> {
        self.get(key)
    }
}

pub struct PaletteLoad {
    pub palette: UserPalette,
    pub source: Option<PathBuf>,
}

pub fn load(path_arg: Option<&Path>) -> PaletteLoad {
    if let Some(path) = path_arg {
        let palette = read_palette_file(path);
        return PaletteLoad {
            palette,
            source: Some(path.to_path_buf()),
        };
    }

    let default = Path::new("gdse-palette.txt");
    if default.exists() {
        let palette = read_palette_file(default);
        PaletteLoad {
            palette,
            source: Some(default.to_path_buf()),
        }
    } else {
        PaletteLoad {
            palette: UserPalette::empty(),
            source: None,
        }
    }
}

fn read_palette_file(path: &Path) -> UserPalette {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Could not read palette file {}: {e}", path.display());
        std::process::exit(1);
    });

    let mut values = HashMap::new();
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key_raw, val_raw)) = line.split_once('=') else {
            eprintln!(
                "Invalid palette line {} in {}: expected key=value",
                line_no + 1,
                path.display()
            );
            std::process::exit(1);
        };

        let key = key_raw.trim().to_ascii_lowercase();
        if !is_known_key(&key) {
            eprintln!(
                "Unknown palette key '{}' on line {} in {}",
                key,
                line_no + 1,
                path.display()
            );
            std::process::exit(1);
        }

        let value = val_raw.trim();
        let code = parse_color_code(value).unwrap_or_else(|| {
            eprintln!(
                "Invalid color code '{}' on line {} in {}. Use one letter like w, y, g, f, r.",
                value,
                line_no + 1,
                path.display()
            );
            std::process::exit(1);
        });

        values.insert(key, code);
    }

    UserPalette { values }
}

fn parse_color_code(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let c = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if c.is_ascii_alphabetic() {
        Some(c.to_ascii_lowercase())
    } else {
        None
    }
}

fn is_known_key(key: &str) -> bool {
    matches!(
        key,
        "rarity.common"
            | "rarity.magical"
            | "rarity.rare"
            | "rarity.epic"
            | "rarity.legendary"
            | "damage.physical"
            | "damage.pierce"
            | "damage.bleeding"
            | "damage.fire"
            | "damage.cold"
            | "damage.lightning"
            | "damage.poison"
            | "damage.vitality"
            | "damage.life"
            | "damage.aether"
            | "damage.chaos"
            | "damage.elemental"
            | "nondamage.attribute0"
            | "nondamage.mastery_increment"
            | "nondamage.all_skill_increment"
            | "nondamage.run_speed"
            | "nondamage.cast_speed"
            | "nondamage.attack_speed"
            | "nondamage.total_speed"
            | "nondamage.run_speed_modifier"
            | "nondamage.offensive_ability"
            | "nondamage.defensive_ability"
            | "nondamage.crit_damage"
            | "nondamage.damage_mult"
            | "nondamage.total_damage"
    )
}
