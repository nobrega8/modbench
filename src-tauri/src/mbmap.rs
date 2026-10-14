//! Reader and writer for `.mbmap` device maps.
//!
//! Reads format version 2 (JSON, SPEC-v2.md in the mbmap-library repo) and
//! the deprecated version 1 (CSV, SPEC.md); writes version 2. Both versions
//! go through the same checks and diagnostic codes: each reader turns its
//! file into raw rows, and `finish` validates them.
//! `tests/mbmap_fixtures.rs` checks this module against the shared fixtures.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::codec::{ByteOrder, DataType, Value};
use crate::firmware;
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
    /// JSON Pointer to the value, for version 2 files (`/registers/3/address`).
    pub pointer: Option<String>,
    /// Stable code from SPEC §7, e.g. `missing-bit`.
    pub code: &'static str,
    pub message: String,
}

impl Diagnostic {
    /// "line 5", "/registers/3/address" or "file".
    pub fn location(&self) -> String {
        match (&self.line, &self.pointer) {
            (_, Some(p)) => p.clone(),
            (Some(l), None) => format!("line {l}"),
            (None, None) => "file".into(),
        }
    }
}

/// Shown when a version 1 file is opened.
pub const V1_DEPRECATED: &str =
    "format version 1 (CSV) is deprecated; convert the map to version 2 (JSON)";

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

/// What an identity register tells about the device (spec section 4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Model,
    Firmware,
    Serial,
}

impl Role {
    fn parse(text: &str) -> Option<Role> {
        match text {
            "model" => Some(Role::Model),
            "firmware" => Some(Role::Firmware),
            "serial" => Some(Role::Serial),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Role::Model => "model",
            Role::Firmware => "firmware",
            Role::Serial => "serial",
        }
    }
}

/// A document the map was built or checked from (spec section 3.3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub title: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
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
    /// Free notes about the whole file (`notes`; `#` lines before the v1 header).
    #[serde(default)]
    pub notes: Vec<String>,
    /// Every model name or code the map applies to; empty means `model` alone.
    #[serde(default)]
    pub models: Vec<String>,
    /// Revision of the map's content, and its date (spec section 3.1).
    #[serde(default)]
    pub revision: Option<u32>,
    #[serde(default)]
    pub revised: Option<String>,
    /// Firmware range the map applies to, as written (spec section 3.2).
    #[serde(default)]
    pub firmware_min: Option<String>,
    #[serde(default)]
    pub firmware_max: Option<String>,
    #[serde(default)]
    pub sources: Vec<Source>,
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
    /// The firmware range the whole map applies to.
    pub fn firmware_range(&self) -> firmware::Range {
        range_of(&self.firmware_min, &self.firmware_max)
    }
    /// The model names a device may report: `models`, or `model` alone.
    pub fn model_names(&self) -> Vec<&str> {
        if self.models.is_empty() {
            vec![self.model.as_str()]
        } else {
            self.models.iter().map(String::as_str).collect()
        }
    }
}

fn range_of(min: &Option<String>, max: &Option<String>) -> firmware::Range {
    firmware::Range {
        min: min.as_deref().and_then(firmware::parse),
        max: max.as_deref().and_then(firmware::parse),
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
    /// Smallest / largest value a write accepts, in engineering units.
    #[serde(default)]
    pub write_min: Option<f64>,
    #[serde(default)]
    pub write_max: Option<f64>,
    /// Raw value a healthy device holds here (spec section 4.6), as written.
    #[serde(default)]
    pub reference: Option<String>,
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
    /// Free note about this register (`comment`; `#` lines above a v1 row).
    #[serde(default)]
    pub comment: String,
    /// Firmware range that has this register, as written (spec section 3.2).
    #[serde(default)]
    pub firmware_min: Option<String>,
    #[serde(default)]
    pub firmware_max: Option<String>,
    /// What this register tells about the device, if anything.
    #[serde(default)]
    pub role: Option<Role>,
}

impl Register {
    /// The firmware range that has this register.
    pub fn firmware_range(&self) -> firmware::Range {
        range_of(&self.firmware_min, &self.firmware_max)
    }

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
    /// Format version of the file (1 is deprecated), when it could be told.
    pub version: Option<u8>,
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
const KNOWN_COLUMNS: [&str; 19] = [
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
    "write_min",
    "write_max",
    "byte_order",
    "bit",
    "bits",
    "enum",
    "reference",
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
pub(crate) fn parse_int(text: &str) -> Option<i64> {
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

pub(crate) fn decimal_in(text: &str, lo: i64, hi: i64) -> Option<i64> {
    parse_decimal(text).filter(|n| (lo..=hi).contains(n))
}

/// A domain (`se.com`) or an `http(s)://` URL, optionally with a port and a path.
pub(crate) fn is_website(text: &str) -> bool {
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

/// Decimal or 0x hex integer of any supported width (up to uint64).
pub(crate) fn parse_i128(text: &str) -> Option<i128> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let n = match digits.get(..2) {
        Some("0x" | "0X") => i128::from_str_radix(&digits[2..], 16).ok()?,
        _ if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
            digits.parse::<i128>().ok()?
        }
        _ => return None,
    };
    Some(if negative { -n } else { n })
}

/// Whether `text` is a raw value that a row of this type can hold.
fn valid_reference(ty: DataType, bits: Option<u8>, text: &str, length: Option<u16>) -> bool {
    if let Some(width) = bits {
        return parse_i128(text).is_some_and(|n| (0..1i128 << width).contains(&n));
    }
    match ty {
        DataType::Bool => matches!(
            text.to_ascii_lowercase().as_str(),
            "0" | "1" | "true" | "false"
        ),
        DataType::String => length.is_none_or(|n| text.chars().count() <= 2 * usize::from(n)),
        DataType::Float32 | DataType::Float64 => parse_number(text).is_some(),
        _ => match sentinel_range(ty) {
            Some((lo, hi)) => parse_i128(text).is_some_and(|n| (lo..=hi).contains(&n)),
            None => false,
        },
    }
}

/// One spelling per value, so both versions store the same text: `true` /
/// `false` for bool, shortest form for floats, decimal integers without a
/// sign or leading zeros (hex is kept as written).
fn canonical_reference(ty: DataType, bits: Option<u8>, text: &str) -> String {
    match ty {
        DataType::Bool if bits.is_none() => {
            matches!(text.to_ascii_lowercase().as_str(), "1" | "true").to_string()
        }
        DataType::Float32 | DataType::Float64 => {
            parse_number(text).map_or_else(|| text.to_string(), |f| f.to_string())
        }
        DataType::String => text.to_string(),
        _ if text.to_ascii_lowercase().contains("0x") => text.to_string(),
        _ => parse_i128(text).map_or_else(|| text.to_string(), |n| n.to_string()),
    }
}

/// Compares a decoded raw value with the row's `reference` (spec 4.6):
/// `Some(true)` match, `Some(false)` mismatch, `None` when the row has none.
pub fn reference_matches(reg: &Register, raw: &Value) -> Option<bool> {
    let text = reg.reference.as_deref()?;
    Some(match raw {
        Value::Bool(b) => matches!(text.to_ascii_lowercase().as_str(), "1" | "true") == *b,
        Value::Int(n) => parse_i128(text) == Some(i128::from(*n)),
        Value::Uint(n) => parse_i128(text) == Some(i128::from(*n)),
        Value::Float(f) => match (reg.ty, parse_number(text)) {
            (DataType::Float32, Some(r)) => (r as f32) == (*f as f32),
            (_, Some(r)) => r == *f,
            _ => false,
        },
        Value::Text(t) => {
            let trim = |s: &str| s.trim_end_matches(['\0', ' ']).to_string();
            trim(t) == trim(text)
        }
    })
}

fn parse_number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// Integer types that can have a sentinel table, with their raw range.
pub(crate) fn sentinel_range(ty: DataType) -> Option<(i128, i128)> {
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

pub(crate) fn parse_type(text: &str) -> Option<DataType> {
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

pub(crate) fn area_name(area: Area) -> &'static str {
    match area {
        Area::Coil => "coil",
        Area::Discrete => "discrete",
        Area::Input => "input",
        Area::Holding => "holding",
    }
}

pub(crate) fn type_name(ty: DataType) -> &'static str {
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

/// Where a diagnostic points: a line (version 1) or a JSON Pointer (version 2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Loc {
    pub line: Option<usize>,
    pub pointer: Option<String>,
}

impl Loc {
    pub(crate) fn pointer(pointer: impl Into<String>) -> Self {
        Loc {
            line: None,
            pointer: Some(pointer.into()),
        }
    }

    /// "line 5" or "/registers/3", for messages that refer to another row.
    fn describe(&self) -> String {
        match (&self.line, &self.pointer) {
            (Some(l), _) => format!("line {l}"),
            (None, Some(p)) => p.clone(),
            (None, None) => "the file".into(),
        }
    }
}

impl From<Option<usize>> for Loc {
    fn from(line: Option<usize>) -> Self {
        Loc {
            line,
            pointer: None,
        }
    }
}

impl From<&Loc> for Loc {
    fn from(loc: &Loc) -> Self {
        loc.clone()
    }
}

pub(crate) struct Checker {
    pub out: Vec<Diagnostic>,
}

impl Checker {
    pub(crate) fn error(&mut self, at: impl Into<Loc>, code: &'static str, message: String) {
        let at = at.into();
        self.out.push(Diagnostic {
            severity: Severity::Error,
            line: at.line,
            pointer: at.pointer,
            code,
            message,
        });
    }

    pub(crate) fn warn(&mut self, at: impl Into<Loc>, code: &'static str, message: String) {
        let at = at.into();
        self.out.push(Diagnostic {
            severity: Severity::Warning,
            line: at.line,
            pointer: at.pointer,
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

/// A register before validation, from either version: its cells as v1 text,
/// the non-empty values of keys this version does not know, an enum table
/// given in place (v2), and its comment.
#[derive(Default)]
pub(crate) struct RawRow {
    pub at: Loc,
    pub cells: HashMap<String, String>,
    pub extra: Vec<(String, String)>,
    pub enum_table: Option<Vec<(i64, String)>>,
    pub comment: String,
}

/// Checks a metadata value; returns false if it is out of range.
pub(crate) fn apply_meta(meta: &mut Meta, key: &str, value: &str) -> bool {
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

/// Parses a `.mbmap` file of either version. Never fails: problems come back
/// as diagnostics, and `map` is `None` when there is at least one error.
pub fn parse(bytes: &[u8]) -> Parsed {
    let mut ck = Checker { out: Vec::new() };
    let Ok(text) = std::str::from_utf8(bytes) else {
        ck.error(None, "not-utf8", "file is not valid UTF-8".into());
        return Parsed {
            map: None,
            diagnostics: ck.out,
            version: None,
        };
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    // A version 2 file is a JSON object; a version 1 line never starts with '{'.
    let (version, read) = if text.trim_start().starts_with('{') {
        (2, crate::mbmap_v2::read(text, &mut ck))
    } else {
        (1, read_v1(text, &mut ck))
    };
    let map = read.and_then(|(meta, rows)| finish(&mut ck, meta, rows, version));
    if version == 2 {
        // Version 2 names this code after keys, not CSV cells.
        for d in &mut ck.out {
            if d.code == "empty-cell" {
                d.code = "missing-key";
            }
        }
    }
    Parsed {
        map,
        diagnostics: ck.out,
        version: Some(version),
    }
}

/// Version 1: metadata lines, a CSV header and CSV rows. Returns `None` when
/// the file has no usable header.
fn read_v1(text: &str, ck: &mut Checker) -> Option<(Meta, Vec<RawRow>)> {
    let mut meta = Meta::default();
    let mut seen_meta: Vec<String> = Vec::new();
    let mut header: Option<Vec<String>> = None;
    let sep = ',';
    let mut rows: Vec<RawRow> = Vec::new();
    // Comment lines between rows, waiting for the row below them.
    let mut pending: Vec<String> = Vec::new();

    for (index, raw) in text.lines().enumerate() {
        let lineno = index + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            let comment = comment.strip_prefix(' ').unwrap_or(comment).to_string();
            if header.is_none() {
                meta.notes.push(comment);
            } else {
                pending.push(comment);
            }
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
            // The separator is always a comma; a ';' line gets a hint below.
            let semicolon_hint = line.contains(';') && !line.contains(',');
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
                        format!(
                            "header is missing required column {col:?}{}",
                            if semicolon_hint {
                                " (fields are separated by commas; this line uses ';')"
                            } else {
                                ""
                            }
                        ),
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
        let mut row = RawRow {
            at: Some(lineno).into(),
            comment: std::mem::take(&mut pending).join("\n"),
            ..RawRow::default()
        };
        for (i, col) in columns.iter().enumerate() {
            let value = fields.get(i).cloned().unwrap_or_default();
            if row.cells.contains_key(col) {
                continue; // with a duplicated column, the first one wins
            }
            if !KNOWN_COLUMNS.contains(&col.as_str()) && !value.is_empty() {
                row.extra.push((col.clone(), value.clone()));
            }
            row.cells.insert(col.clone(), value);
        }
        rows.push(row);
    }
    // Comments after the last row belong to the file.
    meta.notes.extend(pending);

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
        return None;
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
        return None;
    }
    Some((meta, rows))
}

/// Checks the rows of either version and builds the map, if nothing is wrong.
fn finish(ck: &mut Checker, meta: Meta, rows: Vec<RawRow>, version: u8) -> Option<DeviceMap> {
    if rows.is_empty() {
        ck.warn(None, "no-rows", "no register rows".into());
    }

    let base = i64::from(meta.address_base());
    // Registers of different firmware versions may share a name.
    let mut names: HashMap<String, Vec<(Loc, firmware::Range)>> = HashMap::new();
    // (area, start, end, row index, name) for the overlap check
    let mut spans: Vec<(&'static str, i64, i64, usize, String)> = Vec::new();
    let mut locs: Vec<Loc> = Vec::new();
    let mut ranges: Vec<firmware::Range> = Vec::new();
    let file_range = meta.firmware_range();
    let mut roles_seen: Vec<Role> = Vec::new();
    let mut registers = Vec::new();
    // (location, register index) of rows that name a scale factor register
    let mut scaled: Vec<(Loc, usize)> = Vec::new();

    for (row_index, raw) in rows.into_iter().enumerate() {
        let RawRow {
            at,
            cells: row,
            extra,
            enum_table,
            comment,
        } = raw;
        let line = &at;
        let cell = |col: &str| row.get(col).map(String::as_str).unwrap_or("");
        let errors_before = ck.error_count();

        let empty: Vec<&str> = REQUIRED_COLUMNS
            .into_iter()
            .filter(|c| cell(c).is_empty())
            .collect();
        if !empty.is_empty() {
            for col in empty {
                ck.error(
                    line,
                    "empty-cell",
                    format!("missing or empty required field {col:?}"),
                );
            }
            continue;
        }

        let name = cell("name").to_string();
        let (firmware_min, firmware_max, fw) = if version == 2 {
            check_firmware(
                ck,
                line,
                cell("firmware_min"),
                cell("firmware_max"),
                &file_range,
            )
        } else {
            (None, None, firmware::Range::default())
        };
        ranges.resize(row_index + 1, firmware::Range::default());
        ranges[row_index] = fw.clone();
        let same = names
            .get(&name)
            .and_then(|seen| seen.iter().find(|(_, other)| other.meets(&fw)));
        if let Some((first, _)) = same {
            ck.warn(
                line,
                "duplicate-name",
                format!("name {name:?} already used at {}", first.describe()),
            );
        }
        names
            .entry(name.clone())
            .or_default()
            .push((at.clone(), fw));

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
                format!("address {address} is below address_base={base}"),
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
        let mut limit = |col: &str| -> Option<f64> {
            let text = cell(col);
            if text.is_empty() {
                return None;
            }
            let v = parse_number(text);
            if v.is_none() {
                ck.error(
                    line,
                    "bad-number",
                    format!("{col} is not a number: {text:?}"),
                );
            }
            v
        };
        let write_min = limit("write_min");
        let write_max = limit("write_max");
        if let (Some(lo), Some(hi)) = (write_min, write_max)
            && lo > hi
        {
            ck.error(
                line,
                "bad-write-limit",
                format!("write_min {lo} is greater than write_max {hi}"),
            );
        }
        if (!cell("write_min").is_empty() || !cell("write_max").is_empty())
            && (matches!(ty, DataType::Bool | DataType::String) || access == Access::R)
        {
            ck.warn(
                line,
                "unused-write-limit",
                "write_min/write_max have no effect on this row".into(),
            );
        }
        let reference_text = cell("reference");
        let reference = if reference_text.is_empty() {
            None
        } else if access == Access::W {
            ck.error(
                line,
                "bad-reference",
                "reference on a row that cannot be read".into(),
            );
            None
        } else if !valid_reference(
            ty,
            bits,
            reference_text,
            length_ok.then_some(length).flatten(),
        ) {
            ck.error(
                line,
                "bad-reference",
                format!(
                    "invalid reference {reference_text:?} for type {}",
                    type_name(ty)
                ),
            );
            None
        } else {
            Some(canonical_reference(ty, bits, reference_text))
        };

        let role_text = if version == 2 { cell("role") } else { "" };
        let role = if role_text.is_empty() {
            None
        } else {
            match Role::parse(role_text) {
                None => {
                    ck.error(
                        line,
                        "bad-role",
                        format!(
                            "role must be \"model\", \"firmware\" or \"serial\", not {role_text:?}"
                        ),
                    );
                    None
                }
                Some(_) if access == Access::W => {
                    ck.error(
                        line,
                        "bad-role",
                        "a register with a role must be readable".into(),
                    );
                    None
                }
                Some(r) if r != Role::Firmware && roles_seen.contains(&r) => {
                    ck.error(
                        line,
                        "bad-role",
                        format!("only one register can have the role {:?}", r.name()),
                    );
                    None
                }
                Some(r) => {
                    roles_seen.push(r);
                    Some(r)
                }
            }
        };

        let enum_text = cell("enum");
        let mut enum_ref = None;
        let enum_labels = if let Some(table) = enum_table {
            table
        } else if enum_text.is_empty() {
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
                        format!("no enum table named {name:?}"),
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
            spans.push((area_name(area), wire, wire + count, row_index, name.clone()));
        }
        let scale_factor = Some(cell("scale_factor").to_string()).filter(|s| !s.is_empty());
        if scale_factor.is_some() {
            scaled.push((at.clone(), registers.len()));
        }
        locs.resize(row_index + 1, Loc::default());
        locs[row_index] = at.clone();
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
            write_min,
            write_max,
            reference,
            byte_order,
            bit,
            bits,
            enum_labels,
            enum_ref,
            description: cell("description").to_string(),
            extra,
            comment,
            firmware_min,
            firmware_max,
            role,
        });
    }

    // A scale factor names an integer register of the same map that does not
    // itself depend on a scale factor.
    for (at, i) in scaled {
        let reg = &registers[i];
        let target = reg.scale_factor.as_deref().unwrap_or_default();
        let Some(sf) = registers.iter().find(|r| r.name == target) else {
            ck.error(
                &at,
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
            ck.error(&at, "bad-scale-factor", message);
        }
    }

    spans.sort();
    for i in 0..spans.len() {
        let (area, start, _, row_index, ref name) = spans[i];
        if let Some((_, _, _, first_index, first_name)) = spans[..i]
            .iter()
            .rev()
            // Rows starting at the same address are alternative readings,
            // and rows of different firmware versions never meet.
            .find(|(a, s, end, other, _)| {
                *a == area
                    && *s != start
                    && start < *end
                    && ranges[*other].meets(&ranges[row_index])
            })
        {
            ck.warn(
                &locs[row_index],
                "overlap",
                format!(
                    "{name:?} overlaps {first_name:?} ({})",
                    locs[*first_index].describe()
                ),
            );
        }
    }

    (ck.error_count() == 0).then_some(DeviceMap { meta, registers })
}

/// A register's firmware range (spec section 3.2): the bounds as written,
/// when valid, and the parsed range (open where a bound is invalid).
fn check_firmware(
    ck: &mut Checker,
    at: &Loc,
    min: &str,
    max: &str,
    file_range: &firmware::Range,
) -> (Option<String>, Option<String>, firmware::Range) {
    let mut bound = |key: &str, text: &str| {
        if text.is_empty() {
            return None;
        }
        let v = firmware::parse(text);
        if v.is_none() {
            ck.error(
                at,
                "bad-version",
                format!("{key} {text:?} is not a version like 2.1"),
            );
        }
        v.map(|v| (text.to_string(), v))
    };
    let (lo, hi) = (bound("firmware_min", min), bound("firmware_max", max));
    let range = firmware::Range {
        min: lo.as_ref().map(|(_, v)| v.clone()),
        max: hi.as_ref().map(|(_, v)| v.clone()),
    };
    if range.is_inverted() {
        ck.error(
            at,
            "bad-firmware-range",
            format!("firmware_min {min} is after firmware_max {max}"),
        );
        return (None, None, firmware::Range::default());
    }
    if !range.is_open() && !range.meets(file_range) {
        ck.warn(
            at,
            "firmware-outside-map",
            "this register's firmware range is outside the map's, so it never applies".into(),
        );
    }
    (lo.map(|(t, _)| t), hi.map(|(t, _)| t), range)
}

/// Version 2 `byte_order` and `word_order` of a version 1 byte order (SPEC §9.3).
pub(crate) fn split_order(order: ByteOrder) -> (&'static str, &'static str) {
    match order {
        ByteOrder::ABCD => ("big", "big"),
        ByteOrder::BADC => ("little", "big"),
        ByteOrder::CDAB => ("big", "little"),
        ByteOrder::DCBA => ("little", "little"),
    }
}

/// The byte order with the given byte and word parts (`true` = big).
pub(crate) fn join_order(byte_big: bool, word_big: bool) -> ByteOrder {
    match (byte_big, word_big) {
        (true, true) => ByteOrder::ABCD,
        (false, true) => ByteOrder::BADC,
        (true, false) => ByteOrder::CDAB,
        (false, false) => ByteOrder::DCBA,
    }
}

const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/nobrega8/mbmap-library/main/schema/mbmap-2.schema.json";

/// Integers beyond this are written as strings: not every JSON parser reads
/// them exactly as numbers.
const SAFE_INTEGER: i128 = (1 << 53) - 1;

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// A number as JSON, without a needless `.0` on whole values.
fn json_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        serde_json::to_string(&n).unwrap_or_else(|_| "0".into())
    }
}

/// An integer written as text in a map: kept as a string when it is hex or
/// too large for a JSON number, otherwise a JSON integer.
fn json_integer(text: &str) -> String {
    match parse_i128(text) {
        Some(n) if !text.to_ascii_lowercase().contains("0x") && n.abs() <= SAFE_INTEGER => {
            n.to_string()
        }
        _ => json_str(text),
    }
}

fn json_table<V: std::fmt::Display>(entries: impl Iterator<Item = (V, String)>) -> String {
    let pairs: Vec<String> = entries
        .map(|(v, l)| format!("{}: {}", json_str(&v.to_string()), json_str(&l)))
        .collect();
    format!("{{{}}}", pairs.join(", "))
}

fn source_json(s: &Source) -> String {
    let mut fields = vec![format!("\"title\": {}", json_str(&s.title))];
    for (key, value) in [
        ("version", &s.version),
        ("date", &s.date),
        ("url", &s.url),
        ("note", &s.note),
    ] {
        if let Some(v) = value {
            fields.push(format!("{}: {}", json_str(key), json_str(v)));
        }
    }
    format!("{{{}}}", fields.join(", "))
}

fn reference_json(r: &Register, text: &str) -> String {
    if r.bits.is_some() || sentinel_range(r.ty).is_some() {
        return json_integer(text);
    }
    match r.ty {
        DataType::Bool => matches!(text.to_ascii_lowercase().as_str(), "1" | "true").to_string(),
        DataType::Float32 | DataType::Float64 => {
            parse_number(text).map_or_else(|| json_str(text), json_number)
        }
        _ => json_str(text),
    }
}

/// Writes a map as a version 2 `.mbmap.json` document, one register per line
/// (the layout of the library's `tools/migrate.py`).
pub fn to_string(map: &DeviceMap) -> String {
    let m = &map.meta;
    let mut top: Vec<(String, String)> = vec![
        ("$schema".into(), json_str(SCHEMA_URL)),
        ("mbmap".into(), "2".into()),
        ("vendor".into(), json_str(&m.vendor)),
        ("model".into(), json_str(&m.model)),
    ];
    if !m.models.is_empty() {
        let names: Vec<String> = m.models.iter().map(|n| json_str(n)).collect();
        top.push(("models".into(), format!("[{}]", names.join(", "))));
    }
    let mut text = |key: &str, value: &Option<String>| {
        if let Some(v) = value {
            top.push((key.into(), json_str(v)));
        }
    };
    text("vendor_website", &m.vendor_website);
    text("description", &m.description);
    if let Some(r) = m.revision {
        top.push(("revision".into(), r.to_string()));
    }
    let mut text = |key: &str, value: &Option<String>| {
        if let Some(v) = value {
            top.push((key.into(), json_str(v)));
        }
    };
    text("revised", &m.revised);
    text("firmware", &m.firmware);
    text("firmware_min", &m.firmware_min);
    text("firmware_max", &m.firmware_max);
    text("source", &m.source);
    if !m.sources.is_empty() {
        let lines: Vec<String> = m
            .sources
            .iter()
            .map(|s| format!("    {}", source_json(s)))
            .collect();
        top.push(("sources".into(), format!("[\n{}\n  ]", lines.join(",\n"))));
    }
    if !m.notes.is_empty() {
        let lines: Vec<String> = m
            .notes
            .iter()
            .map(|n| format!("    {}", json_str(n)))
            .collect();
        top.push(("notes".into(), format!("[\n{}\n  ]", lines.join(",\n"))));
    }
    let mut number = |key: &str, value: Option<String>| {
        if let Some(v) = value {
            top.push((key.into(), v));
        }
    };
    number(
        "protocol",
        m.protocol.map(|p| {
            json_str(match p {
                Protocol::Tcp => "tcp",
                Protocol::Rtu => "rtu",
                Protocol::TcpRtu => "tcp+rtu",
            })
        }),
    );
    number("unit_id", m.unit_id.map(|v| v.to_string()));
    number("port", m.port.map(|v| v.to_string()));
    number("baudrate", m.baudrate.map(|v| v.to_string()));
    number("parity", m.parity.map(|p| json_str(&format!("{p:?}"))));
    number("databits", m.data_bits.map(|v| v.to_string()));
    number("stopbits", m.stop_bits.map(|v| v.to_string()));
    if let Some(order) = m.byte_order {
        let (byte, word) = split_order(order);
        number("byte_order", Some(json_str(byte)));
        number("word_order", Some(json_str(word)));
    }
    number("address_base", m.address_base.map(|v| v.to_string()));
    number(
        "max_read_registers",
        m.max_read_registers.map(|v| v.to_string()),
    );
    number("max_read_bits", m.max_read_bits.map(|v| v.to_string()));
    let block = |entries: Vec<String>| format!("{{\n{}\n  }}", entries.join(",\n"));
    if !m.enums.is_empty() {
        let entries = m
            .enums
            .iter()
            .map(|(name, labels)| {
                format!(
                    "    {}: {}",
                    json_str(name),
                    json_table(labels.iter().cloned())
                )
            })
            .collect();
        top.push(("enums".into(), block(entries)));
    }
    if !m.sentinels.is_empty() {
        let entries = m
            .sentinels
            .iter()
            .map(|(ty, table)| {
                let keys = table.iter().map(|(v, l)| {
                    let key = if *v < 0 {
                        v.to_string()
                    } else {
                        format!("0x{v:X}")
                    };
                    (key, l.clone())
                });
                format!("    {}: {}", json_str(type_name(*ty)), json_table(keys))
            })
            .collect();
        top.push(("sentinels".into(), block(entries)));
    }
    for (k, v) in &m.extra {
        top.push((k.clone(), json_str(v)));
    }

    let base = u32::from(m.address_base());
    let registers: Vec<String> = map
        .registers
        .iter()
        .map(|r| {
            let mut f: Vec<(&str, String)> = vec![("name", json_str(&r.name))];
            if !r.group.is_empty() {
                f.push(("group", json_str(&r.group)));
            }
            f.push(("area", json_str(area_name(r.area))));
            f.push(("address", (u32::from(r.address) + base).to_string()));
            f.push(("type", json_str(type_name(r.ty))));
            if let Some(n) = r.length {
                f.push(("length", n.to_string()));
            }
            if let Some(b) = r.bit {
                f.push(("bit", b.to_string()));
            }
            if let Some(b) = r.bits {
                f.push(("bits", b.to_string()));
            }
            if r.scale != 1.0 {
                f.push(("scale", json_number(r.scale)));
            }
            if let Some(sf) = &r.scale_factor {
                f.push(("scale_factor", json_str(sf)));
            }
            if r.offset != 0.0 {
                f.push(("offset", json_number(r.offset)));
            }
            if !r.unit.is_empty() {
                f.push(("unit", json_str(&r.unit)));
            }
            if r.access != Access::R {
                f.push(("access", json_str(&format!("{:?}", r.access))));
            }
            if let Some(order) = r.byte_order {
                let (byte, word) = split_order(order);
                f.push(("byte_order", json_str(byte)));
                f.push(("word_order", json_str(word)));
            }
            if let Some(v) = r.write_min {
                f.push(("write_min", json_number(v)));
            }
            if let Some(v) = r.write_max {
                f.push(("write_max", json_number(v)));
            }
            match &r.enum_ref {
                Some(name) => f.push(("enum", json_str(name))),
                None if !r.enum_labels.is_empty() => {
                    f.push(("enum", json_table(r.enum_labels.iter().cloned())))
                }
                None => {}
            }
            if let Some(text) = &r.reference {
                f.push(("reference", reference_json(r, text)));
            }
            if let Some(v) = &r.firmware_min {
                f.push(("firmware_min", json_str(v)));
            }
            if let Some(v) = &r.firmware_max {
                f.push(("firmware_max", json_str(v)));
            }
            if let Some(role) = r.role {
                f.push(("role", json_str(role.name())));
            }
            if !r.description.is_empty() {
                f.push(("description", json_str(&r.description)));
            }
            if !r.comment.is_empty() {
                f.push(("comment", json_str(&r.comment)));
            }
            let mut fields: Vec<String> = f
                .into_iter()
                .map(|(k, v)| format!("{}: {v}", json_str(k)))
                .collect();
            fields.extend(
                r.extra
                    .iter()
                    .map(|(k, v)| format!("{}: {}", json_str(k), json_str(v))),
            );
            format!("    {{{}}}", fields.join(", "))
        })
        .collect();
    top.push((
        "registers".into(),
        if registers.is_empty() {
            "[]".into()
        } else {
            format!("[\n{}\n  ]", registers.join(",\n"))
        },
    ));

    let body: Vec<String> = top
        .iter()
        .map(|(k, v)| format!("  {}: {v}", json_str(k)))
        .collect();
    format!("{{\n{}\n}}\n", body.join(",\n"))
}

/// File name of a map converted to version 2: `x.mbmap` becomes `x.mbmap.json`.
pub fn v2_file_name(name: &str) -> String {
    if name.to_ascii_lowercase().ends_with(".mbmap.json") {
        name.to_string()
    } else {
        format!("{name}.json")
    }
}

/// Whether a file name looks like a map of either version.
pub fn is_map_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".mbmap") || lower.ends_with(".mbmap.json")
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
    fn write_limits_and_references_round_trip() {
        let cols = "name,area,address,type,scale,unit,access,write_min,write_max,reference\n";
        let map = ok(&format!(
            "{HEAD}{cols}Setpoint,holding,100,uint16,0.1,C,RW,5,30,\nCode,holding,101,uint16,,,R,,,0x016A\n"
        ));
        let r = &map.registers[0];
        assert_eq!((r.write_min, r.write_max), (Some(5.0), Some(30.0)));
        assert_eq!(map.registers[1].reference.as_deref(), Some("0x016A"));
        let text = to_string(&map);
        assert!(
            text.contains("write_min") && text.contains("reference"),
            "{text}"
        );
        assert_eq!(parse(text.as_bytes()).map.unwrap(), map);
        // Columns nobody uses are not written.
        let plain = ok(&format!(
            "{HEAD}name,area,address,type\nA,holding,0,int16\n"
        ));
        let written = to_string(&plain);
        assert!(!written.contains("write_min") && !written.contains("reference"));

        let codes = |body: &str| {
            parse(format!("{HEAD}{cols}{body}").as_bytes())
                .diagnostics
                .iter()
                .map(|d| d.code)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            codes("A,holding,0,uint16,,,RW,30,5,\n"),
            ["bad-write-limit"]
        );
        assert_eq!(codes("A,holding,0,uint16,,,RW,low,,\n"), ["bad-number"]);
        assert_eq!(
            codes("A,holding,0,uint16,,,R,0,10,\n"),
            ["unused-write-limit"]
        );
        assert_eq!(codes("A,holding,0,uint16,,,R,,,70000\n"), ["bad-reference"]);
        assert_eq!(codes("A,holding,0,uint16,,,W,,,5\n"), ["bad-reference"]);
        assert!(codes("A,holding,0,uint16,,,R,,,0xFFFF\n").is_empty());
    }

    #[test]
    fn reference_comparison() {
        let mut reg =
            parse(format!("{HEAD}name,area,address,type\nA,holding,0,uint16\n").as_bytes())
                .map
                .unwrap()
                .registers
                .remove(0);
        assert_eq!(reference_matches(&reg, &Value::Uint(1)), None);
        reg.reference = Some("0x16A".into());
        assert_eq!(reference_matches(&reg, &Value::Uint(362)), Some(true));
        assert_eq!(reference_matches(&reg, &Value::Uint(363)), Some(false));
        reg.ty = DataType::Float32;
        reg.reference = Some("0.1".into());
        assert_eq!(
            reference_matches(&reg, &Value::Float(0.1f32 as f64)),
            Some(true)
        );
        reg.ty = DataType::String;
        reg.reference = Some("SIM-1".into());
        assert_eq!(
            reference_matches(&reg, &Value::Text("SIM-1  ".into())),
            Some(true)
        );
        reg.ty = DataType::Bool;
        reg.reference = Some("true".into());
        assert_eq!(reference_matches(&reg, &Value::Bool(false)), Some(false));
    }

    #[test]
    fn only_comma_separates_fields() {
        let parsed = parse(format!("{HEAD}name;area;address;type\nA;holding;0;int16\n").as_bytes());
        assert!(parsed.map.is_none());
        let first = parsed.errors().next().unwrap();
        assert_eq!(first.code, "missing-column");
        assert!(first.message.contains("commas"), "{}", first.message);
        // A semicolon inside a field is just text.
        let map = ok(&format!(
            "{HEAD}name,area,address,type,description\nA;B,holding,0,int16,x;y\n"
        ));
        assert_eq!(map.registers[0].name, "A;B");
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
        assert!(to_string(&map).contains(r#""vendor_website": "se.com""#));
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
        assert!(text.contains(r##""name": "#1 pump""##), "{text}");
        assert!(text.contains(r#""name": "@home""#), "{text}");
        assert!(text.contains(r#""flow, \"raw\"""#), "{text}");
        assert!(text.contains(r#""x_tool": "1""#), "{text}");
        let again = parse(text.as_bytes());
        assert_eq!(again.version, Some(2));
        assert_eq!(again.map.unwrap(), map);
    }

    const V2: &str = r#"{
  "mbmap": 2,
  "vendor": "Test",
  "model": "T1",
  "byte_order": "big",
  "word_order": "little",
  "enums": {"relay": {"0": "Open", "1": "Closed"}},
  "registers": [
    {"name": "Energy", "area": "holding", "address": "0x10", "type": "uint32", "scale": 0.01},
    {"name": "Swapped", "area": "holding", "address": 20, "type": "uint32", "byte_order": "little"},
    {"name": "Relay", "area": "holding", "address": 30, "type": "bool", "bit": 3, "enum": "relay"},
    {"name": "Mode", "area": "holding", "address": 31, "type": "uint16", "enum": {"0": "A|B", "1": "x=y"}, "reference": "0x0001"},
    {"name": "Seen", "area": "coil", "address": 0, "type": "bool", "reference": true, "x_note": 5}
  ]
}"#;

    #[test]
    fn reads_version_2() {
        let parsed = parse(V2.as_bytes());
        assert_eq!(parsed.version, Some(2));
        assert!(parsed.diagnostics.is_empty(), "{:#?}", parsed.diagnostics);
        let map = parsed.map.unwrap();
        assert_eq!(map.meta.byte_order(), ByteOrder::CDAB);
        let r = &map.registers;
        assert_eq!((r[0].address, r[0].scale), (16, 0.01));
        assert_eq!(r[0].byte_order(&map.meta), ByteOrder::CDAB);
        // A register that sets only one part keeps the file's other part.
        assert_eq!(r[1].byte_order(&map.meta), ByteOrder::DCBA);
        assert_eq!(r[2].enum_ref.as_deref(), Some("relay"));
        assert_eq!(r[2].enum_labels.len(), 2);
        // Inline labels may hold '|' and '=' in version 2.
        assert_eq!(r[3].enum_labels[0], (0, "A|B".to_string()));
        assert_eq!(r[3].reference.as_deref(), Some("0x0001"));
        assert_eq!(r[4].reference.as_deref(), Some("true"));
        assert_eq!(r[4].extra, [("x_note".to_string(), "5".to_string())]);
        // Written back and read again, nothing changes.
        assert_eq!(parse(to_string(&map).as_bytes()).map.unwrap(), map);
    }

    #[test]
    fn version_2_diagnostics_point_into_the_document() {
        let diags = |text: &str| {
            parse(text.as_bytes())
                .diagnostics
                .into_iter()
                .map(|d| (d.code, d.pointer.or(d.line.map(|l| format!("line {l}")))))
                .collect::<Vec<_>>()
        };
        let doc = |regs: &str| {
            format!(r#"{{"mbmap": 2, "vendor": "T", "model": "M", "registers": [{regs}]}}"#)
        };
        let at = |p: &str| Some(p.to_string());
        assert_eq!(
            diags(&doc(
                r#"{"name": "A", "area": "Holding", "address": 0, "type": "int16"}"#
            )),
            [("bad-area", at("/registers/0/area"))]
        );
        assert_eq!(
            diags(&doc(
                r#"{"name": "A", "area": "holding", "address": 0, "type": "int16", "scale": "2"}"#
            )),
            [("bad-json-type", at("/registers/0/scale"))]
        );
        assert_eq!(
            diags(&doc(
                r#"{"name": "A", "area": "holding", "address": 0, "type": "int32", "word_order": "middle"}"#
            )),
            [("bad-byte-order", at("/registers/0/word_order"))]
        );
        assert_eq!(
            diags(&doc(
                r#"{"name": "A", "area": "holding", "address": 0, "type": "int16", "enum": "nope"}"#
            )),
            [("unknown-enum", at("/registers/0"))]
        );
        assert_eq!(
            diags(&doc(
                r#"{"name": "A", "area": "holding", "address": 0, "type": "int16", "colour": "red"}"#
            )),
            [("unknown-key", at("/registers/0/colour"))]
        );
        assert_eq!(
            diags(&doc(
                r#"{"name": "A", "area": "holding", "address": 0, "type": "int32"},
                   {"name": "B", "area": "holding", "address": 1, "type": "int16"}"#
            )),
            [("overlap", at("/registers/1"))]
        );
        assert_eq!(
            diags(r#"{"mbmap": 2, "vendor": "T", "vendor": "U", "model": "M", "registers": []}"#),
            [("not-json", None)]
        );
        assert_eq!(
            diags(r#"{"mbmap": 3, "vendor": "T", "model": "M"}"#),
            [("bad-meta-value", at("/mbmap")), ("missing-key", None)]
        );
    }

    #[test]
    fn version_1_is_still_read() {
        let parsed = parse(format!("{HEAD}name,area,address,type\nA,holding,0,int16\n").as_bytes());
        assert_eq!(parsed.version, Some(1));
        assert!(parsed.map.is_some());
    }

    #[test]
    fn version_1_comments_become_notes() {
        let map = ok(&format!(
            "# About the file\n{HEAD}name,area,address,type\n# Section\nA,holding,0,int16\n# End\n"
        ));
        assert_eq!(map.meta.notes, ["About the file", "End"]);
        assert_eq!(map.registers[0].comment, "Section");
        let text = to_string(&map);
        assert!(text.contains(r#""comment": "Section""#), "{text}");
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
