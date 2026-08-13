//! Opening the Grim Dawn `.arz` databases and iterating records.
//!
//! Adapted from the consumption patterns in `~/gddb/src/util.rs`.

use std::fs::{File, canonicalize};
use std::io::{BufRead, BufReader, Seek};
use std::path::PathBuf;

use blake3::Hasher;
use lib_gddb::arz::{Database, RawRecord, Record};

/// Relative paths to the base game + expansion databases, in load order.
const DBS: [&str; 4] = [
    "database/database.arz",
    "gdx1/database/GDX1.arz",
    "gdx2/database/GDX2.arz",
    "gdx3/database/GDX3.arz",
];

/// Resolves `GRIM_DAWN_INSTALL_PATH` to a canonical install directory.
pub fn install_path() -> PathBuf {
    let raw = std::env::var_os("GRIM_DAWN_INSTALL_PATH").unwrap_or_else(|| {
        eprintln!("Please set GRIM_DAWN_INSTALL_PATH");
        std::process::exit(1);
    });
    canonicalize(PathBuf::from(&raw)).unwrap_or_else(|e| {
        eprintln!("Could not resolve GRIM_DAWN_INSTALL_PATH={:?}: {e}", raw);
        std::process::exit(1);
    })
}

/// Opens every available database. Missing expansion DBs are skipped.
pub fn open_all() -> Vec<Database<BufReader<File>>> {
    let base = install_path();
    let dbs: Vec<_> = DBS
        .iter()
        .filter_map(|rel| Database::open(base.join(rel)).ok())
        .collect();
    if dbs.is_empty() {
        eprintln!("Could not read any database files under {}", base.display());
        std::process::exit(1);
    }
    dbs
}

/// Computes a combined content hash across all available game database files.
///
/// The hash is based on bytes from the same ARZ files used for data inference,
/// in load order, and includes each relative path as a separator/input.
pub fn database_hash() -> String {
    let base = install_path();
    let mut hasher = Hasher::new();
    let mut saw_any = false;

    for rel in DBS {
        let path = base.join(rel);
        if !path.exists() {
            continue;
        }
        saw_any = true;
        hasher.update(rel.as_bytes());
        hasher.update(&[0]);

        let mut f = File::open(&path).unwrap_or_else(|e| {
            eprintln!("Could not open database file {}: {e}", path.display());
            std::process::exit(1);
        });
        std::io::copy(&mut f, &mut hasher).unwrap_or_else(|e| {
            eprintln!("Could not read database file {}: {e}", path.display());
            std::process::exit(1);
        });
        hasher.update(&[0xFF]);
    }

    if !saw_any {
        eprintln!("Could not read any database files under {}", base.display());
        std::process::exit(1);
    }

    hasher.finalize().to_hex().to_string()
}

/// Best-effort Steam build id lookup for Grim Dawn (App ID 219990).
///
/// Returns None when the game was not installed via Steam, the manifest file
/// is not reachable from the install path, or the build id key is missing.
pub fn steam_build_id() -> Option<String> {
    let install = install_path();
    let mut candidates = Vec::new();

    // Typical layout: <steam root>/steamapps/common/Grim Dawn
    if let Some(common_dir) = install.parent() {
        if common_dir
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("common"))
        {
            if let Some(steamapps_dir) = common_dir.parent() {
                candidates.push(steamapps_dir.join("appmanifest_219990.acf"));
            }
        }
    }

    // Also try any ancestor that is itself a steamapps directory.
    for anc in install.ancestors() {
        if anc
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("steamapps"))
        {
            candidates.push(anc.join("appmanifest_219990.acf"));
        }
    }

    for manifest in candidates {
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        if let Some(v) = parse_acf_value(&text, "buildid") {
            return Some(v.to_string());
        }
    }

    None
}

fn parse_acf_value<'a>(acf: &'a str, key: &str) -> Option<&'a str> {
    for line in acf.lines() {
        let parts: Vec<_> = line.trim().split('"').collect();
        if parts.len() >= 4 && parts[1] == key {
            return Some(parts[3]);
        }
    }
    None
}

/// Resolves every record whose id satisfies `keep`, across all databases.
///
/// Filtering on the (cheap) record id before resolving avoids decompressing
/// records we don't care about.
pub fn iter_records<T: BufRead + Seek>(
    dbs: &mut [Database<T>],
    keep: impl Fn(&str) -> bool + Copy,
) -> Vec<Record> {
    let mut out = Vec::new();
    for db in dbs.iter_mut() {
        let raws: Vec<RawRecord> = db
            .iter_records()
            .expect("iter_records")
            .collect::<Result<_, _>>()
            .expect("collect raw records");
        for raw in raws {
            let Ok(id) = db.record_id(&raw) else { continue };
            if keep(&id) {
                if let Ok(record) = db.resolve(raw) {
                    out.push(record);
                }
            }
        }
    }
    out
}
