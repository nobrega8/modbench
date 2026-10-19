//! The device library: maps kept in the app's data folder as
//! `<vendor>/<model>.mbmap.json` (format version 2), the layout of the
//! mbmap-library repo. Maps are stored in version 2 whatever version they
//! came in, and `upgrade` converts the version 1 files of older releases.
//!
//! `.origins.json` in the library folder remembers where each map came from
//! (the online library, or a file the user added) and a hash of what was
//! stored, so the Library can show which maps the user added or changed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::mbmap::{self, DeviceMap, Parsed, Severity};

#[derive(Debug, Clone, Serialize)]
pub struct LibraryEntry {
    pub path: String,
    pub vendor: String,
    pub model: String,
    /// Every model name or code the map applies to (`models`); searched too.
    pub models: Vec<String>,
    pub description: Option<String>,
    pub vendor_website: Option<String>,
    /// `x_status` of the map: "verified", "official" or "unofficial" (older maps say "unverified" for "official").
    pub status: Option<String>,
    pub registers: usize,
    pub warnings: usize,
    /// Why the file cannot be used, if it has errors.
    pub error: Option<String>,
    /// Where the map came from: "online" or "imported" (added by the user);
    /// none for maps stored by older releases.
    pub origin: Option<Origin>,
    /// The file changed since it was stored.
    pub edited: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Online,
    Imported,
}

/// What the library remembers of a stored map.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Record {
    origin: Origin,
    /// FNV-1a hash of the bytes that were stored, as hex.
    hash: String,
}

const ORIGINS: &str = ".origins.json";

/// FNV-1a 64-bit: stable across builds, enough to notice an edited file.
fn hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// The key of a map in the origins file: its path inside the library.
fn key(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

fn read_origins(root: &Path) -> BTreeMap<String, Record> {
    std::fs::read(root.join(ORIGINS))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn write_origins(root: &Path, origins: &BTreeMap<String, Record>) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(origins).map_err(std::io::Error::other)?;
    std::fs::write(root.join(ORIGINS), text)
}

/// Remembers (or, with `None`, forgets) where the map at `path` came from.
fn remember(root: &Path, path: &Path, origin: Option<(Origin, &[u8])>) -> std::io::Result<()> {
    let Some(k) = key(root, path) else {
        return Ok(());
    };
    let mut origins = read_origins(root);
    match origin {
        Some((origin, bytes)) => {
            origins.insert(
                k,
                Record {
                    origin,
                    hash: hash(bytes),
                },
            );
        }
        None => {
            origins.remove(&k);
        }
    }
    write_origins(root, &origins)
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub source: String,
    /// Where the map was stored, if it was imported.
    pub stored: Option<String>,
    pub error: Option<String>,
}

/// Lowercase slug with `-` separators, as used for library file names.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "unnamed".into()
    } else {
        out
    }
}

/// The first few errors of a file that could not be read, in one line.
fn error_summary(parsed: &Parsed) -> String {
    parsed
        .errors()
        .take(3)
        .map(|d| match (d.line, &d.pointer) {
            (None, None) => d.message.clone(),
            _ => format!("{}: {}", d.location(), d.message),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn read_valid(bytes: &[u8]) -> Result<DeviceMap, String> {
    let parsed = mbmap::parse(bytes);
    let summary = error_summary(&parsed);
    parsed.map.ok_or(summary)
}

/// Path a map gets inside the library.
pub fn path_for(root: &Path, map: &DeviceMap) -> PathBuf {
    root.join(slug(&map.meta.vendor))
        .join(format!("{}.mbmap.json", slug(&map.meta.model)))
}

fn is_map(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(mbmap::is_map_file)
}

/// Converts the valid version 1 maps of the library to version 2, in place.
/// Files with errors are left as they are. Returns how many were converted.
pub fn upgrade(root: &Path) -> std::io::Result<usize> {
    let mut converted = 0;
    if !root.exists() {
        return Ok(0);
    }
    for vendor in std::fs::read_dir(root)? {
        let vendor = vendor?.path();
        if !vendor.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&vendor)? {
            let path = file?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("mbmap") {
                continue;
            }
            let parsed = mbmap::parse(&std::fs::read(&path)?);
            if let (Some(1), Some(map)) = (parsed.version, parsed.map) {
                let target = path.with_extension("mbmap.json");
                std::fs::write(&target, mbmap::to_string(&map))?;
                std::fs::remove_file(&path)?;
                converted += 1;
            }
        }
    }
    Ok(converted)
}

/// The entry of the map file at `path` (in the folder of `vendor`), without
/// what the origins file says about it.
fn entry_of(vendor: &Path, path: &Path, bytes: &[u8]) -> LibraryEntry {
    let parsed = mbmap::parse(bytes);
    let warnings = parsed
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Warning)
        .count();
    let error = parsed.map.is_none().then(|| error_summary(&parsed));
    match parsed.map {
        Some(map) => LibraryEntry {
            path: path.display().to_string(),
            vendor: map.meta.vendor.clone(),
            model: map.meta.model.clone(),
            models: map.meta.models.clone(),
            description: map.meta.description.clone(),
            vendor_website: map.meta.vendor_website.clone(),
            status: map
                .meta
                .extra
                .iter()
                .find(|(k, _)| k == "x_status")
                .map(|(_, v)| v.clone()),
            registers: map.registers.len(),
            warnings,
            error: None,
            origin: None,
            edited: false,
        },
        None => LibraryEntry {
            path: path.display().to_string(),
            vendor: vendor
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            model: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .map(|n| {
                    n.trim_end_matches(".json")
                        .trim_end_matches(".mbmap")
                        .into()
                })
                .unwrap_or_default(),
            models: Vec::new(),
            description: None,
            vendor_website: None,
            status: None,
            registers: 0,
            warnings,
            error,
            origin: None,
            edited: false,
        },
    }
}

/// What `list` found in a file the last time, and the file's size and
/// modification time then: a file that has not changed is not read and parsed
/// again (the whole library takes close to a second to parse).
struct Cached {
    modified: Option<SystemTime>,
    len: u64,
    hash: String,
    entry: LibraryEntry,
}

static CACHE: Mutex<BTreeMap<PathBuf, Cached>> = Mutex::new(BTreeMap::new());

/// Every map in the library, sorted by vendor and model.
pub fn list(root: &Path) -> std::io::Result<Vec<LibraryEntry>> {
    let mut entries = Vec::new();
    if !root.exists() {
        return Ok(entries);
    }
    let origins = read_origins(root);
    for vendor in std::fs::read_dir(root)? {
        let vendor = vendor?.path();
        if !vendor.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(&vendor)? {
            let path = file?.path();
            if !is_map(&path) {
                continue;
            }
            let meta = std::fs::metadata(&path)?;
            let (modified, len) = (meta.modified().ok(), meta.len());
            let cached = {
                let cache = CACHE.lock().unwrap();
                cache
                    .get(&path)
                    .filter(|c| c.modified == modified && c.len == len)
                    .map(|c| (c.entry.clone(), c.hash.clone()))
            };
            let (mut entry, file_hash) = match cached {
                Some(hit) => hit,
                None => {
                    let bytes = std::fs::read(&path)?;
                    let entry = entry_of(&vendor, &path, &bytes);
                    let file_hash = hash(&bytes);
                    // A file written a moment ago may be written again within
                    // the clock's resolution, with the same size: it is not kept.
                    let settled = modified.is_some_and(|m| {
                        SystemTime::now()
                            .duration_since(m)
                            .is_ok_and(|age| age > Duration::from_secs(2))
                    });
                    if settled {
                        CACHE.lock().unwrap().insert(
                            path.clone(),
                            Cached {
                                modified,
                                len,
                                hash: file_hash.clone(),
                                entry: entry.clone(),
                            },
                        );
                    }
                    (entry, file_hash)
                }
            };
            let record = key(root, &path).and_then(|k| origins.get(&k));
            entry.origin = record.map(|r| r.origin);
            entry.edited = record.is_some_and(|r| r.hash != file_hash);
            entries.push(entry);
        }
    }
    entries.sort_by(|a, b| {
        (a.vendor.to_lowercase(), a.model.to_lowercase())
            .cmp(&(b.vendor.to_lowercase(), b.model.to_lowercase()))
    });
    Ok(entries)
}

/// Stores a valid map of either version, as version 2; returns where it was
/// stored. A map with the same vendor and model replaces the stored one.
pub fn store(root: &Path, bytes: &[u8], origin: Origin) -> Result<String, String> {
    let map = read_valid(bytes)?;
    write_map(root, &map, origin)
}

fn write_map(root: &Path, map: &DeviceMap, origin: Origin) -> Result<String, String> {
    let target = path_for(root, map);
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = mbmap::to_string(map);
    std::fs::write(&target, &text).map_err(|e| e.to_string())?;
    remember(root, &target, Some((origin, text.as_bytes()))).map_err(|e| e.to_string())?;
    // The version 1 copy an older release stored for the same model.
    let legacy = target.with_extension("");
    if legacy.exists() {
        std::fs::remove_file(&legacy).map_err(|e| e.to_string())?;
    }
    Ok(target.display().to_string())
}

impl ImportResult {
    pub fn new(source: &str, outcome: Result<String, String>) -> Self {
        match outcome {
            Ok(stored) => ImportResult {
                source: source.to_string(),
                stored: Some(stored),
                error: None,
            },
            Err(e) => ImportResult {
                source: source.to_string(),
                stored: None,
                error: Some(e),
            },
        }
    }
}

/// Copies valid map files into the library.
pub fn import(root: &Path, sources: &[String]) -> Vec<ImportResult> {
    sources
        .iter()
        .map(|source| {
            let outcome = std::fs::read(source)
                .map_err(|e| e.to_string())
                .and_then(|bytes| store(root, &bytes, Origin::Imported));
            ImportResult::new(source, outcome)
        })
        .collect()
}

/// The map at `path`, refused unless it is a map inside the library folder.
pub fn library_map(root: &Path, path: &str) -> std::io::Result<PathBuf> {
    let target = std::fs::canonicalize(path)?;
    let root = std::fs::canonicalize(root)?;
    if !target.starts_with(&root) || !is_map(&target) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "only maps inside the library can be changed",
        ));
    }
    Ok(target)
}

/// Copies a library map as a new map of the same vendor, named
/// "<model> copy" (or "copy 2", "copy 3"…). Returns where it was stored.
pub fn duplicate(root: &Path, path: &str) -> Result<String, String> {
    let source = library_map(root, path).map_err(|e| e.to_string())?;
    let mut map = read_valid(&std::fs::read(&source).map_err(|e| e.to_string())?)?;
    let model = map.meta.model.clone();
    let mut n = 1;
    loop {
        map.meta.model = if n == 1 {
            format!("{model} copy")
        } else {
            format!("{model} copy {n}")
        };
        if !path_for(root, &map).exists() {
            break;
        }
        n += 1;
    }
    // The copy is the user's: it keeps no list of other model names.
    map.meta.models = Vec::new();
    write_map(root, &map, Origin::Imported)
}

/// Deletes a map, refusing anything outside the library folder.
pub fn delete(root: &Path, path: &str) -> std::io::Result<()> {
    let target = library_map(root, path)?;
    let root = std::fs::canonicalize(root)?;
    std::fs::remove_file(&target)?;
    remember(&root, &target, None)?;
    // Drop the vendor folder once it is empty.
    if let Some(dir) = target.parent()
        && dir != root
        && std::fs::read_dir(dir)?.next().is_none()
    {
        std::fs::remove_dir(dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "@mbmap=1\n@vendor=Acme Meters\n@model=PM 200/B\nname,area,address,type\nV,input,0,float32\n";

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("modbench-lib-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("Schneider Electric"), "schneider-electric");
        assert_eq!(slug("  PM 200/B  "), "pm-200-b");
        assert_eq!(slug("--"), "unnamed");
    }

    #[test]
    fn entries_carry_every_model() {
        // The library's example lists EM-200 and EM-200-MID in `models`; a
        // search for either must find it (issue #122).
        let root = temp_root("models");
        let src = root.join("em-200.mbmap.json");
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../vendor/mbmap-library/examples/example-corp/em-200.mbmap.json"
            ),
            &src,
        )
        .unwrap();
        let lib = root.join("library");
        import(&lib, &[src.display().to_string()]);
        let entries = list(&lib).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].models, ["EM-200", "EM-200-MID"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn import_list_delete() {
        let root = temp_root("roundtrip");
        let src = root.join("incoming.mbmap");
        std::fs::write(&src, MAP).unwrap();
        let bad = root.join("bad.mbmap");
        std::fs::write(&bad, "@mbmap=1\n").unwrap();
        let lib = root.join("library");

        let results = import(
            &lib,
            &[src.display().to_string(), bad.display().to_string()],
        );
        assert!(
            results[0]
                .stored
                .as_deref()
                .unwrap()
                .ends_with("pm-200-b.mbmap.json")
        );
        assert!(results[1].error.is_some());

        let entries = list(&lib).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            (
                entries[0].vendor.as_str(),
                entries[0].model.as_str(),
                entries[0].registers
            ),
            ("Acme Meters", "PM 200/B", 1)
        );

        // Files outside the library are never deleted.
        assert!(delete(&lib, &src.display().to_string()).is_err());
        assert!(src.exists());

        // Imported, not edited; editing the file shows it.
        assert_eq!(entries[0].origin, Some(Origin::Imported));
        assert!(!entries[0].edited);
        let stored = std::fs::read_to_string(&entries[0].path).unwrap();
        std::fs::write(&entries[0].path, stored.replace("PM 200/B", "PM 200/C")).unwrap();
        assert!(list(&lib).unwrap()[0].edited);
        std::fs::write(&entries[0].path, stored).unwrap();

        // A duplicate is a new imported map of the same vendor.
        let copy = duplicate(&lib, &entries[0].path).unwrap();
        assert!(copy.ends_with("pm-200-b-copy.mbmap.json"));
        assert!(
            duplicate(&lib, &entries[0].path)
                .unwrap()
                .ends_with("pm-200-b-copy-2.mbmap.json")
        );
        let all = list(&lib).unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().any(|e| e.model == "PM 200/B copy"
            && e.origin == Some(Origin::Imported)
            && !e.edited));
        delete(&lib, &copy).unwrap();
        for e in list(&lib)
            .unwrap()
            .iter()
            .filter(|e| e.model.contains("copy"))
        {
            delete(&lib, &e.path).unwrap();
        }

        delete(&lib, &entries[0].path).unwrap();
        assert!(list(&lib).unwrap().is_empty());
        assert!(!lib.join("acme-meters").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn version_1_maps_are_upgraded() {
        let root = temp_root("upgrade");
        let dir = root.join("acme-meters");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pm-200-b.mbmap"), MAP).unwrap();
        std::fs::write(dir.join("broken.mbmap"), "@mbmap=1\n").unwrap();

        assert_eq!(upgrade(&root).unwrap(), 1);
        assert!(!dir.join("pm-200-b.mbmap").exists());
        let v2 = std::fs::read(dir.join("pm-200-b.mbmap.json")).unwrap();
        assert_eq!(mbmap::parse(&v2).version, Some(2));
        // A file with errors stays as it is, and is still listed.
        assert!(dir.join("broken.mbmap").exists());
        let entries = list(&root).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|e| e.model == "broken" && e.error.is_some())
        );

        // Storing the same model again replaces the old copy, in version 2.
        std::fs::write(dir.join("pm-200-b.mbmap"), MAP).unwrap();
        store(&root, MAP.as_bytes(), Origin::Imported).unwrap();
        assert!(!dir.join("pm-200-b.mbmap").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
