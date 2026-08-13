//! The colorizer: bakes `{^X}` color codes into the localization tag text and
//! writes the rewritten `.txt` files for Grim Dawn to load.
//!
//! Source text is read from the game's pristine `Text_EN.arc` bundles (never the
//! already-colored files in settings/text_en, which would double-color). Each
//! tag we have a color for has its value rewritten by `apply_color` (a port of
//! WanezGD's placement cascade), and any file that changed is written whole —
//! comments, Desc lines and untouched tags preserved — to the output directory.

use std::collections::{BTreeSet, HashMap};
use std::io::{BufRead, Seek};
use std::path::Path;

use lib_gddb::arc::Archive;
use lib_gddb::arz::Database;

use crate::color;
use crate::db::install_path;
use crate::infer;
use crate::property::{self, DamageColors};
use crate::user_palette::UserPalette;

/// The text bundles for a language, base game + expansions, in load order. EN
/// ships one arc per part; other languages bundle everything into the base arc
/// (with `aom/`/`fg/` subdirs), so the missing per-expansion arcs are simply
/// skipped when they don't exist.
fn text_arcs(lang: &str) -> [String; 4] {
    let up = lang.to_uppercase();
    [
        format!("resources/Text_{up}.arc"),
        format!("gdx1/resources/Text_{up}.arc"),
        format!("gdx2/resources/Text_{up}.arc"),
        format!("gdx3/resources/Text_{up}.arc"),
    ]
}

/// Best-effort patch-version detection from decoded tag text.
///
/// Looks for comment markers like `#Patch v1.3.1` inside `tags*.txt` records
/// from the language text archives and returns distinct discovered versions.
pub fn detect_patch_versions(lang: &str) -> Vec<String> {
    let base = install_path();
    let mut versions = BTreeSet::new();

    for rel in text_arcs(lang) {
        let Ok(mut arc) = Archive::open(base.join(&rel)) else {
            continue;
        };
        let Ok(records) = arc.iter_records() else {
            continue;
        };
        for record in records.flatten() {
            if !record.id.contains("tag") || !record.id.ends_with(".txt") {
                continue;
            }
            let text = String::from_utf8_lossy(&record.data);
            for line in text.lines() {
                if let Some(v) = parse_patch_version(line) {
                    versions.insert(v.to_string());
                }
            }
        }
    }

    versions.into_iter().collect()
}

fn parse_patch_version(line: &str) -> Option<&str> {
    // Most releases use comment markers like `#Patch v1.3.1`, but some builds
    // may use other words such as `Hotfix` or `Update`.
    let lower = line.to_ascii_lowercase();
    let marker_idx = ["#patch", "#hotfix", "#update"]
        .iter()
        .filter_map(|m| lower.find(m))
        .min()?;

    let tail = line[marker_idx..].trim();
    extract_version_token(tail)
}

fn extract_version_token(s: &str) -> Option<&str> {
    for raw in s.split_whitespace() {
        let trimmed = raw.trim_matches(|c: char| ",;:()[]{}\"'".contains(c));
        let core = trimmed
            .strip_prefix('v')
            .or_else(|| trimmed.strip_prefix('V'))
            .unwrap_or(trimmed);
        if is_versionish(core) {
            return Some(core);
        }
    }
    None
}

fn is_versionish(s: &str) -> bool {
    let mut chars = s.chars().peekable();
    let mut saw_dot = false;
    let mut saw_digit = false;
    let mut starts_with_digit = false;

    if let Some(c) = chars.peek().copied() {
        starts_with_digit = c.is_ascii_digit();
    }

    for c in chars {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else if c == '.' {
            saw_dot = true;
        } else if c == '-' || c == '_' || c.is_ascii_alphabetic() {
            // Allow suffixes like 1.3.1a or 1.3.1-hotfix.
            continue;
        } else {
            return false;
        }
    }

    starts_with_digit && saw_digit && saw_dot
}

/// Infers tag colors, rewrites `lang`'s text bundles, and writes the changed
/// `.txt` files under `out_dir`, printing one line per file written.
pub fn run<T: BufRead + Seek>(
    dbs: &mut [Database<T>],
    out_dir: &Path,
    lang: &str,
    damage_colors: DamageColors,
    user_palette: &UserPalette,
) {
    let colors = color_map(dbs, user_palette);

    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("Could not create {}: {e}", out_dir.display());
        std::process::exit(1);
    }

    let base = install_path();

    for rel in text_arcs(lang) {
        let Ok(mut arc) = Archive::open(base.join(&rel)) else {
            continue;
        };
        let Ok(records) = arc.iter_records() else {
            continue;
        };
        for record in records.flatten() {
            // Tag text lives in the `tags*.txt` records; skip everything else.
            if !record.id.contains("tag") || !record.id.ends_with(".txt") {
                continue;
            }
            let text = String::from_utf8_lossy(&record.data);
            let (rewritten, colored) = recolor_file(&text, &colors, damage_colors, user_palette);
            if colored == 0 {
                continue;
            }
            let dest = out_dir.join(&record.id);
            // Non-EN bundles nest records under `aom/`/`fg/`; make those subdirs.
            if let Some(parent) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Could not create {}: {e}", parent.display());
                    std::process::exit(1);
                }
            }
            if let Err(e) = std::fs::write(&dest, rewritten) {
                eprintln!("Could not write {}: {e}", dest.display());
                std::process::exit(1);
            }
            println!("{} ({colored} tags)", dest.display());
        }
    }
}

/// Builds the tag -> color-letter map for the DB-inferred item / affix tags.
/// Damage-type Property tags aren't listed here: they're recognized by name on
/// the fly in `recolor_file` (see `property::color_for`).
fn color_map<T: BufRead + Seek>(
    dbs: &mut [Database<T>],
    user_palette: &UserPalette,
) -> HashMap<String, char> {
    infer::infer(dbs)
        .into_iter()
        .filter_map(|(tag, info)| color::color_for(&info, user_palette).map(|c| (tag, c)))
        .collect()
}

/// Rewrites one `.txt` file's content, recoloring each line whose tag is in
/// `colors`. Returns the new content and how many tag values actually changed.
/// Comments, blank lines, and untouched tags are preserved verbatim.
fn recolor_file(
    text: &str,
    colors: &HashMap<String, char>,
    damage_colors: DamageColors,
    user_palette: &UserPalette,
) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut colored = 0usize;
    let mut class_values: Vec<(String, String)> = Vec::new();
    for segment in text.split_inclusive('\n') {
        let (line, eol) = split_eol(segment);
        if let Some((tag, value)) = line.split_once('=') {
            // DB-inferred item/affix color, else the name-derived Property color.
            if let Some(color) = colors
                .get(tag)
                .copied()
                .or_else(|| property::color_for(tag, damage_colors, user_palette))
            {
                let mut new_value = apply_color(value, color);
                // Conversion labels carry no placeholder, so the color would
                // bleed to the line's end; close it with `{^E}` (WanezGD's rule).
                if tag.contains("Conversion") {
                    new_value.push_str("{^E}");
                }
                if new_value != value {
                    colored += 1;
                }
                out.push_str(tag);
                out.push('=');
                out.push_str(&new_value);
                out.push_str(eol);
                continue;
            }
            if let Some(color) = property::color_other_for(tag, user_palette) {
                let new_value = apply_color(value, color);
                if new_value != value {
                    colored += 1;
                }
                out.push_str(tag);
                out.push('=');
                out.push_str(&new_value);
                out.push_str(eol);
                continue;
            }

            if let Some((name, class_value)) = property::text_class(tag, value) {
                // Capture localized class names and append them to class skill names later.
                class_values.push((name, class_value));
                out.push_str(tag);
                out.push('=');
                out.push_str(value);
                out.push_str(eol);
                continue;
            }

            if let Some(suffix) = property::text_for(tag, &class_values) {
                let new_value = format!("{value} {suffix}");
                if new_value != value {
                    colored += 1;
                }
                out.push_str(tag);
                out.push('=');
                out.push_str(&new_value);
                out.push_str(eol);
                continue;
            }
        }
        out.push_str(segment);
    }
    (out, colored)
}

/// Splits a trailing `\n` or `\r\n` off a line segment, returning (body, eol).
fn split_eol(segment: &str) -> (&str, &str) {
    if let Some(body) = segment.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = segment.strip_suffix('\n') {
        (body, "\n")
    } else {
        (segment, "")
    }
}

/// Bakes color code `color` into a tag value, porting WanezGD's placement
/// cascade (SRainbowFilter.js `ApplyColorInSourceData`). The cascade handles
/// values that already contain codes or structural prefixes; the common case
/// (a plain name) just gets the code prepended.
fn apply_color(value: &str, color: char) -> String {
    let cc = format!("{{^{}}}", color.to_ascii_uppercase());

    // `{^E}`/`{^S}` are placeholders the source uses to mark where the name's
    // color should go: replace the placeholder, then restore it at the end so
    // any trailing text returns to its original (brown/value) color.
    if value.contains("{^E}") {
        return format!("{}{{^E}}", value.replacen("{^E}", &cc, 1));
    }
    if value.contains("{^S}") {
        return format!("{}{{^S}}", value.replacen("{^S}", &cc, 1));
    }
    // An existing inline code: overwrite every code with ours.
    if has_color_code(value) {
        return replace_color_codes(value, &cc);
    }
    // A `[header]` prefix (optionally `$`-quoted): inject after the bracket.
    if value.starts_with('[') || value.starts_with("$[") {
        return insert_after_brackets(value, &cc);
    }
    // A leading `$` (no-translate marker): inject right after it.
    if let Some(rest) = value.strip_prefix('$') {
        return format!("${cc}{rest}");
    }
    // A `|n` formatting prefix: inject after the `|n`.
    if value.starts_with('|') {
        return insert_after_bar_digit(value, &cc);
    }
    // Plain name: prepend, provided there's actually a letter to color. Uses
    // Unicode `is_alphabetic` so non-Latin names (CJK, Cyrillic, …) also match.
    if value.chars().any(|c| c.is_alphabetic()) {
        return format!("{cc}{value}");
    }
    value.to_string()
}

/// Whether a `{^X}` (X = a single letter) inline code appears anywhere in `s`.
fn has_color_code(s: &str) -> bool {
    let b = s.as_bytes();
    (0..b.len()).any(|i| is_color_code_at(b, i))
}

fn is_color_code_at(b: &[u8], i: usize) -> bool {
    b.get(i) == Some(&b'{')
        && b.get(i + 1) == Some(&b'^')
        && b.get(i + 2).is_some_and(u8::is_ascii_alphabetic)
        && b.get(i + 3) == Some(&b'}')
}

/// Replaces every `{^X}` inline code in `s` with `cc`. Slices are taken at the
/// code's ASCII boundaries, so non-ASCII text in between is preserved intact.
fn replace_color_codes(s: &str, cc: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        if is_color_code_at(b, i) {
            out.push_str(&s[last..i]);
            out.push_str(cc);
            i += 4;
            last = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[last..]);
    out
}

/// Inserts `cc` after each `[letters]` header segment.
fn insert_after_brackets(s: &str, cc: &str) -> String {
    let mut out = String::with_capacity(s.len() + cc.len());
    let mut chars = s.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        out.push(c);
        if c == ']' && out[..out.len() - 1].ends_with(|c: char| c.is_ascii_alphabetic()) {
            // Only after a `[...]` that held letters; cheap check: the char
            // before `]` was a letter.
            if let Some(open) = out.rfind('[') {
                if out[open + 1..out.len() - 1]
                    .chars()
                    .all(|c| c.is_ascii_alphabetic())
                {
                    out.push_str(cc);
                }
            }
        }
    }
    out
}

/// Inserts `cc` after each `|n` (pipe + digit) formatting marker.
fn insert_after_bar_digit(s: &str, cc: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len() + cc.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        if b[i] == b'|' && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
            out.push_str(&s[last..i + 2]);
            out.push_str(cc);
            i += 2;
            last = i;
        } else {
            i += 1;
        }
    }
    out.push_str(&s[last..]);
    out
}
