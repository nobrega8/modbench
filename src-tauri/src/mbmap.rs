//! Reader and writer for `.mbmap` files (format version 1).
//!
//! The rules and diagnostic codes follow SPEC.md in the mbmap-library repo;
//! `tests/mbmap_fixtures.rs` checks this module against its shared fixtures.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::codec::{ByteOrder, DataType, Value};
use crate::modbus::Parity;
use crate::planner::Area;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// 1-based line, or `None` for problems with the whole file.
    pub line: Option<usize>,
    /// Stable code from SPEC §7, e.g. `missing-bit`.
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    #[serde(rename = "tcp")]
    Tcp,
    #[serde(rename = "rtu")]
    Rtu,
    #[serde(rename = "tcp+rtu")]
    TcpRtu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Access {
    R,
    W,
    RW,
}

impl Access {
    pub fn readable(self) -> bool {
        matches!(self, Access::R | Access::RW)
    }

    pub fn writable(self) -> bool {
        matches!(self, Access::W | Access::RW)
    }
}

/// File metadata. Optional values keep whether they were present, so a map
/// is written back as it was read; the accessors apply the spec defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub vendor: String,
    pub model: String,
    pub vendor_website: Option<String>,
    pub description: Option<String>,
    pub firmware: Option<String>,
    pub source: Option<String>,
    pub protocol: Option<Protocol>,
    pub unit_id: Option<u8>,
    pub port: Option<u16>,
    pub baudrate: Option<u32>,
    pub parity: Option<Parity>,
    pub data_bits: Option<u8>,
    pub stop_bits: Option<u8>,
    pub byte_order: Option<ByteOrder>,
    pub address_base: Option<u8>,
    pub max_read_registers: Option<u16>,
    pub max_read_bits: Option<u16>,
    /// Named translation tables (`@enum_<name>`), in file order.
    pub enums: Vec<(String, Vec<(i64, String)>)>,
    /// Sentinel tables (`@sentinel_<type>`): reserved raw values per type.
    pub sentinels: Vec<(DataType, Vec<(i128, String)>)>,
    /// Keys this version does not know (e.g. `x_...`), kept in file order.
    pub extra: Vec<(String, String)>,
}

impl Meta {
    pub fn protocol(&self) -> Protocol {
        self.protocol.unwrap_or(Protocol::TcpRtu)
    }
    pub fn unit_id(&self) -> u8 {
        self.unit_id.unwrap_or(1)
    }
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(502)
    }
    pub fn baudrate(&self) -> u32 {
        self.baudrate.unwrap_or(9600)
    }
    pub fn parity(&self) -> Parity {
        self.parity.unwrap_or(Parity::E)
    }
    pub fn data_bits(&self) -> u8 {
        self.data_bits.unwrap_or(8)
    }
    pub fn stop_bits(&self) -> u8 {
        self.stop_bits.unwrap_or(1)
    }
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order.unwrap_or_default()
    }
    pub fn address_base(&self) -> u8 {
        self.address_base.unwrap_or(0)
    }
    pub fn max_read_registers(&self) -> u16 {
        self.max_read_registers.unwrap_or(125)
    }
    pub fn max_read_bits(&self) -> u16 {
        self.max_read_bits.unwrap_or(2000)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Register {
    pub name: String,
    pub group: String,
    pub area: Area,
    /// Address on the wire (already adjusted for `@address_base`).
    pub address: u16,
    #[serde(rename = "type")]
    pub ty: DataType,
    /// Register count, only for strings.
    pub length: Option<u16>,
    pub scale: f64,
    /// Name of the register holding a power-of-ten exponent for this value
    /// (SunSpec "scale factor"): value = raw x 10^sf x scale + offset.
    #[serde(default)]
    pub scale_factor: Option<String>,
    pub offset: f64,
    pub unit: String,
    pub access: Access,
    /// Overrides the file's `@byte_order` when set.
    pub byte_order: Option<ByteOrder>,
    /// Bit index, only for bool in input/holding registers.
    pub bit: Option<u8>,
    /// Width of a `uint16` bit field starting at `bit`.
    pub bits: Option<u8>,
    /// Labels for raw values, resolved from inline pairs or a named table.
    #[serde(rename = "enum")]
    pub enum_labels: Vec<(i64, String)>,
    /// Name of the table the labels come from, when the file used `@name`.
    pub enum_ref: Option<String>,
    pub description: String,
    /// Columns this version does not know (e.g. `x_...`), by column name.
    pub extra: Vec<(String, String)>,
}

impl Register {
    /// Registers (or bits) this value takes.
    pub fn count(&self) -> u16 {
        match self.ty.register_count() {
            Some(0) => 1,
            Some(n) => n,
            None => self.length.unwrap_or(1),
        }
    }

    pub fn byte_order(&self, meta: &Meta) -> ByteOrder {
        self.byte_order.unwrap_or(meta.byte_order())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceMap {
    pub meta: Meta,
    pub registers: Vec<Register>,
}

impl DeviceMap {
    /// Scale factor registers that the `chosen` registers need but that are
    /// not among them, each once.
    pub fn scale_registers(&self, chosen: &[usize]) -> Vec<Register> {
        let mut out: Vec<Register> = Vec::new();
        for &i in chosen {
            let Some(name) = self.registers[i].scale_factor.as_deref() else {
                continue;
            };
            let have = chosen.iter().any(|&j| self.registers[j].name == name)
                || out.iter().any(|r| r.name == name);
            if !have && let Some(sf) = self.registers.iter().find(|r| r.name == name) {
                out.push(sf.clone());
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Parsed {
    /// The map, if the file has no errors.
    pub map: Option<DeviceMap>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Parsed {
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
    }
}

const REQUIRED_META: [&str; 3] = ["mbmap", "vendor", "model"];
const REQUIRED_COLUMNS: [&str; 4] = ["name", "area", "address", "type"];
const KNOWN_COLUMNS: [&str; 16] = [
    "name",
    "group",
    "area",
    "address",
    "type",
    "length",
    "scale",
    "scale_factor",
    "offset",
    "unit",
    "access",
    "byte_order",
    "bit",
    "bits",
    "enum",
    "description",
];

/// Decimal integer with an optional sign. Used for metadata, `bit` and `length`.
fn parse_decimal(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Decimal or `0x` hex integer, as allowed in `address` and `enum`.
fn parse_int(text: &str) -> Option<i64> {
    match text.get(..2) {
        Some("0x" | "0X") => {
            let hex = &text[2..];
            if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            i64::from_str_radix(hex, 16).ok()
        }
        _ => parse_decimal(text),
    }
}

fn decimal_in(text: &str, lo: i64, hi: i64) -> Option<i64> {
    parse_decimal(text).filter(|n| (lo..=hi).contains(n))
}

/// A domain (`se.com`) or an `http(s)://` URL, optionally with a port and a path.
fn is_website(text: &str) -> bool {
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text);
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    let host = match authority.split_once(':') {
        Some((host, port)) => {
            if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            host
        }
        None => authority,
    };
    let labels: Vec<&str> = host.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'))
        && !path.chars().any(char::is_whitespace)
}

fn parse_number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// Integer types that can have a sentinel table, with their raw range.
fn sentinel_range(ty: DataType) -> Option<(i128, i128)> {
    Some(match ty {
        DataType::Int16 => (i16::MIN.into(), i16::MAX.into()),
        DataType::Uint16 => (0, u16::MAX.into()),
        DataType::Int32 => (i32::MIN.into(), i32::MAX.into()),
        DataType::Uint32 => (0, u32::MAX.into()),
        DataType::Int64 => (i64::MIN.into(), i64::MAX.into()),
        DataType::Uint64 => (0, u64::MAX.into()),
        _ => return None,
    })
}

/// `value=label|…` pairs whose values fit the type.
fn parse_sentinels(ty: DataType, text: &str) -> Option<Vec<(i128, String)>> {
    let (lo, hi) = sentinel_range(ty)?;
    text.split('|')
        .map(|pair| {
            let (value, label) = pair.split_once('=')?;
            let value = value.trim();
            let n = match value.get(..2) {
                Some("0x" | "0X") => i128::from_str_radix(&value[2..], 16).ok()?,
                _ => {
                    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
                    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    value.parse().ok()?
                }
            };
            let label = label.trim();
            ((lo..=hi).contains(&n) && !label.is_empty()).then(|| (n, label.to_string()))
        })
        .collect()
}

/// Label of a sentinel matching a raw (unscaled) value of type `ty`.
pub fn find_sentinel<'a>(
    tables: &'a [(DataType, Vec<(i128, String)>)],
    ty: DataType,
    raw: &Value,
) -> Option<&'a str> {
    let n: i128 = match raw {
        Value::Int(v) => (*v).into(),
        Value::Uint(v) => (*v).into(),
        _ => return None,
    };
    tables
        .iter()
        .find(|(t, _)| *t == ty)?
        .1
        .iter()
        .find(|(v, _)| *v == n)
        .map(|(_, l)| l.as_str())
}

fn parse_enum(text: &str) -> Option<Vec<(i64, String)>> {
    text.split('|')
        .map(|pair| {
            let (value, label) = pair.split_once('=')?;
            let value = parse_int(value.trim())?;
            let label = label.trim();
            (!label.is_empty()).then(|| (value, label.to_string()))
        })
        .collect()
}

fn parse_area(text: &str) -> Option<Area> {
    match text.to_lowercase().as_str() {
        "coil" => Some(Area::Coil),
        "discrete" => Some(Area::Discrete),
        "input" => Some(Area::Input),
        "holding" => Some(Area::Holding),
        _ => None,
    }
}

fn parse_type(text: &str) -> Option<DataType> {
    Some(match text.to_lowercase().as_str() {
        "bool" => DataType::Bool,
        "int16" => DataType::Int16,
        "uint16" => DataType::Uint16,
        "int32" => DataType::Int32,
        "uint32" => DataType::Uint32,
        "float32" => DataType::Float32,
        "int64" => DataType::Int64,
        "uint64" => DataType::Uint64,
        "float64" => DataType::Float64,
        "string" => DataType::String,
        _ => return None,
    })
}

fn parse_byte_order(text: &str) -> Option<ByteOrder> {
    match text.to_uppercase().as_str() {
        "ABCD" => Some(ByteOrder::ABCD),
        "CDAB" => Some(ByteOrder::CDAB),
        "BADC" => Some(ByteOrder::BADC),
        "DCBA" => Some(ByteOrder::DCBA),
        _ => None,
    }
}

fn parse_access(text: &str) -> Option<Access> {
    match text.to_uppercase().as_str() {
        "R" => Some(Access::R),
        "W" => Some(Access::W),
        "RW" => Some(Access::RW),
        _ => None,
    }
}

fn access_allowed(area: Area, access: Access) -> bool {
    match area {
        Area::Coil | Area::Holding => true,
        Area::Discrete | Area::Input => access == Access::R,
    }
}

fn area_name(area: Area) -> &'static str {
    match area {
        Area::Coil => "coil",
        Area::Discrete => "discrete",
        Area::Input => "input",
        Area::Holding => "holding",
    }
}

fn type_name(ty: DataType) -> &'static str {
    match ty {
        DataType::Bool => "bool",
        DataType::Int16 => "int16",
        DataType::Uint16 => "uint16",
        DataType::Int32 => "int32",
        DataType::Uint32 => "uint32",
        DataType::Float32 => "float32",
        DataType::Int64 => "int64",
        DataType::Uint64 => "uint64",
        DataType::Float64 => "float64",
        DataType::String => "string",
    }
}

/// Splits one CSV line (RFC 4180 quoting, no multi-line fields) and trims
/// every field.
fn split_csv(line: &str, sep: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut at_start = true;
    let mut quoted = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == sep {
            fields.push(field.trim().to_string());
            field.clear();
            at_start = true;
            continue;
        } else if c == '"' && at_start {
            quoted = true;
        } else {
            field.push(c);
        }
        at_start = false;
    }
    fields.push(field.trim().to_string());
    fields
}

struct Checker {
    out: Vec<Diagnostic>,
}

impl Checker {
    fn error(&mut self, line: Option<usize>, code: &'static str, message: String) {
        self.out.push(Diagnostic {
            severity: Severity::Error,
            line,
            code,
            message,
        });
    }

    fn warn(&mut self, line: Option<usize>, code: &'static str, message: String) {
        self.out.push(Diagnostic {
            severity: Severity::Warning,
            line,
            code,
            message,
        });
    }

    fn error_count(&self) -> usize {
        self.out
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}

/// A data line before validation: line number, cells by column, and the
/// non-empty cells of columns this version does not know.
type RawRow = (usize, HashMap<String, String>, Vec<(String, String)>);

/// Checks a metadata value; returns false if it is out of range.
fn apply_meta(meta: &mut Meta, key: &str, value: &str) -> bool {
    let lower = value.to_lowercase();
    macro_rules! range {
        ($field:ident, $lo:expr, $hi:expr) => {
            match decimal_in(value, $lo, $hi) {
                Some(n) => {
                    meta.$field = Some(n as _);
                    true
                }
                None => false,
            }
        };
    }
    match key {
        "mbmap" => value == "1",
        "vendor" => {
            meta.vendor = value.to_string();
            true
        }
        "model" => {
            meta.model = value.to_string();
            true
        }
        "vendor_website" => {
            meta.vendor_website = Some(value.to_string());
            is_website(value)
        }
        "description" => {
            meta.description = Some(value.to_string());
            true
        }
        "firmware" => {
            meta.firmware = Some(value.to_string());
            true
        }
        "source" => {
            meta.source = Some(value.to_string());
            true
        }
        "protocol" => {
            meta.protocol = match lower.as_str() {
                "tcp" => Some(Protocol::Tcp),
                "rtu" => Some(Protocol::Rtu),
                "tcp+rtu" => Some(Protocol::TcpRtu),
                _ => return false,
            };
            true
        }
        "unit_id" => range!(unit_id, 0, 255),
        "port" => range!(port, 1, 65535),
        "baudrate" => range!(baudrate, 1, 10_000_000),
        "parity" => {
            meta.parity = match lower.as_str() {
                "n" => Some(Parity::N),
                "e" => Some(Parity::E),
                "o" => Some(Parity::O),
                _ => return false,
            };
            true
        }
        "databits" => matches!(value, "7" | "8") && range!(data_bits, 7, 8),
        "stopbits" => matches!(value, "1" | "2") && range!(stop_bits, 1, 2),
        "byte_order" => {
            meta.byte_order = parse_byte_order(value);
            meta.byte_order.is_some()
        }
        "address_base" => matches!(value, "0" | "1") && range!(address_base, 0, 1),
        "max_read_registers" => range!(max_read_registers, 1, 125),
        "max_read_bits" => range!(max_read_bits, 1, 2000),
        _ => {
            meta.extra.push((key.to_string(), value.to_string()));
            true
        }
    }
}

/// Parses a `.mbmap` file. Never fails: problems come back as diagnostics,
/// and `map` is `None` when there is at least one error.
pub fn parse(bytes: &[u8]) -> Parsed {
    let mut ck = Checker { out: Vec::new() };
    let Ok(text) = std::str::from_utf8(bytes) else {
        ck.error(None, "not-utf8", "file is not valid UTF-8".into());
        return Parsed {
            map: None,
            diagnostics: ck.out,
        };
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    let mut meta = Meta::default();
    let mut seen_meta: Vec<String> = Vec::new();
    let mut header: Option<Vec<String>> = None;
    let mut sep = ',';
    let mut rows: Vec<RawRow> = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let lineno = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('@') {
            if header.is_some() {
                ck.error(
                    Some(lineno),
                    "meta-after-header",
                    "metadata after the header".into(),
                );
                continue;
            }
            let valid_key = |k: &str| {
                !k.is_empty()
                    && k.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            };
            match rest.split_once('=') {
                Some((key, value)) if valid_key(key.trim()) => {
                    let (key, value) = (key.trim(), value.trim());
                    if seen_meta.iter().any(|k| k == key) {
                        ck.error(
                            Some(lineno),
                            "duplicate-meta",
                            format!("duplicate metadata key @{key}"),
                        );
                    } else {
                        seen_meta.push(key.to_string());
                        if let Some(type_name) = key.strip_prefix("sentinel_") {
                            match parse_type(type_name)
                                .and_then(|ty| parse_sentinels(ty, value).map(|t| (ty, t)))
                            {
                                Some(table) => meta.sentinels.push(table),
                                None => ck.error(
                                    Some(lineno),
                                    "bad-sentinel",
                                    format!("invalid sentinel table @{key}"),
                                ),
                            }
                        } else if let Some(name) =
                            key.strip_prefix("enum_").filter(|n| !n.is_empty())
                        {
                            match parse_enum(value) {
                                Some(labels) => meta.enums.push((name.to_string(), labels)),
                                None => {
                                    ck.error(
                                        Some(lineno),
                                        "bad-enum",
                                        format!("malformed table @{key} (expected 0=Off|1=On)"),
                                    );
                                    // Still defined, so rows using it are not
                                    // reported a second time.
                                    meta.enums.push((name.to_string(), Vec::new()));
                                }
                            }
                        } else if !apply_meta(&mut meta, key, value) {
                            ck.error(
                                Some(lineno),
                                "bad-meta-value",
                                format!("invalid value for @{key}: {value:?}"),
                            );
                        }
                    }
                }
                _ => ck.error(
                    Some(lineno),
                    "malformed-meta",
                    format!("malformed metadata line: {line:?}"),
                ),
            }
            continue;
        }

        let Some(columns) = &header else {
            sep = match (line.find(','), line.find(';')) {
                (Some(c), Some(s)) if s < c => ';',
                (None, Some(_)) => ';',
                _ => ',',
            };
            let cols: Vec<String> = split_csv(line, sep)
                .into_iter()
                .map(|c| c.to_lowercase())
                .collect();
            for (i, col) in cols.iter().enumerate() {
                if cols[..i].contains(col) {
                    ck.error(
                        Some(lineno),
                        "duplicate-column",
                        format!("duplicate column {col:?}"),
                    );
                }
            }
            for col in REQUIRED_COLUMNS {
                if !cols.iter().any(|c| c == col) {
                    ck.error(
                        Some(lineno),
                        "missing-column",
                        format!("header is missing required column {col:?}"),
                    );
                }
            }
            header = Some(cols);
            continue;
        };

        let fields = split_csv(line, sep);
        if fields.len() > columns.len() {
            ck.error(
                Some(lineno),
                "too-many-fields",
                format!(
                    "row has {} fields, header has {}",
                    fields.len(),
                    columns.len()
                ),
            );
            continue;
        }
        let mut row = HashMap::new();
        let mut extra = Vec::new();
        for (i, col) in columns.iter().enumerate() {
            let value = fields.get(i).cloned().unwrap_or_default();
            if row.contains_key(col) {
                continue; // with a duplicated column, the first one wins
            }
            if !KNOWN_COLUMNS.contains(&col.as_str()) && !value.is_empty() {
                extra.push((col.clone(), value.clone()));
            }
            row.insert(col.clone(), value);
        }
        rows.push((lineno, row, extra));
    }

    for key in REQUIRED_META {
        if !seen_meta.iter().any(|k| k == key) {
            ck.error(
                None,
                "missing-meta",
                format!("missing required metadata @{key}"),
            );
        }
    }
    let Some(columns) = header else {
        ck.error(None, "no-header", "no header line".into());
        return Parsed {
            map: None,
            diagnostics: ck.out,
        };
    };
    for (i, col) in columns.iter().enumerate() {
        if !columns[..i].contains(col)
            && !KNOWN_COLUMNS.contains(&col.as_str())
            && !col.starts_with("x_")
        {
            ck.warn(
                None,
                "unknown-column",
                format!("unknown column {col:?} (custom columns should start with 'x_')"),
            );
        }
    }
    if REQUIRED_COLUMNS
        .iter()
        .any(|c| !columns.iter().any(|col| col == c))
    {
        return Parsed {
            map: None,
            diagnostics: ck.out,
        };
    }
    if rows.is_empty() {
        ck.warn(None, "no-rows", "no register rows".into());
    }

    let base = i64::from(meta.address_base());
    let mut names: HashMap<String, usize> = HashMap::new();
    // (area, start, end, line, name) for the overlap check
    let mut spans: Vec<(&'static str, i64, i64, usize, String)> = Vec::new();
    let mut registers = Vec::new();
    // (line, register index) of rows that name a scale factor register
    let mut scaled: Vec<(usize, usize)> = Vec::new();

    for (lineno, row, extra) in rows {
        let line = Some(lineno);
        let cell = |col: &str| row.get(col).map(String::as_str).unwrap_or("");
        let errors_before = ck.error_count();

        let empty: Vec<&str> = REQUIRED_COLUMNS
            .into_iter()
            .filter(|c| cell(c).is_empty())
            .collect();
        if !empty.is_empty() {
            for col in empty {
                ck.error(line, "empty-cell", format!("empty required cell {col:?}"));
            }
            continue;
        }

        let name = cell("name").to_string();
        if let Some(first) = names.get(&name) {
            ck.warn(
                line,
                "duplicate-name",
                format!("name {name:?} already used on line {first}"),
            );
        } else {
            names.insert(name.clone(), lineno);
        }

        let Some(area) = parse_area(cell("area")) else {
            ck.error(line, "bad-area", format!("unknown area {:?}", cell("area")));
            continue;
        };
        let Some(ty) = parse_type(cell("type")) else {
            ck.error(line, "bad-type", format!("unknown type {:?}", cell("type")));
            continue;
        };
        let Some(address) = parse_int(cell("address")) else {
            ck.error(
                line,
                "bad-address",
                format!("invalid address {:?}", cell("address")),
            );
            continue;
        };
        if area.is_bit() && ty != DataType::Bool {
            ck.error(
                line,
                "type-area",
                format!("area {:?} only holds bool values", area_name(area)),
            );
            continue;
        }

        let register_bool = ty == DataType::Bool && !area.is_bit();
        let mut bit = None;
        let mut bits = None;
        let bit_text = cell("bit");
        let bits_text = cell("bits");
        if !bits_text.is_empty() {
            let width = decimal_in(bits_text, 1, 16);
            if width.is_none() || ty != DataType::Uint16 || area.is_bit() || bit_text.is_empty() {
                ck.error(
                    line,
                    "bad-bits",
                    format!("invalid bit field: bits={bits_text:?}"),
                );
            } else if let Some(b) = decimal_in(bit_text, 0, 15) {
                let w = width.unwrap_or(0);
                if b + w > 16 {
                    ck.error(line, "bad-bits", format!("bit {b} + bits {w} exceeds 16"));
                } else {
                    bit = Some(b as u8);
                    bits = Some(w as u8);
                }
            } else {
                ck.error(line, "bad-bit", format!("invalid bit {bit_text:?}"));
            }
        } else if register_bool {
            if bit_text.is_empty() {
                ck.error(
                    line,
                    "missing-bit",
                    "bool in a register area needs a 'bit' (0-15)".into(),
                );
            } else if let Some(b) = decimal_in(bit_text, 0, 15) {
                bit = Some(b as u8);
            } else {
                ck.error(line, "bad-bit", format!("invalid bit {bit_text:?}"));
            }
        } else if !bit_text.is_empty() {
            ck.warn(
                line,
                "unused-bit",
                "'bit' is only used for bool in input/holding; ignored".into(),
            );
        }

        let mut count = i64::from(ty.register_count().unwrap_or(0).max(1));
        let mut length = None;
        let mut length_ok = true;
        if ty == DataType::String {
            let text = cell("length");
            if text.is_empty() {
                ck.error(
                    line,
                    "missing-length",
                    "string needs a 'length' (registers)".into(),
                );
                length_ok = false;
            } else if let Some(n) = decimal_in(text, 1, 65536) {
                count = n;
                length = Some(n.min(i64::from(u16::MAX)) as u16);
            } else {
                ck.error(line, "bad-length", format!("invalid length {text:?}"));
                length_ok = false;
            }
        }

        let wire = address - base;
        if wire < 0 {
            ck.error(
                line,
                "address-range",
                format!("address {address} is below @address_base={base}"),
            );
        } else if length_ok && wire + count > 65536 {
            ck.error(
                line,
                "address-range",
                "value goes beyond address 65535".into(),
            );
        }

        let access_text = cell("access");
        let access = if access_text.is_empty() {
            Some(Access::R)
        } else {
            parse_access(access_text)
        };
        let access = match access {
            Some(a) if access_allowed(area, a) => a,
            _ => {
                let shown = if access_text.is_empty() {
                    "R"
                } else {
                    access_text
                };
                ck.error(
                    line,
                    "bad-access",
                    format!(
                        "access {:?} is not allowed for area {:?}",
                        shown.to_uppercase(),
                        area_name(area)
                    ),
                );
                Access::R
            }
        };

        let order_text = cell("byte_order");
        let byte_order = if order_text.is_empty() {
            None
        } else {
            let order = parse_byte_order(order_text);
            if order.is_none() {
                ck.error(
                    line,
                    "bad-byte-order",
                    format!("invalid byte_order {order_text:?}"),
                );
            }
            order
        };

        let mut number = |col: &str, default: f64| {
            let text = cell(col);
            if text.is_empty() {
                return default;
            }
            parse_number(text).unwrap_or_else(|| {
                ck.error(
                    line,
                    "bad-number",
                    format!("{col} is not a number: {text:?}"),
                );
                default
            })
        };
        let scale = number("scale", 1.0);
        let offset = number("offset", 0.0);

        let enum_text = cell("enum");
        let mut enum_ref = None;
        let enum_labels = if enum_text.is_empty() {
            Vec::new()
        } else if let Some(name) = enum_text.strip_prefix('@') {
            match meta.enums.iter().find(|(n, _)| n == name) {
                Some((_, labels)) => {
                    enum_ref = Some(name.to_string());
                    labels.clone()
                }
                None => {
                    ck.error(
                        line,
                        "unknown-enum",
                        format!("no table @enum_{name} for {enum_text:?}"),
                    );
                    Vec::new()
                }
            }
        } else {
            parse_enum(enum_text).unwrap_or_else(|| {
                ck.error(
                    line,
                    "bad-enum",
                    format!("malformed enum {enum_text:?} (expected 0=Off|1=On)"),
                );
                Vec::new()
            })
        };

        if ck.error_count() != errors_before {
            continue;
        }
        // Bits and bit fields share their register with other rows.
        if !register_bool && bits.is_none() {
            spans.push((area_name(area), wire, wire + count, lineno, name.clone()));
        }
        let scale_factor = Some(cell("scale_factor").to_string()).filter(|s| !s.is_empty());
        if scale_factor.is_some() {
            scaled.push((lineno, registers.len()));
        }
        registers.push(Register {
            name,
            group: cell("group").to_string(),
            area,
            address: wire as u16,
            ty,
            length,
            scale,
            scale_factor,
            offset,
            unit: cell("unit").to_string(),
            access,
            byte_order,
            bit,
            bits,
            enum_labels,
            enum_ref,
            description: cell("description").to_string(),
            extra,
        });
    }

    // A scale factor names an integer register of the same map that does not
    // itself depend on a scale factor.
    for (lineno, i) in scaled {
        let reg = &registers[i];
        let target = reg.scale_factor.as_deref().unwrap_or_default();
        let Some(sf) = registers.iter().find(|r| r.name == target) else {
            ck.error(
                Some(lineno),
                "unknown-scale-factor",
                format!("no register named {target:?} for the scale factor"),
            );
            continue;
        };
        let numeric = |t: DataType| !matches!(t, DataType::Bool | DataType::String);
        let integer =
            |t: DataType| numeric(t) && !matches!(t, DataType::Float32 | DataType::Float64);
        let problem = if !numeric(reg.ty) {
            Some(format!(
                "{} values cannot have a scale factor",
                type_name(reg.ty)
            ))
        } else if !integer(sf.ty) || sf.bits.is_some() {
            Some(format!(
                "scale factor {target:?} must be a whole integer register"
            ))
        } else if sf.scale_factor.is_some() {
            Some(format!("scale factor {target:?} has a scale factor itself"))
        } else {
            None
        };
        if let Some(message) = problem {
            ck.error(Some(lineno), "bad-scale-factor", message);
        }
    }

    spans.sort();
    for i in 0..spans.len() {
        let (area, start, _, lineno, ref name) = spans[i];
        if let Some((_, _, _, first_line, first_name)) = spans[..i]
            .iter()
            .rev()
            // Rows starting at the same address are alternative readings.
            .find(|(a, s, end, _, _)| *a == area && *s != start && start < *end)
        {
            ck.warn(
                Some(lineno),
                "overlap",
                format!("{name:?} overlaps {first_name:?} (line {first_line})"),
            );
        }
    }

    let has_errors = ck.error_count() > 0;
    Parsed {
        map: (!has_errors).then_some(DeviceMap { meta, registers }),
        diagnostics: ck.out,
    }
}

fn quote(field: &str, sep: char, first: bool) -> String {
    let needs = field.contains(sep)
        || field.contains('"')
        || (first && (field.starts_with('#') || field.starts_with('@')));
    if needs {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn enum_pairs(labels: &[(i64, String)]) -> String {
    labels
        .iter()
        .map(|(v, l)| format!("{v}={l}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn fmt_number(n: f64) -> String {
    format!("{n}")
}

/// Writes a map as `.mbmap` text (LF line endings, `,` separator).
pub fn to_string(map: &DeviceMap) -> String {
    let m = &map.meta;
    let mut out = String::from("@mbmap=1\n");
    let mut meta = |key: &str, value: Option<String>| {
        if let Some(v) = value {
            out.push_str(&format!("@{key}={v}\n"));
        }
    };
    meta("vendor", Some(m.vendor.clone()));
    meta("model", Some(m.model.clone()));
    meta("vendor_website", m.vendor_website.clone());
    meta("description", m.description.clone());
    meta("firmware", m.firmware.clone());
    meta("source", m.source.clone());
    meta(
        "protocol",
        m.protocol.map(|p| {
            match p {
                Protocol::Tcp => "tcp",
                Protocol::Rtu => "rtu",
                Protocol::TcpRtu => "tcp+rtu",
            }
            .to_string()
        }),
    );
    meta("unit_id", m.unit_id.map(|v| v.to_string()));
    meta("port", m.port.map(|v| v.to_string()));
    meta("baudrate", m.baudrate.map(|v| v.to_string()));
    meta("parity", m.parity.map(|p| format!("{p:?}")));
    meta("databits", m.data_bits.map(|v| v.to_string()));
    meta("stopbits", m.stop_bits.map(|v| v.to_string()));
    meta("byte_order", m.byte_order.map(|b| format!("{b:?}")));
    meta("address_base", m.address_base.map(|v| v.to_string()));
    meta(
        "max_read_registers",
        m.max_read_registers.map(|v| v.to_string()),
    );
    meta("max_read_bits", m.max_read_bits.map(|v| v.to_string()));
    for (ty, table) in &m.sentinels {
        let pairs = table
            .iter()
            .map(|(v, l)| {
                if *v < 0 {
                    format!("{v}={l}")
                } else {
                    format!("0x{v:X}={l}")
                }
            })
            .collect::<Vec<_>>()
            .join("|");
        meta(&format!("sentinel_{}", type_name(*ty)), Some(pairs));
    }
    for (name, labels) in &m.enums {
        meta(&format!("enum_{name}"), Some(enum_pairs(labels)));
    }
    for (k, v) in &m.extra {
        meta(k, Some(v.clone()));
    }

    let mut extra_cols: Vec<&str> = Vec::new();
    for r in &map.registers {
        for (col, _) in &r.extra {
            if !extra_cols.contains(&col.as_str()) {
                extra_cols.push(col);
            }
        }
    }
    // The scale_factor column is only written when a register uses it.
    let with_sf = map.registers.iter().any(|r| r.scale_factor.is_some());
    let keep = |col: &&str| with_sf || *col != "scale_factor";
    let mut header: Vec<&str> = KNOWN_COLUMNS.iter().copied().filter(keep).collect();
    header.extend(&extra_cols);
    out.push('\n');
    out.push_str(&header.join(","));
    out.push('\n');

    let base = u32::from(m.address_base());
    for r in &map.registers {
        let enum_text = match &r.enum_ref {
            Some(name) => format!("@{name}"),
            None => enum_pairs(&r.enum_labels),
        };
        let mut fields = vec![
            r.name.clone(),
            r.group.clone(),
            area_name(r.area).to_string(),
            (u32::from(r.address) + base).to_string(),
            type_name(r.ty).to_string(),
            r.length.map(|n| n.to_string()).unwrap_or_default(),
            if r.scale == 1.0 {
                String::new()
            } else {
                fmt_number(r.scale)
            },
            r.scale_factor.clone().unwrap_or_default(),
            if r.offset == 0.0 {
                String::new()
            } else {
                fmt_number(r.offset)
            },
            r.unit.clone(),
            format!("{:?}", r.access),
            r.byte_order.map(|b| format!("{b:?}")).unwrap_or_default(),
            r.bit.map(|b| b.to_string()).unwrap_or_default(),
            r.bits.map(|b| b.to_string()).unwrap_or_default(),
            enum_text,
            r.description.clone(),
        ];
        if !with_sf {
            fields.remove(7);
        }
        for col in &extra_cols {
            let value = r
                .extra
                .iter()
                .find(|(c, _)| c == col)
                .map(|(_, v)| v.clone());
            fields.push(value.unwrap_or_default());
        }
        let line: Vec<String> = fields
            .iter()
            .enumerate()
            .map(|(i, f)| quote(f, ',', i == 0))
            .collect();
        out.push_str(line.join(",").trim_end_matches(','));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "@mbmap=1\n@vendor=Test\n@model=T1\n";

    fn ok(text: &str) -> DeviceMap {
        let parsed = parse(text.as_bytes());
        assert!(
            parsed.errors().next().is_none(),
            "{:#?}",
            parsed.diagnostics
        );
        parsed.map.unwrap()
    }

    #[test]
    fn scale_factors() {
        let cols = "name,area,address,type,scale_factor
";
        let map = ok(&format!(
            "{HEAD}{cols}W,holding,0,int16,W_SF
W_SF,holding,1,int16,
"
        ));
        assert_eq!(map.registers[0].scale_factor.as_deref(), Some("W_SF"));
        assert_eq!(map.scale_registers(&[0]).len(), 1);
        assert!(map.scale_registers(&[0, 1]).is_empty());
        let text = to_string(&map);
        assert!(text.contains("scale_factor"), "{text}");
        assert_eq!(parse(text.as_bytes()).map.unwrap(), map);

        let codes = |body: &str| {
            parse(format!("{HEAD}{cols}{body}").as_bytes())
                .errors()
                .map(|d| d.code)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            codes(
                "W,holding,0,int16,Nope
"
            ),
            ["unknown-scale-factor"]
        );
        assert_eq!(
            codes(
                "W,holding,0,int16,F
F,holding,1,float32,
"
            ),
            ["bad-scale-factor"]
        );
        assert_eq!(
            codes(
                "S,coil,0,bool,F
F,holding,1,int16,
"
            )
            .iter()
            .filter(|c| **c == "bad-scale-factor")
            .count(),
            1
        );
        // Without a scale factor the column is not written.
        let plain = ok(&format!(
            "{HEAD}name,area,address,type
A,holding,0,int16
"
        ));
        assert!(!to_string(&plain).contains("scale_factor"));
    }

    #[test]
    fn vendor_website() {
        for good in [
            "se.com",
            "www.se.com",
            "https://www.se.com/ww/en/",
            "http://10.0.0.1:8080",
        ] {
            assert!(is_website(good), "{good}");
        }
        for bad in [
            "se",
            "se dot com",
            "https://",
            "ftp://se.com",
            "se..com",
            "se.com:x",
        ] {
            assert!(!is_website(bad), "{bad}");
        }
        let map = ok(&format!(
            "{HEAD}@vendor_website=se.com
name,area,address,type
A,holding,0,uint16
"
        ));
        assert_eq!(map.meta.vendor_website.as_deref(), Some("se.com"));
        assert!(to_string(&map).contains(
            "@vendor_website=se.com
"
        ));
    }

    #[test]
    fn csv_splitting() {
        assert_eq!(split_csv("a, b ,c", ','), ["a", "b", "c"]);
        assert_eq!(
            split_csv(r#""x, y","say ""hi""",z"#, ','),
            ["x, y", r#"say "hi""#, "z"]
        );
        assert_eq!(split_csv("a;;", ';'), ["a", "", ""]);
        assert_eq!(split_csv(r#"ab"c"#, ','), [r#"ab"c"#]);
    }

    #[test]
    fn typed_values_and_defaults() {
        let map = ok(&format!(
            "{HEAD}@address_base=1\n@byte_order=CDAB\n\
             name,area,address,type,scale,offset,access,byte_order,bit,enum,length,x_note\n\
             Temp,holding,0x10,int16,0.1,-40,rw,,,,,hot\n\
             Run,holding,17,bool,,,,,3,0=Stopped|1=Running,,\n\
             Serial,input,100,string,,,,BADC,,,8,\n"
        ));
        assert_eq!(map.meta.unit_id(), 1);
        assert_eq!(map.meta.address_base(), 1);
        assert_eq!(map.meta.byte_order(), ByteOrder::CDAB);

        let temp = &map.registers[0];
        assert_eq!(temp.address, 15); // 0x10 with a 1-based file
        assert_eq!((temp.scale, temp.offset), (0.1, -40.0));
        assert_eq!(temp.access, Access::RW);
        assert_eq!(temp.byte_order(&map.meta), ByteOrder::CDAB);
        assert_eq!(temp.extra, [("x_note".to_string(), "hot".to_string())]);

        let run = &map.registers[1];
        assert_eq!((run.bit, run.access, run.count()), (Some(3), Access::R, 1));
        assert_eq!(
            run.enum_labels,
            [(0, "Stopped".to_string()), (1, "Running".to_string())]
        );

        let serial = &map.registers[2];
        assert_eq!((serial.length, serial.count()), (Some(8), 8));
        assert_eq!(serial.byte_order(&map.meta), ByteOrder::BADC);
    }

    #[test]
    fn writer_round_trips_and_quotes() {
        let map = ok(&format!(
            "{HEAD}@unit_id=3\n@x_tool=1\n\
             name,area,address,type,description,unit\n\
             \"#1 pump\",holding,0,uint16,\"flow, \"\"raw\"\"\",m³/h\n\
             \"@home\",coil,5,bool,,\n"
        ));
        let text = to_string(&map);
        assert!(text.contains("\"#1 pump\""), "{text}");
        assert!(text.contains("\"@home\""), "{text}");
        assert!(text.contains("\"flow, \"\"raw\"\"\""), "{text}");
        assert!(text.contains("@x_tool=1\n"), "{text}");
        assert_eq!(ok(&text), map);
    }

    #[test]
    fn errors_drop_the_map() {
        let parsed =
            parse(format!("{HEAD}name,area,address,type\nA,holding,0,flot32\n").as_bytes());
        assert!(parsed.map.is_none());
        assert_eq!(parsed.diagnostics[0].code, "bad-type");
        assert_eq!(parsed.diagnostics[0].line, Some(5));
    }
}
