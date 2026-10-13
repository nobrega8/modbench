//! Reads every register of a map in a loop and reports decoded values.

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::codec::{self, ByteOrder, Value};
use crate::mbmap::Register;
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
}

#[derive(Debug, Clone, Serialize)]
pub struct PollCycle {
    pub cycle: u64,
    pub duration_ms: f64,
    pub requests: usize,
    pub results: Vec<PointResult>,
}

fn decode_point(reg: &Register, order: ByteOrder, data: RawData) -> PointResult {
    let value = match &data {
        RawData::Bits(bits) => Ok(Value::Bool(bits.first().copied().unwrap_or(false))),
        RawData::Registers(words) => codec::decode(words, reg.ty, order, reg.bit)
            .map(|v| codec::to_engineering(v, reg.scale, reg.offset)),
    };
    match value {
        Ok(v) => PointResult {
            index: 0,
            value: Some(v),
            raw: Some(data),
            error: None,
        },
        Err(e) => PointResult {
            index: 0,
            value: None,
            raw: Some(data),
            error: Some(e.to_string()),
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
    let readable: Vec<usize> = (0..req.registers.len())
        .filter(|&i| req.registers[i].access.readable())
        .collect();
    let points: Vec<Point> = readable
        .iter()
        .map(|&i| {
            let r = &req.registers[i];
            Point {
                area: r.area,
                address: r.address,
                count: r.count(),
            }
        })
        .collect();
    let blocks = planner::plan(&points, req.limits);
    let delay = Duration::from_millis(req.delay_ms);

    let mut results = Vec::with_capacity(points.len());
    let mut requests = 0;
    let mut finish = |pi: usize, mut r: PointResult| {
        r.index = req
            .indexes
            .get(readable[pi])
            .copied()
            .unwrap_or(readable[pi]);
        results.push(r);
    };

    for block in &blocks {
        if requests > 0 && !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        requests += 1;
        let outcome = session.read(block.area, block.address, block.count).await;
        match outcome {
            Ok(data) => {
                for &pi in &block.points {
                    let reg = &req.registers[readable[pi]];
                    let order = req
                        .byte_order_override
                        .unwrap_or(reg.byte_order.unwrap_or(req.byte_order));
                    finish(
                        pi,
                        decode_point(reg, order, slice(&data, block, &points[pi])),
                    );
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
                    let reg = &req.registers[readable[pi]];
                    let order = req
                        .byte_order_override
                        .unwrap_or(reg.byte_order.unwrap_or(req.byte_order));
                    let r = match session.read(p.area, p.address, p.count.max(1)).await {
                        Ok(data) => decode_point(reg, order, data),
                        Err(e) => PointResult {
                            index: 0,
                            value: None,
                            raw: None,
                            error: Some(e.to_string()),
                        },
                    };
                    finish(pi, r);
                }
            }
            Err(e) => {
                let msg = e.to_string();
                for &pi in &block.points {
                    finish(
                        pi,
                        PointResult {
                            index: 0,
                            value: None,
                            raw: None,
                            error: Some(msg.clone()),
                        },
                    );
                }
                // A dead link fails every block the same way; stop here.
                if matches!(e, ModbusError::Io(_)) {
                    break;
                }
            }
        }
    }
    (results, requests)
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
