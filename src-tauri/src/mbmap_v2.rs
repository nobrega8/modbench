//! Reader for `.mbmap` format version 2 (JSON, SPEC-v2.md in the
//! mbmap-library repo).
//!
//! It checks what is particular to JSON (syntax, duplicate keys, JSON types,
//! exact spelling of enumerated values) and turns the document into the same
//! raw rows as the version 1 reader, so `mbmap::finish` applies one set of
//! rules to both versions.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use crate::firmware;
use crate::mbmap::{
    Checker, Loc, Meta, RawRow, Source, apply_meta, join_order, parse_i128, parse_int, parse_type,
    sentinel_range,
};

/// A JSON value that keeps the order of object keys and rejects duplicates.
enum J {
    Null,
    Bool(bool),
    Num(serde_json::Number),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl<'de> Deserialize<'de> for J {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(JVisitor)
    }
}

struct JVisitor;

impl<'de> Visitor<'de> for JVisitor {
    type Value = J;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E>(self) -> Result<J, E> {
        Ok(J::Null)
    }
    fn visit_bool<E>(self, v: bool) -> Result<J, E> {
        Ok(J::Bool(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<J, E> {
        Ok(J::Num(v.into()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<J, E> {
        Ok(J::Num(v.into()))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<J, E> {
        serde_json::Number::from_f64(v)
            .map(J::Num)
            .ok_or_else(|| E::custom("number out of range"))
    }
    fn visit_str<E>(self, v: &str) -> Result<J, E> {
        Ok(J::Str(v.to_string()))
    }
    fn visit_string<E>(self, v: String) -> Result<J, E> {
        Ok(J::Str(v))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<J, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(J::Arr(items))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<J, A::Error> {
        let mut entries: Vec<(String, J)> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(de::Error::custom(format!("duplicate key {key:?}")));
            }
            let value = map.next_value()?;
            entries.push((key, value));
        }
        Ok(J::Obj(entries))
    }
}

impl J {
    fn kind(&self) -> &'static str {
        match self {
            J::Null => "null",
            J::Bool(_) => "a boolean",
            J::Num(_) => "a number",
            J::Str(_) => "a string",
            J::Arr(_) => "an array",
            J::Obj(_) => "an object",
        }
    }

    /// Kept as text for keys this reader does not interpret (`x_...`).
    fn to_text(&self) -> String {
        match self {
            J::Str(s) => s.clone(),
            J::Null => "null".into(),
            J::Bool(b) => b.to_string(),
            J::Num(n) => n.to_string(),
            J::Arr(_) | J::Obj(_) => self.to_json(),
        }
    }

    fn to_json(&self) -> String {
        match self {
            J::Str(s) => serde_json::to_string(s).unwrap_or_default(),
            J::Arr(items) => format!(
                "[{}]",
                items.iter().map(J::to_json).collect::<Vec<_>>().join(", ")
            ),
            J::Obj(entries) => format!(
                "{{{}}}",
                entries
                    .iter()
                    .map(|(k, v)| format!(
                        "{}: {}",
                        serde_json::to_string(k).unwrap_or_default(),
                        v.to_json()
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => other.to_text(),
        }
    }
}

/// Appends a key to a JSON Pointer (RFC 6901 escaping).
fn child(pointer: &str, key: &str) -> String {
    format!("{pointer}/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn wrong_type(ck: &mut Checker, at: &str, expected: &str, got: &J) {
    ck.error(
        Loc::pointer(at),
        "bad-json-type",
        format!("expected {expected}, found {}", got.kind()),
    );
}

/// A table of raw value -> label (`enums` and inline `enum`).
fn enum_table(ck: &mut Checker, at: &str, value: &J) -> Option<Vec<(i64, String)>> {
    let J::Obj(entries) = value else {
        wrong_type(ck, at, "an object of value -> label", value);
        return None;
    };
    let mut table: Vec<(i64, String)> = Vec::new();
    for (key, label) in entries {
        let n = parse_int(key);
        let ok = match (n, label) {
            (Some(n), J::Str(l)) => !l.trim().is_empty() && !table.iter().any(|(v, _)| *v == n),
            _ => false,
        };
        if !ok {
            ck.error(
                Loc::pointer(child(at, key)),
                "bad-enum",
                format!("{key:?} is not an integer with a label, or repeats a value"),
            );
            return None;
        }
        if let (Some(n), J::Str(l)) = (n, label) {
            table.push((n, l.clone()));
        }
    }
    if table.is_empty() {
        ck.error(Loc::pointer(at), "bad-enum", "empty table".into());
        return None;
    }
    Some(table)
}

const ORDERS: [&str; 2] = ["big", "little"];

/// Reads a version 2 document into metadata and raw rows. Returns `None`
/// when it is not a JSON object with a `registers` array.
pub(crate) fn read(text: &str, ck: &mut Checker) -> Option<(Meta, Vec<RawRow>)> {
    let doc: J = match serde_json::from_str(text) {
        Ok(doc) => doc,
        Err(e) => {
            // Parsers place syntax errors differently, so the position is
            // only in the message.
            ck.error(None, "not-json", format!("not a valid JSON document: {e}"));
            return None;
        }
    };
    let J::Obj(entries) = doc else {
        ck.error(None, "not-json", "the document is not a JSON object".into());
        return None;
    };

    let mut meta = Meta::default();
    let mut seen: Vec<&str> = Vec::new();
    let mut file_order: (Option<bool>, Option<bool>) = (None, None);
    let mut registers: Option<&Vec<J>> = None;

    for (key, value) in &entries {
        let at = child("", key);
        seen.push(key);
        match (key.as_str(), value) {
            ("$schema", _) => {}
            ("mbmap", J::Num(n)) if n.as_u64() == Some(2) => {}
            ("mbmap", _) => ck.error(
                Loc::pointer(at),
                "bad-meta-value",
                format!("this reader supports mbmap 2, found {}", value.to_text()),
            ),
            ("vendor", J::Str(s)) => meta.vendor = s.clone(),
            ("model", J::Str(s)) => meta.model = s.clone(),
            (
                "vendor_website" | "description" | "firmware" | "source" | "protocol" | "parity",
                J::Str(s),
            ) => {
                // Unlike version 1, enumerated values are case-sensitive.
                let exact = match key.as_str() {
                    "protocol" => ["tcp", "rtu", "tcp+rtu"].contains(&s.as_str()),
                    "parity" => ["N", "E", "O"].contains(&s.as_str()),
                    _ => true,
                };
                if !exact || !apply_meta(&mut meta, key, s) {
                    ck.error(
                        Loc::pointer(at),
                        "bad-meta-value",
                        format!("invalid value for {key}: {s:?}"),
                    );
                }
            }
            (
                "unit_id" | "port" | "baudrate" | "databits" | "stopbits" | "address_base"
                | "max_read_registers" | "max_read_bits",
                J::Num(_) | J::Str(_),
            ) => {
                if !apply_meta(&mut meta, key, &value.to_text()) {
                    ck.error(
                        Loc::pointer(at),
                        "bad-meta-value",
                        format!("invalid value for {key}: {}", value.to_text()),
                    );
                }
            }
            ("byte_order" | "word_order", J::Str(s)) if ORDERS.contains(&s.as_str()) => {
                let big = Some(s == "big");
                if key == "byte_order" {
                    file_order.0 = big;
                } else {
                    file_order.1 = big;
                }
            }
            ("byte_order" | "word_order", J::Str(s)) => ck.error(
                Loc::pointer(at),
                "bad-meta-value",
                format!("{key} must be \"big\" or \"little\", not {s:?}"),
            ),
            ("enums", J::Obj(tables)) => {
                for (name, table) in tables {
                    let at = child(&at, name);
                    let valid_name = !name.is_empty()
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
                    if !valid_name {
                        ck.error(
                            Loc::pointer(&at),
                            "bad-enum",
                            format!("invalid enum name {name:?} (a-z, 0-9, _)"),
                        );
                    }
                    // A broken table is still defined, so rows using it are
                    // not reported a second time.
                    let labels = enum_table(ck, &at, table).unwrap_or_default();
                    meta.enums.push((name.clone(), labels));
                }
            }
            ("sentinels", J::Obj(tables)) => {
                for (type_name, table) in tables {
                    let at = child(&at, type_name);
                    let ty = parse_type(type_name).filter(|ty| sentinel_range(*ty).is_some());
                    let (Some(ty), J::Obj(pairs)) = (ty, table) else {
                        ck.error(
                            Loc::pointer(at),
                            "bad-sentinel",
                            format!("invalid sentinel table for {type_name:?}"),
                        );
                        continue;
                    };
                    let (lo, hi) = sentinel_range(ty).unwrap_or_default();
                    let mut values: Vec<(i128, String)> = Vec::new();
                    let bad = pairs.iter().find(|(k, l)| match (parse_i128(k), l) {
                        (Some(n), J::Str(l))
                            if (lo..=hi).contains(&n)
                                && !l.trim().is_empty()
                                && !values.iter().any(|(v, _)| *v == n) =>
                        {
                            values.push((n, l.clone()));
                            false
                        }
                        _ => true,
                    });
                    match bad {
                        Some((key, _)) => ck.error(
                            Loc::pointer(child(&at, key)),
                            "bad-sentinel",
                            format!(
                                "{key:?} is not a valid value with a label, or repeats a value"
                            ),
                        ),
                        None if values.is_empty() => {
                            ck.error(Loc::pointer(at), "bad-sentinel", "empty table".into())
                        }
                        None => meta.sentinels.push((ty, values)),
                    }
                }
            }
            ("notes", J::Arr(items)) => {
                for (i, item) in items.iter().enumerate() {
                    match item {
                        J::Str(s) => meta.notes.push(s.clone()),
                        other => wrong_type(ck, &child(&at, &i.to_string()), "a string", other),
                    }
                }
            }
            ("registers", J::Arr(items)) => registers = Some(items),
            ("models", J::Arr(items)) => {
                if items.is_empty() {
                    ck.error(
                        Loc::pointer(&at),
                        "bad-meta-value",
                        "models cannot be empty; leave it out to use model".into(),
                    );
                }
                for (i, item) in items.iter().enumerate() {
                    let here = child(&at, &i.to_string());
                    match item {
                        J::Str(s) if s.trim().is_empty() => ck.error(
                            Loc::pointer(here),
                            "bad-meta-value",
                            "a model name cannot be empty".into(),
                        ),
                        J::Str(s) => meta.models.push(s.clone()),
                        other => wrong_type(ck, &here, "a string", other),
                    }
                }
            }
            ("revision", J::Num(n)) => match n.as_u64().filter(|r| *r >= 1) {
                Some(r) if r <= u64::from(u32::MAX) => meta.revision = Some(r as u32),
                _ => ck.error(
                    Loc::pointer(&at),
                    "bad-meta-value",
                    format!("revision must be a whole number from 1, not {n}"),
                ),
            },
            ("revised", J::Str(s)) => {
                if s.len() == 10 && valid_date(s) {
                    meta.revised = Some(s.clone());
                } else {
                    ck.error(
                        Loc::pointer(&at),
                        "bad-meta-value",
                        format!("revised must be a date YYYY-MM-DD, not {s:?}"),
                    );
                }
            }
            ("firmware_min" | "firmware_max", J::Str(s)) => {
                if firmware::parse(s).is_none() {
                    ck.error(
                        Loc::pointer(&at),
                        "bad-version",
                        format!("{key} {s:?} is not a version like 2.1"),
                    );
                } else if key == "firmware_min" {
                    meta.firmware_min = Some(s.clone());
                } else {
                    meta.firmware_max = Some(s.clone());
                }
            }
            ("sources", J::Arr(items)) => {
                for (i, item) in items.iter().enumerate() {
                    if let Some(source) = read_source(ck, &child(&at, &i.to_string()), item) {
                        meta.sources.push(source);
                    }
                }
            }
            (
                "vendor" | "model" | "vendor_website" | "description" | "firmware" | "source"
                | "protocol" | "parity" | "unit_id" | "port" | "baudrate" | "databits" | "stopbits"
                | "address_base" | "max_read_registers" | "max_read_bits" | "byte_order"
                | "word_order" | "enums" | "sentinels" | "notes" | "registers" | "models"
                | "revision" | "revised" | "firmware_min" | "firmware_max" | "sources",
                other,
            ) => wrong_type(ck, &at, expected_type(key), other),
            // Keys this version does not know (e.g. `x_...`), kept as text.
            (_, other) => meta.extra.push((key.clone(), other.to_text())),
        }
    }
    if meta.firmware_range().is_inverted() {
        ck.error(
            Loc::pointer("/firmware_min"),
            "bad-firmware-range",
            format!(
                "firmware_min {} is after firmware_max {}",
                meta.firmware_min.as_deref().unwrap_or_default(),
                meta.firmware_max.as_deref().unwrap_or_default()
            ),
        );
    }
    if file_order != (None, None) {
        meta.byte_order = Some(join_order(
            file_order.0.unwrap_or(true),
            file_order.1.unwrap_or(true),
        ));
    }

    for key in ["mbmap", "vendor", "model", "registers"] {
        if !seen.contains(&key) {
            ck.error(None, "missing-key", format!("missing required key {key:?}"));
        }
    }
    let items = registers?;

    let defaults = (file_order.0.unwrap_or(true), file_order.1.unwrap_or(true));
    let mut warned: Vec<String> = Vec::new();
    let rows = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            let at = format!("/registers/{i}");
            let J::Obj(fields) = item else {
                wrong_type(ck, &at, "an object", item);
                return None;
            };
            Some(read_register(ck, &at, fields, defaults, &mut warned))
        })
        .collect();
    Some((meta, rows))
}

fn expected_type(key: &str) -> &'static str {
    match key {
        "unit_id" | "port" | "baudrate" | "databits" | "stopbits" | "address_base"
        | "max_read_registers" | "max_read_bits" | "revision" => "an integer",
        "enums" | "sentinels" => "an object",
        "notes" | "registers" | "models" => "an array",
        "sources" => "an array of objects",
        "firmware_min" | "firmware_max" => "a version string",
        _ => "a string",
    }
}

/// `YYYY`, `YYYY-MM` or `YYYY-MM-DD`, a real calendar date.
fn valid_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let widths_ok = matches!(parts.as_slice(), [y] | [y, _] | [y, _, _] if y.len() == 4)
        && parts[1..].iter().all(|p| p.len() == 2);
    let Some(nums) = parts
        .iter()
        .map(|p| {
            p.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| p.parse::<u32>().ok())
                .flatten()
        })
        .collect::<Option<Vec<u32>>>()
    else {
        return false;
    };
    if !widths_ok {
        return false;
    }
    let (year, month, day) = (
        nums[0],
        nums.get(1).copied().unwrap_or(1),
        nums.get(2).copied().unwrap_or(1),
    );
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

/// One item of `sources` (spec section 3.3).
fn read_source(ck: &mut Checker, at: &str, value: &J) -> Option<Source> {
    let J::Obj(fields) = value else {
        ck.error(
            Loc::pointer(at),
            "bad-source",
            "a source must be an object with a title".into(),
        );
        return None;
    };
    let mut source = Source::default();
    let mut ok = true;
    let mut fail = |ck: &mut Checker, key: &str, message: String| {
        ck.error(Loc::pointer(child(at, key)), "bad-source", message);
        ok = false;
    };
    for (key, field) in fields {
        let text = match field {
            J::Str(s) => Some(s.clone()),
            _ => None,
        };
        match key.as_str() {
            "title" => match text {
                Some(s) if !s.trim().is_empty() => source.title = s,
                _ => fail(ck, key, "a source needs a non-empty title".into()),
            },
            "version" | "note" => match text {
                Some(s) if key == "version" => source.version = Some(s),
                Some(s) => source.note = Some(s),
                None => fail(ck, key, format!("{key} must be a string")),
            },
            "date" => match text {
                Some(s) if valid_date(&s) => source.date = Some(s),
                _ => fail(
                    ck,
                    key,
                    format!(
                        "date must be YYYY, YYYY-MM or YYYY-MM-DD, not {}",
                        field.to_text()
                    ),
                ),
            },
            "url" => match text {
                Some(s)
                    if (s.starts_with("http://") || s.starts_with("https://"))
                        && s.len() > s.find("//").unwrap_or(0) + 2
                        && !s.chars().any(char::is_whitespace) =>
                {
                    source.url = Some(s)
                }
                _ => fail(
                    ck,
                    key,
                    format!("url must be an http(s):// URL, not {}", field.to_text()),
                ),
            },
            _ => {}
        }
    }
    if !fields.iter().any(|(k, _)| k == "title") {
        fail(ck, "title", "a source needs a non-empty title".into());
    }
    ok.then_some(source)
}

fn read_register(
    ck: &mut Checker,
    at: &str,
    fields: &[(String, J)],
    file_order: (bool, bool),
    warned: &mut Vec<String>,
) -> RawRow {
    let mut row = RawRow {
        at: Loc::pointer(at),
        ..RawRow::default()
    };
    let mut order: (Option<bool>, Option<bool>) = (None, None);
    for (key, value) in fields {
        let here = child(at, key);
        let set = |row: &mut RawRow, text: String| {
            row.cells.insert(key.clone(), text);
        };
        match (key.as_str(), value) {
            ("name" | "group" | "unit" | "description" | "scale_factor", J::Str(s)) => {
                set(&mut row, s.clone())
            }
            ("comment", J::Str(s)) => row.comment = s.clone(),
            ("area" | "type" | "access", J::Str(s)) => {
                // Unlike version 1, enumerated values are case-sensitive.
                let exact = if key == "access" {
                    s.to_uppercase() == *s
                } else {
                    s.to_lowercase() == *s
                };
                if !exact {
                    let code = match key.as_str() {
                        "area" => "bad-area",
                        "type" => "bad-type",
                        _ => "bad-access",
                    };
                    ck.error(
                        Loc::pointer(&here),
                        code,
                        format!(
                            "{key} {s:?} must be written {}",
                            if key == "access" {
                                "in uppercase"
                            } else {
                                "in lowercase"
                            }
                        ),
                    );
                }
                set(&mut row, s.clone());
            }
            ("address" | "length" | "bit" | "bits", J::Num(_) | J::Str(_)) => {
                set(&mut row, value.to_text())
            }
            ("scale" | "offset" | "write_min" | "write_max", J::Num(n)) => {
                set(&mut row, n.to_string())
            }
            ("reference", J::Num(_) | J::Bool(_) | J::Str(_)) => set(&mut row, value.to_text()),
            ("byte_order" | "word_order", J::Str(s)) if ORDERS.contains(&s.as_str()) => {
                let big = Some(s == "big");
                if key == "byte_order" {
                    order.0 = big;
                } else {
                    order.1 = big;
                }
            }
            ("byte_order" | "word_order", J::Str(s)) => ck.error(
                Loc::pointer(&here),
                "bad-byte-order",
                format!("{key} must be \"big\" or \"little\", not {s:?}"),
            ),
            ("enum", J::Str(name)) => set(&mut row, format!("@{name}")),
            ("firmware_min" | "firmware_max" | "role", J::Str(s)) => set(&mut row, s.clone()),
            ("enum", J::Obj(_)) => {
                row.enum_table = Some(enum_table(ck, &here, value).unwrap_or_default())
            }
            (
                "name" | "group" | "unit" | "description" | "scale_factor" | "comment" | "area"
                | "type" | "access" | "address" | "length" | "bit" | "bits" | "scale" | "offset"
                | "write_min" | "write_max" | "reference" | "byte_order" | "word_order" | "enum"
                | "firmware_min" | "firmware_max" | "role",
                other,
            ) => {
                let expected = match key.as_str() {
                    "address" | "length" | "bit" | "bits" => "an integer",
                    "scale" | "offset" | "write_min" | "write_max" => "a number",
                    "reference" => "a number, boolean or string",
                    "enum" => "a table name or an object",
                    _ => "a string",
                };
                wrong_type(ck, &here, expected, other);
            }
            (_, other) => {
                if !key.starts_with("x_") && !warned.contains(key) {
                    warned.push(key.clone());
                    ck.warn(
                        Loc::pointer(&here),
                        "unknown-key",
                        format!("unknown key {key:?} (custom keys should start with 'x_')"),
                    );
                }
                row.extra.push((key.clone(), other.to_text()));
            }
        }
    }
    if order != (None, None) {
        let joined = join_order(
            order.0.unwrap_or(file_order.0),
            order.1.unwrap_or(file_order.1),
        );
        row.cells.insert("byte_order".into(), format!("{joined:?}"));
    }
    row
}
