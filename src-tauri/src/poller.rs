//! Reads every register of a map in a loop and reports decoded values.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::codec::{self, ByteOrder, DataType, Value};
use crate::mbmap::{Register, find_sentinel};
use crate::modbus::{ModbusError, RawData, Session};
use crate::planner::{self, Block, PlanLimits, Point};

#[derive(Debug, Clone, Deserialize)]
pub struct PollRequest {
    pub registers: Vec<Register>,
    /// The map's default byte order.
    pub byte_order: ByteOrder,
    /// Replaces every register's byte order, to try orders live.
    pub byte_order_override: Option<ByteOrder>,
    pub interval_ms: u64,
    /// Pause between two requests, also between cycles (slow RTU devices).
    pub delay_ms: u64,
    pub limits: PlanLimits,
    /// Map index of each register, reported back in `PointResult::index`.
    /// Empty means the position in `registers`.
    #[serde(default)]
    pub indexes: Vec<usize>,
    /// The map's sentinel tables, to flag reserved raw values.
    #[serde(default)]
    pub sentinels: Vec<(DataType, Vec<(i128, String)>)>,
    /// Scale factor registers that `registers` need but do not include:
    /// read to scale the others, never reported.
    #[serde(default)]
    pub scale_registers: Vec<Register>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PointResult {
    /// `PollRequest::indexes` entry of the register (or its position).
    pub index: usize,
    /// Engineering value (scale and offset applied).
    pub value: Option<Value>,
    /// Raw words or bits as read, for the hex column.
    pub raw: Option<RawData>,
    pub error: Option<String>,
    /// Sentinel label when the raw value is a reserved one (e.g.
    /// "Unimplemented"); `value` is then not a measurement.
    pub sentinel: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PollCycle {
    pub cycle: u64,
    pub duration_ms: f64,
    pub requests: usize,
    pub results: Vec<PointResult>,
}

fn decode_point(
    reg: &Register,
    order: ByteOrder,
    data: RawData,
    sentinels: &[(DataType, Vec<(i128, String)>)],
) -> PointResult {
    let raw_value = match (&data, reg.bit, reg.bits) {
        (RawData::Bits(bits), _, _) => Ok(Value::Bool(bits.first().copied().unwrap_or(false))),
        (RawData::Registers(words), Some(bit), Some(width)) => Ok(Value::Uint(
            codec::extract_field(words.first().copied().unwrap_or(0), bit, width).into(),
        )),
        (RawData::Registers(words), _, _) => codec::decode(words, reg.ty, order, reg.bit),
    };
    match raw_value {
        Ok(v) => {
            // An enum label for the same raw value wins over a sentinel.
            let labelled = match &v {
                Value::Int(n) => reg.enum_labels.iter().any(|(e, _)| e == n),
                Value::Uint(n) => reg
                    .enum_labels
                    .iter()
                    .any(|(e, _)| i128::from(*e) == i128::from(*n)),
                _ => false,
            };
            let sentinel = (!labelled)
                .then(|| find_sentinel(sentinels, reg.ty, &v))
                .flatten()
                .map(str::to_string);
            PointResult {
                index: 0,
                value: Some(codec::to_engineering(v, reg.scale, reg.offset)),
                raw: Some(data),
                error: None,
                sentinel,
            }
        }
        Err(e) => PointResult {
            index: 0,
            value: None,
            raw: Some(data),
            error: Some(e.to_string()),
            sentinel: None,
        },
    }
}

fn slice(data: &RawData, block: &Block, point: &Point) -> RawData {
    let range = block.range_of(point);
    match data {
        RawData::Bits(b) => RawData::Bits(b[range].to_vec()),
        RawData::Registers(w) => RawData::Registers(w[range].to_vec()),
    }
}

/// Runs one polling cycle over all readable registers.
pub async fn poll_once(session: &mut Session, req: &PollRequest) -> (Vec<PointResult>, usize) {
    // The registers to report, then the scale factors only read for them.
    let regs: Vec<&Register> = req.registers.iter().chain(&req.scale_registers).collect();
    let reported = req.registers.len();
    let readable: Vec<usize> = (0..regs.len())
        .filter(|&i| regs[i].access.readable())
        .collect();
    let points: Vec<Point> = readable
        .iter()
        .map(|&i| {
            let r = regs[i];
            Point {
                area: r.area,
                address: r.address,
                count: r.count(),
            }
        })
        .collect();
    let blocks = planner::plan(&points, req.limits);
    let delay = Duration::from_millis(req.delay_ms);
    let order_of = |reg: &Register| {
        req.byte_order_override
            .unwrap_or(reg.byte_order.unwrap_or(req.byte_order))
    };

    // (position in `regs`, result)
    let mut decoded: Vec<(usize, PointResult)> = Vec::with_capacity(points.len());
    let mut requests = 0;

    for block in &blocks {
        if requests > 0 && !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        requests += 1;
        let outcome = session.read(block.area, block.address, block.count).await;
        match outcome {
            Ok(data) => {
                for &pi in &block.points {
                    let reg = regs[readable[pi]];
                    let r = decode_point(
                        reg,
                        order_of(reg),
                        slice(&data, block, &points[pi]),
                        &req.sentinels,
                    );
                    decoded.push((readable[pi], r));
                }
            }
            // A merged block can fail because one address in it is not
            // mapped; retry each point alone so the others still read.
            Err(ModbusError::Exception { .. }) if block.points.len() > 1 => {
                for &pi in &block.points {
                    let p = points[pi];
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                    requests += 1;
                    let reg = regs[readable[pi]];
                    let r = match session.read(p.area, p.address, p.count.max(1)).await {
                        Ok(data) => decode_point(reg, order_of(reg), data, &req.sentinels),
                        Err(e) => failed(e.to_string()),
                    };
                    decoded.push((readable[pi], r));
                }
            }
            Err(e) => {
                let msg = e.to_string();
                for &pi in &block.points {
                    decoded.push((readable[pi], failed(msg.clone())));
                }
                // A dead link fails every block the same way; stop here.
                if matches!(e, ModbusError::Io(_)) {
                    break;
                }
            }
        }
    }

    apply_scale_factors(&regs, &mut decoded);
    let results = decoded
        .into_iter()
        .filter(|(pos, _)| *pos < reported)
        .map(|(pos, mut r)| {
            r.index = req.indexes.get(pos).copied().unwrap_or(pos);
            r
        })
        .collect();
    (results, requests)
}

fn failed(error: String) -> PointResult {
    PointResult {
        index: 0,
        value: None,
        raw: None,
        error: Some(error),
        sentinel: None,
    }
}

/// The exponent a scale factor register holds, if it was read and is usable.
fn exponent(r: &PointResult) -> Option<i32> {
    if r.error.is_some() || r.sentinel.is_some() {
        return None;
    }
    let n = match r.value.as_ref()? {
        Value::Int(n) => *n as f64,
        Value::Uint(n) => *n as f64,
        Value::Float(f) => f.round(),
        _ => return None,
    };
    // SunSpec exponents stay within -10..10; anything else is not a factor.
    (-20.0..=20.0).contains(&n).then_some(n as i32)
}

/// Multiplies each value that names a scale factor by 10^factor, read in the
/// same cycle. Without a usable factor the value is reported as an error.
fn apply_scale_factors(regs: &[&Register], decoded: &mut [(usize, PointResult)]) {
    let wanted: Vec<&str> = regs
        .iter()
        .filter_map(|r| r.scale_factor.as_deref())
        .collect();
    if wanted.is_empty() {
        return;
    }
    let mut factors: HashMap<&str, Option<i32>> = HashMap::new();
    for (pos, r) in decoded.iter() {
        let name = regs[*pos].name.as_str();
        if wanted.contains(&name) {
            factors.entry(name).or_insert_with(|| exponent(r));
        }
    }
    for (pos, r) in decoded.iter_mut() {
        let reg = regs[*pos];
        let Some(name) = reg.scale_factor.as_deref() else {
            continue;
        };
        if r.error.is_some() || r.sentinel.is_some() {
            continue;
        }
        match factors.get(name).copied().flatten() {
            Some(exp) => {
                if let Some(v) = r.value.take() {
                    r.value = Some(codec::apply_power_of_ten(v, reg.offset, exp));
                }
            }
            None => {
                r.value = None;
                r.error = Some(format!("scale factor {name:?} could not be read"));
            }
        }
    }
}

/// Polls until the task is aborted, calling `report` after every cycle.
/// Stops by itself when there is no session.
pub async fn run(
    session: Arc<Mutex<Option<Session>>>,
    req: PollRequest,
    report: impl Fn(Result<PollCycle, String>),
) {
    let interval = Duration::from_millis(req.interval_ms);
    let delay = Duration::from_millis(req.delay_ms);
    let mut cycle = 0;
    loop {
        let started = Instant::now();
        let outcome = {
            let mut slot = session.lock().await;
            match slot.as_mut() {
                Some(s) => Some(poll_once(s, &req).await),
                None => None,
            }
        };
        let Some((results, requests)) = outcome else {
            report(Err("not connected".into()));
            return;
        };
        cycle += 1;
        report(Ok(PollCycle {
            cycle,
            duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            requests,
            results,
        }));
        // The pause between requests also separates the last request of a
        // cycle from the first of the next; never spin without a pause.
        let pause = interval
            .saturating_sub(started.elapsed())
            .max(delay)
            .max(Duration::from_millis(10));
        tokio::time::sleep(pause).await;
    }
}
