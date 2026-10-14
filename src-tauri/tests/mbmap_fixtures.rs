//! Checks the `.mbmap` parser against the shared fixtures in mbmap-library,
//! the same ones its Python validator uses: `tests/` for format version 2
//! and `legacy/v1/tests/` for the deprecated version 1, which Modbench still
//! opens.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use modbench_lib::mbmap::{self, Severity};
use serde_json::Value;

fn library_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/mbmap-library")
}

/// (location, code): "line 5", a JSON Pointer, or "file".
type Expected = Vec<(String, String)>;

fn counted(mut items: Expected) -> BTreeMap<(String, String), usize> {
    items.sort();
    let mut map = BTreeMap::new();
    for item in items {
        *map.entry(item).or_insert(0) += 1;
    }
    map
}

fn read_expected(entry: &Value, key: &str) -> Expected {
    entry[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            let at = match &pair[0] {
                Value::Number(n) => format!("line {n}"),
                Value::String(p) => p.clone(),
                _ => "file".into(),
            };
            (at, pair[1].as_str().unwrap().to_string())
        })
        .collect()
}

/// Checks every fixture listed in `dir/expected.json`; returns how many.
fn check_suite(dir: &Path, version: u8) -> usize {
    let expected_path = dir.join("expected.json");
    let expected: BTreeMap<String, Value> = serde_json::from_str(
        &std::fs::read_to_string(&expected_path).unwrap_or_else(|e| {
            panic!(
                "{} not found ({e}); run `git submodule update --init`",
                expected_path.display()
            )
        }),
    )
    .unwrap();

    let mut failures = Vec::new();
    for (name, entry) in &expected {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let parsed = mbmap::parse(&bytes);
        if parsed.version.is_some() && parsed.version != Some(version) {
            failures.push(format!("{name}: read as version {:?}", parsed.version));
        }
        for (key, severity) in [("errors", Severity::Error), ("warnings", Severity::Warning)] {
            let got: Expected = parsed
                .diagnostics
                .iter()
                .filter(|d| d.severity == severity)
                .map(|d| (d.location(), d.code.to_string()))
                .collect();
            let want = read_expected(entry, key);
            if counted(got.clone()) != counted(want.clone()) {
                failures.push(format!(
                    "{name} {key}:\n    want {want:?}\n    got  {got:?}"
                ));
            }
        }

        let want_errors = !read_expected(entry, "errors").is_empty();
        assert_eq!(parsed.map.is_none(), want_errors, "{name}: map presence");

        // Every valid file must survive a write and a re-read unchanged.
        if let Some(map) = parsed.map {
            let text = mbmap::to_string(&map);
            let again = mbmap::parse(text.as_bytes());
            assert!(
                again.errors().next().is_none(),
                "{name}: written file has errors: {:?}\n{text}",
                again.diagnostics
            );
            assert_eq!(again.map.as_ref(), Some(&map), "{name}: round trip\n{text}");
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    expected.len()
}

#[test]
fn parser_matches_shared_fixtures() {
    let count = check_suite(&library_dir().join("tests"), 2);
    assert!(count >= 30, "fixtures look incomplete ({count})");
}

#[test]
fn parser_matches_version_1_fixtures() {
    let count = check_suite(&library_dir().join("legacy/v1/tests"), 1);
    assert!(count >= 30, "fixtures look incomplete ({count})");
}

#[test]
fn library_examples_are_valid() {
    let examples = library_dir().join("examples");
    let mut count = 0;
    for vendor in std::fs::read_dir(&examples).unwrap() {
        for file in std::fs::read_dir(vendor.unwrap().path()).unwrap() {
            let path = file.unwrap().path();
            let parsed = mbmap::parse(&std::fs::read(&path).unwrap());
            assert!(
                parsed.map.is_some(),
                "{}: {:?}",
                path.display(),
                parsed.diagnostics
            );
            count += 1;
        }
    }
    assert!(count > 0);
}

/// Every map of the library is version 2 and survives a write and a re-read
/// with the same map and the same warnings.
#[test]
fn library_maps_round_trip() {
    let maps = library_dir().join("maps");
    let mut count = 0;
    for vendor in std::fs::read_dir(&maps).unwrap() {
        let vendor = vendor.unwrap().path();
        if !vendor.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(vendor).unwrap() {
            let path = file.unwrap().path();
            let parsed = mbmap::parse(&std::fs::read(&path).unwrap());
            assert_eq!(parsed.version, Some(2), "{}", path.display());
            let Some(map) = parsed.map.clone() else {
                panic!("{}: {:?}", path.display(), parsed.diagnostics);
            };
            let again = mbmap::parse(mbmap::to_string(&map).as_bytes());
            let codes = |p: &mbmap::Parsed| {
                let mut c: Vec<_> = p.diagnostics.iter().map(|d| d.code).collect();
                c.sort();
                c
            };
            assert_eq!(codes(&again), codes(&parsed), "{}", path.display());
            assert_eq!(again.map.as_ref(), Some(&map), "{}", path.display());
            count += 1;
        }
    }
    assert!(count > 100, "only {count} maps found");
}

/// The library's version 2 example (written by its tools/migrate.py) reads
/// as the same map as the version 1 example it was made from.
#[test]
fn library_example_matches_its_version_1_source() {
    let lib = library_dir();
    let read = |p: PathBuf| mbmap::parse(&std::fs::read(p).unwrap()).map.unwrap();
    assert_eq!(
        read(lib.join("examples/example-corp/em-100.mbmap.json")),
        read(lib.join("legacy/v1/examples/example-corp/em-100.mbmap"))
    );
}
