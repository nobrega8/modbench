//! Test sequences for commissioning: write a value, wait, expect a register
//! to reach a value (or a range) within a time, and read values to record
//! them, all on a device's map registers by name. Saved as `.mbtest.json`
//! next to the map; run from the app or `modbench-cli test`, with an HTML
//! report like the commissioning check's.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::check::{REPORT_STYLE, ReportContext, escape};
use crate::codec::{self, Value};
use crate::commands::{TypedAddress, check_write_limits, read_typed, write_typed};
use crate::mbmap::{DeviceMap, Register};
use crate::modbus::{ModbusError, Session};

/// File extension of a saved sequence.
pub const EXTENSION: &str = "mbtest.json";

/// How often `expect` reads again while it waits.
const POLL: Duration = Duration::from_millis(100);

/// A list of steps on one map's registers.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sequence {
    #[serde(default)]
    pub name: String,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "lowercase")]
pub enum Step {
    /// Write a value in engineering units (1 or 0 for on/off).
    Write { register: String, value: f64 },
    /// Do nothing for a while.
    Wait { ms: u64 },
    /// Read until the value passes, or `within_ms` runs out (0: read once).
    Expect {
        register: String,
        #[serde(flatten)]
        test: Test,
        #[serde(default)]
        within_ms: u64,
    },
    /// Read a value, to record it in the report.
    Read { register: String },
}

/// What an expected value must be: equal (within a tolerance), or in a
/// range (either end may be open).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Test {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equals: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl Test {
    pub fn passes(&self, v: f64) -> bool {
        if let Some(want) = self.equals
            && (v - want).abs() > self.tolerance.unwrap_or(0.0).abs() + 1e-9
        {
            return false;
        }
        self.min.is_none_or(|lo| v >= lo) && self.max.is_none_or(|hi| v <= hi)
    }

    /// "= 230 ± 5", "in 10..20", "≥ 3".
    pub fn text(&self) -> String {
        match (self.equals, self.min, self.max) {
            (Some(v), _, _) => match self.tolerance {
                Some(t) if t != 0.0 => format!("= {v} ± {t}"),
                _ => format!("= {v}"),
            },
            (None, Some(lo), Some(hi)) => format!("in {lo}..{hi}"),
            (None, Some(lo), None) => format!("≥ {lo}"),
            (None, None, Some(hi)) => format!("≤ {hi}"),
            (None, None, None) => "anything".into(),
        }
    }
}

impl Step {
    /// The register a step works on, if any.
    pub fn register(&self) -> Option<&str> {
        match self {
            Step::Write { register, .. }
            | Step::Expect { register, .. }
            | Step::Read { register } => Some(register),
            Step::Wait { .. } => None,
        }
    }

    pub fn writes(&self) -> bool {
        matches!(self, Step::Write { .. })
    }

    /// The step in a few words, for logs and the report.
    pub fn text(&self) -> String {
        match self {
            Step::Write { register, value } => format!("write {register} = {value}"),
            Step::Wait { ms } => format!("wait {ms} ms"),
            Step::Expect {
                register,
                test,
                within_ms,
            } => {
                let within = if *within_ms > 0 {
                    format!(" within {within_ms} ms")
                } else {
                    String::new()
                };
                format!("expect {register} {}{within}", test.text())
            }
            Step::Read { register } => format!("read {register}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// An expectation held.
    Pass,
    /// An expectation did not hold.
    Fail,
    /// The step could not run (no such register, the device did not
    /// answer, writes blocked…).
    Error,
    /// A write, a wait or a read went through.
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub index: usize,
    pub status: Status,
    /// The value seen (read, or the last read of an expectation), with its
    /// unit.
    pub value: Option<String>,
    pub message: Option<String>,
    /// When the step started (Unix ms) and how long it took.
    pub time_ms: u64,
    pub ms: u64,
}

pub fn failed(results: &[StepResult]) -> bool {
    results
        .iter()
        .any(|r| matches!(r.status, Status::Fail | Status::Error))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn find<'a>(map: &'a DeviceMap, name: &str) -> Result<&'a Register, ModbusError> {
    map.registers
        .iter()
        .find(|r| r.name == name)
        .ok_or_else(|| ModbusError::Invalid(format!("{name:?} is not in the map")))
}

fn target(map: &DeviceMap, r: &Register) -> Result<TypedAddress, ModbusError> {
    if r.scale_factor.is_some() {
        return Err(ModbusError::Invalid(format!(
            "{} takes its scale from another register, which sequences do not read",
            r.name
        )));
    }
    Ok(TypedAddress {
        area: r.area,
        address: r.address,
        ty: r.ty,
        byte_order: r.byte_order(&map.meta),
        length: r.length,
        bit: r.bit,
        bits: r.bits,
    })
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Int(n) => Some(*n as f64),
        Value::Uint(n) => Some(*n as f64),
        Value::Float(f) => Some(*f),
        Value::Text(_) => None,
    }
}

fn shown(r: &Register, v: f64) -> String {
    let label = r
        .enum_labels
        .iter()
        .find(|(raw, _)| *raw as f64 == v)
        .map(|(_, l)| format!(" ({l})"))
        .unwrap_or_default();
    let unit = if r.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", r.unit)
    };
    format!("{v}{unit}{label}")
}

/// A register's value in engineering units.
async fn read(session: &mut Session, map: &DeviceMap, r: &Register) -> Result<f64, ModbusError> {
    let raw = read_typed(session, &target(map, r)?).await?;
    let v = codec::to_engineering(raw, r.scale, r.offset);
    number(&v).ok_or_else(|| ModbusError::Invalid(format!("{} is text, not a number", r.name)))
}

async fn write(
    session: &mut Session,
    map: &DeviceMap,
    r: &Register,
    value: f64,
) -> Result<(), ModbusError> {
    if !r.access.writable() {
        return Err(ModbusError::Invalid(format!("{} is read-only", r.name)));
    }
    let t = target(map, r)?;
    let raw = if t.ty == codec::DataType::Bool {
        Value::Bool(value != 0.0)
    } else {
        check_write_limits(r, &Value::Float(value))?;
        codec::from_engineering(Value::Float(value), r.scale, r.offset)
    };
    write_typed(session, &t, &raw).await
}

/// Runs every step in order, whatever happened to the ones before, and
/// reports each as it ends. With `allow_writes` false a write step is an
/// error and nothing is written.
pub async fn run(
    session: &mut Session,
    map: &DeviceMap,
    sequence: &Sequence,
    allow_writes: bool,
    mut on_step: impl FnMut(&StepResult),
) -> Vec<StepResult> {
    let mut results = Vec::with_capacity(sequence.steps.len());
    for (index, step) in sequence.steps.iter().enumerate() {
        let time_ms = now_ms();
        let started = Instant::now();
        let (status, value, message) = match run_step(session, map, step, allow_writes).await {
            Ok((status, value, message)) => (status, value, message),
            Err(e) => (Status::Error, None, Some(e.to_string())),
        };
        let result = StepResult {
            index,
            status,
            value,
            message,
            time_ms,
            ms: started.elapsed().as_millis() as u64,
        };
        on_step(&result);
        results.push(result);
    }
    results
}

type Outcome = (Status, Option<String>, Option<String>);

async fn run_step(
    session: &mut Session,
    map: &DeviceMap,
    step: &Step,
    allow_writes: bool,
) -> Result<Outcome, ModbusError> {
    match step {
        Step::Wait { ms } => {
            tokio::time::sleep(Duration::from_millis(*ms)).await;
            Ok((Status::Done, None, None))
        }
        Step::Write { register, value } => {
            if !allow_writes {
                return Err(ModbusError::ReadOnly);
            }
            let r = find(map, register)?;
            write(session, map, r, *value).await?;
            Ok((Status::Done, Some(shown(r, *value)), None))
        }
        Step::Read { register } => {
            let r = find(map, register)?;
            let v = read(session, map, r).await?;
            Ok((Status::Done, Some(shown(r, v)), None))
        }
        Step::Expect {
            register,
            test,
            within_ms,
        } => {
            let r = find(map, register)?;
            let until = Instant::now() + Duration::from_millis(*within_ms);
            loop {
                let v = read(session, map, r).await?;
                if test.passes(v) {
                    return Ok((Status::Pass, Some(shown(r, v)), None));
                }
                if Instant::now() >= until {
                    let message = format!("expected {}, got {}", test.text(), shown(r, v));
                    return Ok((Status::Fail, Some(shown(r, v)), Some(message)));
                }
                tokio::time::sleep(POLL).await;
            }
        }
    }
}

/// The sequence's results as one HTML page, like the commissioning report.
pub fn html(sequence: &Sequence, results: &[StepResult], ctx: &ReportContext) -> String {
    let count = |s: Status| results.iter().filter(|r| r.status == s).count();
    let verdict = if failed(results) {
        ("fail", "FAILED")
    } else {
        ("pass", "PASSED")
    };
    let title = if sequence.name.is_empty() {
        "Test sequence".to_string()
    } else {
        sequence.name.clone()
    };
    let mut out = String::from(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
    );
    out.push_str(&format!(
        "<title>{} - {}</title>\n{REPORT_STYLE}</head><body>\n",
        escape(&title),
        escape(&ctx.map)
    ));
    out.push_str(&format!(
        "<h1>{}</h1>\n<p><span class=\"verdict {}\">{}</span> {} passed, {} failed, {} errors, {} done</p>\n<dl>\n",
        escape(&title),
        verdict.0,
        verdict.1,
        count(Status::Pass),
        count(Status::Fail),
        count(Status::Error),
        count(Status::Done)
    ));
    for (k, v) in [
        ("Date", ctx.when.as_str()),
        ("Connection", ctx.connection.as_str()),
        ("Map", ctx.map.as_str()),
        ("Map file", ctx.map_file.as_str()),
    ] {
        out.push_str(&format!("<dt>{k}</dt><dd>{}</dd>\n", escape(v)));
    }
    out.push_str(&format!("<dt>Unit ID</dt><dd>{}</dd>\n", ctx.unit_id));
    out.push_str(
        "</dl>\n<h2>Steps</h2>\n<table>\n<tr><th>#</th><th>Step</th><th>Value</th>\
         <th>Result</th><th>Note</th><th>ms</th></tr>\n",
    );
    for (i, step) in sequence.steps.iter().enumerate() {
        let r = results.iter().find(|r| r.index == i);
        let (class, text) = match r.map(|r| r.status) {
            Some(Status::Pass) => ("pass", "Pass"),
            Some(Status::Fail) => ("fail", "Fail"),
            Some(Status::Error) => ("fail", "Error"),
            Some(Status::Done) => ("pass", "Done"),
            None => ("skip", "Not run"),
        };
        let bad = matches!(r.map(|r| r.status), Some(Status::Fail | Status::Error));
        out.push_str(&format!(
            "<tr{}><td>{}</td><td>{}</td><td class=\"mono\">{}</td><td class=\"{class}\">{text}</td><td>{}</td><td class=\"mono\">{}</td></tr>\n",
            if bad { " class=\"bad\"" } else { "" },
            i + 1,
            escape(&step.text()),
            escape(r.and_then(|r| r.value.as_deref()).unwrap_or("")),
            escape(r.and_then(|r| r.message.as_deref()).unwrap_or("")),
            r.map_or(String::new(), |r| r.ms.to_string())
        ));
    }
    let wrote = sequence.steps.iter().any(Step::writes);
    out.push_str(&format!(
        "</table>\n<p class=\"skip\">Made by Modbench.{}</p>\n</body></html>\n",
        if wrote {
            " The sequence wrote to the device."
        } else {
            " Nothing was written to the device."
        }
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbmap;
    use crate::modbus::{ConnectionConfig, Transport};
    use crate::simulator::{self, Device, Memory};
    use std::sync::Arc;

    fn demo() -> DeviceMap {
        mbmap::parse(include_bytes!("demo.mbmap.json")).map.unwrap()
    }

    async fn session() -> (simulator::Running, Session) {
        let map = demo();
        let devices = Arc::new(std::sync::Mutex::new(vec![Device::new(
            Some(1),
            Memory::from_map(&map, false),
        )]));
        let listen = Transport::Tcp {
            host: "127.0.0.1".into(),
            port: 0,
        };
        let sim = simulator::start(listen.clone(), devices, None)
            .await
            .unwrap();
        let config = ConnectionConfig {
            transport: Transport::Tcp {
                host: "127.0.0.1".into(),
                port: sim.port().unwrap(),
            },
            unit_id: 1,
            timeout_ms: 1000,
            local_address: None,
        };
        let s = Session::connect(&config, None).await.unwrap();
        (sim, s)
    }

    fn sequence(steps: Vec<Step>) -> Sequence {
        Sequence {
            name: "Setpoint".into(),
            steps,
        }
    }

    #[test]
    fn files_read_back_as_written() {
        let text = r#"{"name":"t","steps":[
            {"do":"write","register":"Setpoint","value":42},
            {"do":"wait","ms":100},
            {"do":"expect","register":"Setpoint","equals":42,"tolerance":0.5,"within_ms":1000},
            {"do":"expect","register":"Frequency","min":49,"max":51},
            {"do":"read","register":"Frequency"}]}"#;
        let seq: Sequence = serde_json::from_str(text).unwrap();
        assert_eq!(seq.steps.len(), 5);
        let again: Sequence = serde_json::from_str(&serde_json::to_string(&seq).unwrap()).unwrap();
        assert_eq!(again, seq);
        assert_eq!(
            seq.steps[2].text(),
            "expect Setpoint = 42 ± 0.5 within 1000 ms"
        );
        assert_eq!(seq.steps[3].text(), "expect Frequency in 49..51");
    }

    #[test]
    fn tests_pass_and_fail() {
        let eq = Test {
            equals: Some(230.0),
            tolerance: Some(5.0),
            ..Test::default()
        };
        assert!(eq.passes(234.9) && !eq.passes(236.0));
        let range = Test {
            min: Some(1.0),
            ..Test::default()
        };
        assert!(range.passes(1.0) && !range.passes(0.5));
    }

    #[tokio::test]
    async fn writes_waits_and_expects_against_the_simulator() {
        let (_sim, mut s) = session().await;
        let seq = sequence(vec![
            Step::Write {
                register: "Setpoint".into(),
                value: 42.0,
            },
            Step::Expect {
                register: "Setpoint".into(),
                test: Test {
                    equals: Some(42.0),
                    ..Test::default()
                },
                within_ms: 500,
            },
            Step::Expect {
                register: "Frequency".into(),
                test: Test {
                    min: Some(49.0),
                    max: Some(51.0),
                    ..Test::default()
                },
                within_ms: 0,
            },
            Step::Read {
                register: "Frequency".into(),
            },
        ]);
        let mut seen = 0;
        let results = run(&mut s, &demo(), &seq, true, |_| seen += 1).await;
        assert_eq!(seen, 4);
        let statuses: Vec<_> = results.iter().map(|r| r.status).collect();
        assert_eq!(
            statuses,
            [Status::Done, Status::Pass, Status::Pass, Status::Done]
        );
        assert_eq!(results[3].value.as_deref(), Some("50 Hz"));
        assert!(!failed(&results));
    }

    #[tokio::test]
    async fn a_failed_expectation_waits_its_time_and_the_rest_still_runs() {
        let (_sim, mut s) = session().await;
        let seq = sequence(vec![
            Step::Expect {
                register: "Frequency".into(),
                test: Test {
                    equals: Some(60.0),
                    ..Test::default()
                },
                within_ms: 300,
            },
            Step::Read {
                register: "Nope".into(),
            },
            Step::Read {
                register: "Frequency".into(),
            },
        ]);
        let started = Instant::now();
        let results = run(&mut s, &demo(), &seq, true, |_| {}).await;
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert_eq!(results[0].status, Status::Fail);
        assert!(
            results[0]
                .message
                .as_deref()
                .unwrap()
                .contains("expected = 60")
        );
        assert_eq!(results[1].status, Status::Error);
        assert_eq!(results[2].status, Status::Done);
        assert!(failed(&results));
    }

    #[tokio::test]
    async fn without_writes_allowed_nothing_is_written() {
        let (_sim, mut s) = session().await;
        let seq = sequence(vec![
            Step::Write {
                register: "Setpoint".into(),
                value: 77.0,
            },
            Step::Expect {
                register: "Setpoint".into(),
                test: Test {
                    equals: Some(77.0),
                    ..Test::default()
                },
                within_ms: 0,
            },
        ]);
        let results = run(&mut s, &demo(), &seq, false, |_| {}).await;
        assert_eq!(results[0].status, Status::Error);
        assert_eq!(results[1].status, Status::Fail);
    }

    #[tokio::test]
    async fn the_report_lists_every_step() {
        let (_sim, mut s) = session().await;
        let seq = sequence(vec![Step::Read {
            register: "Frequency".into(),
        }]);
        let results = run(&mut s, &demo(), &seq, true, |_| {}).await;
        let ctx = ReportContext {
            map: "Demo".into(),
            map_file: "demo.mbmap.json".into(),
            map_revision: None,
            connection: "TCP".into(),
            unit_id: 1,
            identification: vec![],
            when: "now".into(),
        };
        let page = html(&seq, &results, &ctx);
        assert!(page.contains("PASSED"));
        assert!(page.contains("read Frequency"));
        assert!(page.contains("Nothing was written"));
    }
}
