use std::path::PathBuf;
use std::{fs::OpenOptions, io::Write};

use clap::Parser;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

mod color;
mod colorize;
mod db;
mod infer;
mod keywords;
mod palette;
mod property;
mod user_palette;

use property::DamageColors;

/// Reads Grim Dawn resource files and recolors text tags to make damage
/// types and affix rarity legible at a glance.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Language to recolor
    #[arg(short, long, default_value = "en")]
    language: String,
    /// Output path
    /// [default: $GRIM_DAWN_INSTALL_PATH/settings/text_<language>/].
    #[arg(short, long)]
    out: Option<PathBuf>,
    /// Paints Pierce red, like rainbow filter, not pink.
    #[arg(long)]
    rainbow_filter_damage_colors: bool,
    /// Optional palette config file (key=value lines). Defaults to ./gdse-palette.txt if present.
    #[arg(long)]
    palette_file: Option<PathBuf>,
}

fn main() {
    let args = Args::parse();
    let lang = args.language.to_lowercase();
    let db_hash = db::database_hash();
    let steam_build_id = db::steam_build_id().unwrap_or_else(|| "unknown".to_string());
    let patch_versions = colorize::detect_patch_versions(&lang);
    let palette_file = choose_palette_file(args.palette_file.as_deref());
    let palette_load = user_palette::load(palette_file.as_deref());
    let mut dbs = db::open_all();
    let out = args.out.unwrap_or_else(|| {
        db::install_path()
            .join("settings")
            .join(format!("text_{lang}"))
    });
    let damage_colors = if args.rainbow_filter_damage_colors {
        DamageColors::RainbowFilter
    } else {
        DamageColors::Default
    };
    if let Some(source) = &palette_load.source {
        println!("Loaded custom palette from {}", source.display());
    } else {
        println!("Using default gdse palette.");
    }

    let hash_file = out.join("gdse-db-hash.txt");
    let previous = std::fs::read_to_string(&hash_file)
        .ok()
        .and_then(|log| latest_run_info(&log));
    print_change_summary(previous.as_ref(), &db_hash, &steam_build_id, &patch_versions);
    if !confirm_run() {
        println!("Canceled by user; no output files were changed.");
        return;
    }

    colorize::run(&mut dbs, &out, &lang, damage_colors, &palette_load.palette);

    let now = local_timestamp_minute();
    let patch_versions_field = if patch_versions.is_empty() {
        "unknown".to_string()
    } else {
        patch_versions.join(",")
    };
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&hash_file)
        .unwrap_or_else(|e| {
            eprintln!("Could not open {}: {e}", hash_file.display());
            std::process::exit(1);
        });
    if let Err(e) = writeln!(
        f,
        "{now} hash={db_hash} steam_build_id={steam_build_id} patch_versions={patch_versions_field}"
    ) {
        eprintln!("Could not write {}: {e}", hash_file.display());
        std::process::exit(1);
    }
}

#[derive(Debug)]
struct RunInfo {
    hash: String,
    steam_build_id: Option<String>,
    patch_versions: Option<String>,
}

fn latest_run_info(log: &str) -> Option<RunInfo> {
    for line in log.lines().rev() {
        let mut hash = None;
        let mut steam_build_id = None;
        let mut patch_versions = None;

        for token in line.split_whitespace() {
            if let Some(v) = token.strip_prefix("hash=") {
                hash = Some(v.to_string());
            } else if let Some(v) = token.strip_prefix("steam_build_id=") {
                steam_build_id = Some(v.to_string());
            } else if let Some(v) = token.strip_prefix("patch_versions=") {
                patch_versions = Some(v.to_string());
            }
        }

        if let Some(hash) = hash {
            return Some(RunInfo {
                hash,
                steam_build_id,
                patch_versions,
            });
        }

        // Backward compatibility with old lines: "YYYY-MM-DD HH:MM <hash>"
        let tokens: Vec<_> = line.split_whitespace().collect();
        if tokens.len() >= 3 {
            return Some(RunInfo {
                hash: tokens[2].to_string(),
                steam_build_id: None,
                patch_versions: None,
            });
        }
    }
    None
}

fn print_change_summary(
    previous: Option<&RunInfo>,
    current_hash: &str,
    current_build_id: &str,
    current_patch_versions: &[String],
) {
    let current_patch_field = if current_patch_versions.is_empty() {
        "unknown".to_string()
    } else {
        current_patch_versions.join(",")
    };

    if let Some(prev) = previous {
        let hash_changed = prev.hash != current_hash;
        let prev_build = prev.steam_build_id.as_deref().unwrap_or("unknown");
        let prev_patch = prev.patch_versions.as_deref().unwrap_or("unknown");
        let build_changed = prev_build != current_build_id;
        let patch_changed = prev_patch != current_patch_field;

        if hash_changed || build_changed || patch_changed {
            println!("Changes detected since last run.");
        } else {
            println!("No changes detected since last run.");
        }
        println!("Steam build id: last={prev_build} current={current_build_id}");
        println!("Patch version label(s): last={prev_patch} current={current_patch_field}");
    } else {
        println!("No previous run marker found.");
        println!("Steam build id: last=unknown current={current_build_id}");
        println!("Patch version label(s): last=unknown current={current_patch_field}");
    }
}

fn confirm_run() -> bool {
    confirm_yes_no("Proceed with recoloring run? [Y/N]: ")
}

fn choose_palette_file(cli_palette_file: Option<&std::path::Path>) -> Option<PathBuf> {
    if let Some(path) = cli_palette_file {
        return Some(path.to_path_buf());
    }

    if confirm_yes_no("Use default gdse palette? [Y/N]: ") {
        return None;
    }

    println!(
        "Custom palette selected. Create gdse-palette.txt in this folder, then run gdse again."
    );
    std::process::exit(0);
}

fn confirm_yes_no(prompt: &str) -> bool {
    print!("{prompt}");
    if let Err(e) = std::io::stdout().flush() {
        eprintln!("Could not flush prompt to stdout: {e}");
        std::process::exit(1);
    }

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut input) {
        eprintln!("Could not read user input: {e}");
        std::process::exit(1);
    }
    matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn local_timestamp_minute() -> String {
    // Prefer local machine time for user-facing history output.
    let now = UtcOffset::current_local_offset()
        .map(|offset| OffsetDateTime::now_utc().to_offset(offset))
        .unwrap_or_else(|_| OffsetDateTime::now_utc());
    let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]");
    now.format(&fmt)
        .unwrap_or_else(|_| "unknown-time".to_string())
}
