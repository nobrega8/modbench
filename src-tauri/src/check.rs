//! The commissioning check: every register of a map read once, and a status
//! for each one (pass, fail or not read), plus what the device says about
//! its model and firmware against what the map declares. The same result
//! feeds the CLI's `check` and the app's Check device, and becomes a single
//! HTML file. Nothing here writes to a device.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::codec::Value;
use crate::firmware;
use crate::mapfind::text_of;
use crate::mbmap::{DeviceMap, Register, Role};
use crate::modbus::RawData;
use crate::poller::PointResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    NotRead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckRow {
    pub name: String,
    pub group: String,
    pub address: u16,
    #[serde(rename = "type")]
    pub ty: String,
    pub unit: String,
    pub value: Option<String>,
    pub raw: Option<String>,
    pub status: Status,
    /// Why it failed or was not read.
    pub reason: Option<String>,
}

/// A check that is not about one register: the model or the firmware.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub what: String,
    pub status: Status,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    pub rows: Vec<CheckRow>,
    pub findings: Vec<Finding>,
    pub pass: usize,
    pub fail: usize,
    pub not_read: usize,
}

impl CheckReport {
    /// Whether any check failed (rows or findings).
    pub fn failed(&self) -> bool {
        self.fail > 0
    }
}

/// A value in engineering units as text, with its label when it has one.
pub fn format_value(reg: &Register, value: &Value) -> String {
    let label = |raw: i64| {
        reg.enum_labels
            .iter()
            .find(|(v, _)| *v == raw)
            .map(|(_, l)| l.clone())
    };
    match value {
        Value::Bool(b) => label(*b as i64).unwrap_or_else(|| if *b { "ON" } else { "OFF" }.into()),
        Value::Text(t) => t.clone(),
        Value::Int(i) => label(*i).unwrap_or_else(|| i.to_string()),
        Value::Uint(u) => label(*u as i64).unwrap_or_else(|| u.to_string()),
        Value::Float(f) => {
            let raw = ((f - reg.offset) / reg.scale).round() as i64;
            if let Some(l) = label(raw) {
                return l;
            }
            let decimals = if reg.scale > 0.0 && reg.scale < 1.0 {
                (-reg.scale.log10()).ceil().clamp(0.0, 6.0) as usize
            } else {
                3
            };
            let s = format!("{f:.decimals$}");
            if s.contains('.') {
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            } else {
                s
            }
        }
    }
}

/// The words or bits as read, in hex or 0/1.
pub fn format_raw(data: &RawData) -> String {
    match data {
        RawData::Bits(b) => b.iter().map(|&x| if x { '1' } else { '0' }).collect(),
        RawData::Registers(w) => w
            .iter()
            .map(|w| format!("{w:04X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Int(n) => Some(*n as f64),
        Value::Uint(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

/// The status of one register from what its read gave.
fn check_row(reg: &Register, result: Option<&PointResult>) -> CheckRow {
    let mut row = CheckRow {
        name: reg.name.clone(),
        group: reg.group.clone(),
        address: reg.address,
        ty: format!("{:?}", reg.ty).to_lowercase(),
        unit: reg.unit.clone(),
        value: None,
        raw: None,
        status: Status::Pass,
        reason: None,
    };
    if !reg.access.readable() {
        row.status = Status::NotRead;
        row.reason = Some("write-only".into());
        return row;
    }
    let Some(r) = result else {
        row.status = Status::NotRead;
        row.reason = Some("not read".into());
        return row;
    };
    row.raw = r.raw.as_ref().map(format_raw);
    if let Some(error) = &r.error {
        row.status = Status::Fail;
        row.reason = Some(error.clone());
        return row;
    }
    if let Some(label) = &r.sentinel {
        row.status = Status::NotRead;
        row.reason = Some(format!("the device reports: {label}"));
        return row;
    }
    let Some(value) = &r.value else {
        row.status = Status::NotRead;
        row.reason = Some("no value".into());
        return row;
    };
    let shown = format_value(reg, value);
    row.value = Some(shown.clone());
    if r.reference == Some("mismatch") {
        let expected = reg.reference.clone().unwrap_or_default();
        row.status = Status::Fail;
        row.reason = Some(format!(
            "differs from the reference value {expected} (raw {})",
            row.raw.clone().unwrap_or_default()
        ));
    } else if let Some(n) = number(value)
        && (reg.write_min.is_some_and(|min| n < min) || reg.write_max.is_some_and(|max| n > max))
    {
        let limit = |v: Option<f64>| v.map_or("…".to_string(), |v| v.to_string());
        row.status = Status::Fail;
        row.reason = Some(format!(
            "{shown} is outside the declared limits {} to {}",
            limit(reg.write_min),
            limit(reg.write_max)
        ));
    }
    row
}

/// The model and the firmware the device reports, against the map.
fn findings(map: &DeviceMap, results: &HashMap<usize, &PointResult>) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut firmware_parts = Vec::new();
    for (i, reg) in map.registers.iter().enumerate() {
        let Some(role @ (Role::Model | Role::Firmware)) = reg.role else {
            continue;
        };
        let Some(r) = results.get(&i) else { continue };
        let Some(value) = r.value.as_ref().filter(|_| r.error.is_none()) else {
            continue;
        };
        let found = text_of(reg, value);
        if role == Role::Model {
            let names: Vec<&String> = if map.meta.models.is_empty() {
                vec![&map.meta.model]
            } else {
                map.meta.models.iter().collect()
            };
            let ok = names
                .iter()
                .any(|n| n.trim().eq_ignore_ascii_case(found.trim()));
            out.push(Finding {
                what: "Model".into(),
                status: if ok { Status::Pass } else { Status::Fail },
                detail: if ok {
                    found
                } else {
                    format!(
                        "the device says {found}; the map is for {}",
                        names
                            .iter()
                            .map(|n| n.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                },
            });
        } else {
            firmware_parts.push(found);
        }
    }
    if !firmware_parts.is_empty() {
        let text = firmware_parts.join(".");
        let range = map.meta.firmware_range();
        let range_text = {
            let part = |v: &Option<firmware::Version>| {
                v.as_ref()
                    .map_or("any".to_string(), |v| firmware::display(v))
            };
            format!("{} to {}", part(&range.min), part(&range.max))
        };
        out.push(match firmware::find(&text) {
            Some(version) if range.is_open() => Finding {
                what: "Firmware".into(),
                status: Status::Pass,
                detail: format!(
                    "{} (the map declares no range)",
                    firmware::display(&version)
                ),
            },
            Some(version) if range.contains(&version) => Finding {
                what: "Firmware".into(),
                status: Status::Pass,
                detail: format!(
                    "{} (the map covers {range_text})",
                    firmware::display(&version)
                ),
            },
            Some(version) => Finding {
                what: "Firmware".into(),
                status: Status::Fail,
                detail: format!(
                    "{} is outside the map's range ({range_text})",
                    firmware::display(&version)
                ),
            },
            None => Finding {
                what: "Firmware".into(),
                status: Status::NotRead,
                detail: format!("could not read a version from {text:?}"),
            },
        });
    }
    out
}

/// Checks every register of `map` against what one read of all of them
/// gave. `results` hold `PointResult::index` as a position in
/// `map.registers`.
pub fn evaluate(map: &DeviceMap, results: &[PointResult]) -> CheckReport {
    let by_index: HashMap<usize, &PointResult> = results.iter().map(|r| (r.index, r)).collect();
    let rows: Vec<CheckRow> = map
        .registers
        .iter()
        .enumerate()
        .map(|(i, reg)| check_row(reg, by_index.get(&i).copied()))
        .collect();
    let findings = findings(map, &by_index);
    let count = |s: Status| {
        rows.iter().filter(|r| r.status == s).count()
            + findings.iter().filter(|f| f.status == s).count()
    };
    CheckReport {
        pass: count(Status::Pass),
        fail: count(Status::Fail),
        not_read: count(Status::NotRead),
        rows,
        findings,
    }
}

/// What the report says about the run besides the results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportContext {
    /// Name of the map, e.g. "Example Corp EM-100".
    pub map: String,
    /// The map's file, and its revision when it has one.
    pub map_file: String,
    pub map_revision: Option<String>,
    /// How the device was reached, e.g. "TCP 192.168.1.10:502".
    pub connection: String,
    pub unit_id: u8,
    /// What the device says about itself (FC43), as label and value.
    pub identification: Vec<(String, String)>,
    /// When the check ran, as text.
    pub when: String,
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn label(status: Status) -> (&'static str, &'static str) {
    match status {
        Status::Pass => ("pass", "Pass"),
        Status::Fail => ("fail", "Fail"),
        Status::NotRead => ("skip", "Not read"),
    }
}

/// The report as one HTML page, with its style inline: it opens without
/// Modbench, and prints to PDF from any browser.
pub fn html(report: &CheckReport, ctx: &ReportContext) -> String {
    let mut out = String::new();
    let verdict = if report.failed() {
        ("fail", "FAILED")
    } else {
        ("pass", "PASSED")
    };
    out.push_str(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
    );
    out.push_str(&format!(
        "<title>Commissioning report - {}</title>\n",
        escape(&ctx.map)
    ));
    out.push_str(
        "<style>\n\
         body{font:14px/1.45 system-ui,sans-serif;color:#1d1d1f;margin:32px auto;max-width:960px;padding:0 16px}\n\
         h1{font-size:22px;margin:0 0 4px}h2{font-size:16px;margin:28px 0 8px}\n\
         table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:4px 8px;border-bottom:1px solid #ddd;vertical-align:top}\n\
         th{font-size:12px;text-transform:uppercase;color:#666}\n\
         dl{display:grid;grid-template-columns:max-content 1fr;gap:2px 16px;margin:12px 0}dt{color:#666}dd{margin:0}\n\
         .pass{color:#1a7f37}.fail{color:#c0262d;font-weight:600}.skip{color:#777}\n\
         .verdict{display:inline-block;padding:2px 10px;border-radius:4px;font-weight:700;border:2px solid currentColor}\n\
         .mono{font-family:ui-monospace,Menlo,Consolas,monospace;font-size:12px}\n\
         tr.bad td{background:#fdeeee}\n\
         @media print{body{margin:0;max-width:none}tr{break-inside:avoid}}\n\
         </style></head><body>\n",
    );
    out.push_str(&format!(
        "<h1>Commissioning report</h1>\n<p><span class=\"verdict {}\">{}</span> \
         {} passed, {} failed, {} not read</p>\n",
        verdict.0, verdict.1, report.pass, report.fail, report.not_read
    ));
    out.push_str("<dl>\n");
    let mut line = |k: &str, v: &str| {
        out.push_str(&format!("<dt>{}</dt><dd>{}</dd>\n", escape(k), escape(v)));
    };
    line("Date", &ctx.when);
    line("Connection", &ctx.connection);
    line("Unit ID", &ctx.unit_id.to_string());
    for (k, v) in &ctx.identification {
        line(k, v);
    }
    line("Map", &ctx.map);
    line("Map file", &ctx.map_file);
    if let Some(rev) = &ctx.map_revision {
        line("Map revision", rev);
    }
    out.push_str("</dl>\n");

    if !report.findings.is_empty() {
        out.push_str("<h2>Device identity</h2>\n<table>\n");
        for f in &report.findings {
            let (class, text) = label(f.status);
            out.push_str(&format!(
                "<tr><td>{}</td><td class=\"{class}\">{text}</td><td>{}</td></tr>\n",
                escape(&f.what),
                escape(&f.detail)
            ));
        }
        out.push_str("</table>\n");
    }

    out.push_str("<h2>Failures</h2>\n");
    if !report.failed() {
        out.push_str("<p>None.</p>\n");
    } else {
        out.push_str("<table><tr><th>Register</th><th>Address</th><th>Reason</th></tr>\n");
        for f in report.findings.iter().filter(|f| f.status == Status::Fail) {
            out.push_str(&format!(
                "<tr><td>{}</td><td></td><td>{}</td></tr>\n",
                escape(&f.what),
                escape(&f.detail)
            ));
        }
        for r in report.rows.iter().filter(|r| r.status == Status::Fail) {
            out.push_str(&format!(
                "<tr><td>{}</td><td class=\"mono\">{}</td><td>{}</td></tr>\n",
                escape(&r.name),
                r.address,
                escape(r.reason.as_deref().unwrap_or(""))
            ));
        }
        out.push_str("</table>\n");
    }

    out.push_str(
        "<h2>All registers</h2>\n<table>\n<tr><th>Register</th><th>Address</th>\
         <th>Type</th><th>Value</th><th>Status</th><th>Note</th></tr>\n",
    );
    for r in &report.rows {
        let (class, text) = label(r.status);
        let value = match (&r.value, r.unit.is_empty()) {
            (Some(v), false) => format!("{v} {}", r.unit),
            (Some(v), true) => v.clone(),
            (None, _) => String::new(),
        };
        out.push_str(&format!(
            "<tr{}><td>{}</td><td class=\"mono\">{}</td><td class=\"mono\">{}</td><td>{}</td>\
             <td class=\"{class}\">{text}</td><td>{}</td></tr>\n",
            if r.status == Status::Fail {
                " class=\"bad\""
            } else {
                ""
            },
            escape(&r.name),
            r.address,
            escape(&r.ty),
            escape(&value),
            escape(r.reason.as_deref().unwrap_or(""))
        ));
    }
    out.push_str(
        "</table>\n<p class=\"skip\">Made by Modbench. Nothing was written to the device.</p>\n\
         </body></html>\n",
    );
    out
}

/// "2026-10-19 20:05 UTC" for a Unix time in seconds.
pub fn utc_text(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        rest / 3600,
        rest % 3600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(utc_text(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc_text(951_782_400 + 86_399), "2000-02-29 23:59 UTC");
        assert_eq!(utc_text(1_792_440_000), "2026-10-19 20:00 UTC");
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(
            escape("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    fn map() -> DeviceMap {
        let text = r#"{"mbmap": 2, "vendor": "V <b>", "model": "M1",
            "registers": [
              {"name": "Model", "area": "holding", "address": 0, "type": "string", "length": 2, "role": "model"},
              {"name": "Volts", "area": "holding", "address": 2, "type": "uint16", "write_min": 100, "write_max": 300},
              {"name": "Id", "area": "holding", "address": 3, "type": "uint16", "reference": "7"},
              {"name": "Dead", "area": "holding", "address": 4, "type": "uint16"},
              {"name": "Cmd", "area": "holding", "address": 5, "type": "uint16", "access": "W"},
              {"name": "Fine", "area": "holding", "address": 6, "type": "uint16"}
            ]}"#;
        crate::mbmap::parse(text.as_bytes())
            .map
            .expect("a valid map")
    }

    fn point(index: usize, value: Option<Value>) -> PointResult {
        PointResult {
            index,
            value,
            raw: None,
            error: None,
            sentinel: None,
            reference: None,
        }
    }

    #[test]
    fn every_register_gets_a_status() {
        let map = map();
        let mut results = vec![
            point(0, Some(Value::Text("M2".into()))),
            point(1, Some(Value::Uint(500))),
            point(2, Some(Value::Uint(8))),
            point(4, Some(Value::Uint(1))),
            point(5, Some(Value::Uint(1))),
        ];
        results[2].reference = Some("mismatch");
        let mut dead = point(3, None);
        dead.error = Some("timeout".into());
        results.push(dead);

        let report = evaluate(&map, &results);
        let status: Vec<Status> = report.rows.iter().map(|r| r.status).collect();
        assert_eq!(
            status,
            [
                Status::Pass,
                Status::Fail,
                Status::Fail,
                Status::Fail,
                Status::NotRead,
                Status::Pass
            ]
        );
        assert_eq!(report.rows.len(), map.registers.len());
        assert!(
            report.rows[1]
                .reason
                .as_deref()
                .unwrap()
                .contains("outside")
        );
        assert!(
            report.rows[2]
                .reason
                .as_deref()
                .unwrap()
                .contains("reference")
        );
        assert_eq!(report.rows[3].reason.as_deref(), Some("timeout"));
        // The model the device reports is not the one of the map
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].status, Status::Fail);
        assert!(report.failed());
        assert_eq!((report.pass, report.fail, report.not_read), (2, 4, 1));
    }

    #[test]
    fn a_healthy_device_passes() {
        let map = map();
        let results = vec![
            point(0, Some(Value::Text("M1".into()))),
            point(1, Some(Value::Uint(230))),
            point(2, Some(Value::Uint(7))),
            point(3, Some(Value::Uint(0))),
            point(5, Some(Value::Uint(0))),
        ];
        let report = evaluate(&map, &results);
        assert!(!report.failed());
        assert_eq!(report.fail, 0);
    }

    #[test]
    fn the_report_is_one_escaped_page() {
        let map = map();
        let report = evaluate(&map, &[]);
        let ctx = ReportContext {
            map: "V <b> M1".into(),
            map_file: "m.mbmap.json".into(),
            map_revision: Some("3, 2026-01-02".into()),
            connection: "TCP 10.0.0.1:502".into(),
            unit_id: 4,
            identification: vec![("Vendor".into(), "A & B".into())],
            when: utc_text(0),
        };
        let page = html(&report, &ctx);
        assert!(page.starts_with("<!doctype html>"));
        assert!(page.contains("V &lt;b&gt; M1"));
        assert!(!page.contains("<b>"));
        assert!(page.contains("A &amp; B"));
        assert!(page.contains("1970-01-01 00:00 UTC"));
        assert!(!page.contains("src=") && !page.contains("href="));
    }
}
