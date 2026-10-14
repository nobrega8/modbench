//! The device library: maps kept in the app's data folder as
//! `<vendor>/<model>.mbmap.json` (format version 2), the layout of the
//! mbmap-library repo. Maps are stored in version 2 whatever version they
//! came in, and `upgrade` converts the version 1 files of older releases.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::mbmap::{self, DeviceMap, Parsed, Severity};

#[derive(Debug, Clone, Serialize)]
pub struct LibraryEntry {
    pub path: String,
    pub vendor: String,
    pub model: String,
    pub description: Option<String>,
    pub vendor_website: Option<String>,
    /// `x_status` of the map, e.g. "unverified" for converted maps.
    pub status: Option<String>,
    pub registers: usize,
    pub warnings: usize,
    /// Why the file cannot be used, if it has errors.
    pub error: Option<String>,
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

/// Every map in the library, sorted by vendor and model.
pub fn list(root: &Path) -> std::io::Result<Vec<LibraryEntry>> {
    let mut entries = Vec::new();
    if !root.exists() {
        return Ok(entries);
    }
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
            let bytes = std::fs::read(&path)?;
            let parsed = mbmap::parse(&bytes);
            let warnings = parsed
                .diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Warning)
                .count();
            let error = parsed.map.is_none().then(|| error_summary(&parsed));
            let entry = match parsed.map {
                Some(map) => LibraryEntry {
                    path: path.display().to_string(),
                    vendor: map.meta.vendor.clone(),
                    model: map.meta.model.clone(),
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
                    description: None,
                    vendor_website: None,
                    status: None,
                    registers: 0,
                    warnings,
                    error,
                },
            };
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
pub fn store(root: &Path, bytes: &[u8]) -> Result<String, String> {
    let map = read_valid(bytes)?;
    let target = path_for(root, &map);
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&target, mbmap::to_string(&map)).map_err(|e| e.to_string())?;
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
                .and_then(|bytes| store(root, &bytes));
            ImportResult::new(source, outcome)
        })
        .collect()
}

/// Deletes a map, refusing anything outside the library folder.
pub fn delete(root: &Path, path: &str) -> std::io::Result<()> {
    let target = std::fs::canonicalize(path)?;
    let root = std::fs::canonicalize(root)?;
    if !target.starts_with(&root) || !is_map(&target) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "only maps inside the library can be deleted",
        ));
    }
    std::fs::remove_file(&target)?;
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
        store(&root, MAP.as_bytes()).unwrap();
        assert!(!dir.join("pm-200-b.mbmap").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
