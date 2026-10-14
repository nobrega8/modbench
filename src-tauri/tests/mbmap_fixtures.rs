//! Checks the `.mbmap` parser against the shared fixtures in mbmap-library
//! (vendor/mbmap-library/tests), the same ones its Python validator uses.

use std::collections::BTreeMap;
use std::path::PathBuf;

use modbench_lib::mbmap::{self, Severity};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/mbmap-library/tests")
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

#[test]
fn parser_matches_shared_fixtures() {
    let dir = fixtures_dir();
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
    assert!(expected.len() >= 30, "fixtures look incomplete");

    let mut failures = Vec::new();
    for (name, entry) in &expected {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let parsed = mbmap::parse(&bytes);
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
}

#[test]
fn library_examples_are_valid() {
    let examples = fixtures_dir().join("../examples");
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

/// Every map of the library reads the same after conversion to version 2,
/// with the same warnings.
#[test]
fn library_maps_survive_conversion_to_version_2() {
    let maps = fixtures_dir().join("../maps");
    let mut count = 0;
    for vendor in std::fs::read_dir(&maps).unwrap() {
        let vendor = vendor.unwrap().path();
        if !vendor.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(vendor).unwrap() {
            let path = file.unwrap().path();
            let parsed = mbmap::parse(&std::fs::read(&path).unwrap());
            let Some(map) = parsed.map.clone() else {
                panic!("{}: {:?}", path.display(), parsed.diagnostics);
            };
            let text = mbmap::to_string(&map);
            let again = mbmap::parse(text.as_bytes());
            assert_eq!(again.version, Some(2));
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

/// The library's version 2 prototype (written by its tools/migrate.py) reads
/// as the same map as the version 1 example it was made from.
#[test]
fn library_prototype_matches_its_version_1_source() {
    let dir = fixtures_dir().join("../examples/example-corp");
    let v2 = dir.join("em-100.mbmap.json");
    if !v2.exists() {
        return; // library checkout from before version 2
    }
    let read = |p: PathBuf| mbmap::parse(&std::fs::read(p).unwrap()).map.unwrap();
    assert_eq!(read(v2), read(dir.join("em-100.mbmap")));
}
