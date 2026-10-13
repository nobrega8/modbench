//! Checks the `.mbmap` parser against the shared fixtures in mbmap-library
//! (vendor/mbmap-library/tests), the same ones its Python validator uses.

use std::collections::BTreeMap;
use std::path::PathBuf;

use modbench_lib::mbmap::{self, Severity};
use serde_json::Value;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../vendor/mbmap-library/tests")
}

type Expected = Vec<(Option<usize>, String)>;

fn counted(mut items: Expected) -> BTreeMap<(Option<usize>, String), usize> {
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
            let line = pair[0].as_u64().map(|n| n as usize);
            (line, pair[1].as_str().unwrap().to_string())
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
                .map(|d| (d.line, d.code.to_string()))
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
