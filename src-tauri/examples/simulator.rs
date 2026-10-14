//! Modbus TCP device simulator for development.
//!
//! Serves the registers of a `.mbmap` file with values that change over time.
//! Writable registers keep what is written to them; addresses that are not in
//! the map answer with exception 02, like a real device.
//!
//! ```sh
//! cargo run --example simulator -- ../vendor/mbmap-library/examples/example-corp/em-100.mbmap 5020
//! ```

use std::collections::{HashMap, HashSet};
use std::future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use modbench_lib::codec::{self, DataType, Value};
use modbench_lib::mbmap::{self, DeviceMap};
use modbench_lib::planner::Area;
use tokio::net::TcpListener;
use tokio_modbus::prelude::*;
use tokio_modbus::server::Service;
use tokio_modbus::server::tcp::{Server, accept_tcp_connection};

#[derive(Default)]
struct Memory {
    words: HashMap<(Area, u16), u16>,
    bits: HashMap<(Area, u16), bool>,
    mapped: HashSet<(Area, u16)>,
}

#[derive(Clone)]
struct Device {
    memory: Arc<Mutex<Memory>>,
}

impl Device {
    fn read_words(&self, area: Area, addr: u16, n: u16) -> Result<Vec<u16>, ExceptionCode> {
        let m = self.memory.lock().unwrap();
        (addr..addr.saturating_add(n))
            .map(|a| {
                if m.mapped.contains(&(area, a)) {
                    Ok(*m.words.get(&(area, a)).unwrap_or(&0))
                } else {
                    Err(ExceptionCode::IllegalDataAddress)
                }
            })
            .collect()
    }

    fn read_bits(&self, area: Area, addr: u16, n: u16) -> Result<Vec<bool>, ExceptionCode> {
        let m = self.memory.lock().unwrap();
        (addr..addr.saturating_add(n))
            .map(|a| {
                if m.mapped.contains(&(area, a)) {
                    Ok(*m.bits.get(&(area, a)).unwrap_or(&false))
                } else {
                    Err(ExceptionCode::IllegalDataAddress)
                }
            })
            .collect()
    }

    fn write_words(&self, addr: u16, words: &[u16]) -> Result<(), ExceptionCode> {
        let mut m = self.memory.lock().unwrap();
        for i in 0..words.len() as u16 {
            if !m.mapped.contains(&(Area::Holding, addr + i)) {
                return Err(ExceptionCode::IllegalDataAddress);
            }
        }
        for (i, w) in words.iter().enumerate() {
            m.words.insert((Area::Holding, addr + i as u16), *w);
        }
        Ok(())
    }

    fn write_bits(&self, addr: u16, bits: &[bool]) -> Result<(), ExceptionCode> {
        let mut m = self.memory.lock().unwrap();
        for i in 0..bits.len() as u16 {
            if !m.mapped.contains(&(Area::Coil, addr + i)) {
                return Err(ExceptionCode::IllegalDataAddress);
            }
        }
        for (i, b) in bits.iter().enumerate() {
            m.bits.insert((Area::Coil, addr + i as u16), *b);
        }
        Ok(())
    }
}

impl Service for Device {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = future::Ready<Result<Response, ExceptionCode>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        future::ready(match req {
            Request::ReadCoils(a, n) => self.read_bits(Area::Coil, a, n).map(Response::ReadCoils),
            Request::ReadDiscreteInputs(a, n) => self
                .read_bits(Area::Discrete, a, n)
                .map(Response::ReadDiscreteInputs),
            Request::ReadHoldingRegisters(a, n) => self
                .read_words(Area::Holding, a, n)
                .map(Response::ReadHoldingRegisters),
            Request::ReadInputRegisters(a, n) => self
                .read_words(Area::Input, a, n)
                .map(Response::ReadInputRegisters),
            Request::WriteSingleRegister(a, w) => self
                .write_words(a, &[w])
                .map(|_| Response::WriteSingleRegister(a, w)),
            Request::WriteMultipleRegisters(a, words) => self
                .write_words(a, &words)
                .map(|_| Response::WriteMultipleRegisters(a, words.len() as u16)),
            Request::WriteSingleCoil(a, b) => self
                .write_bits(a, &[b])
                .map(|_| Response::WriteSingleCoil(a, b)),
            Request::WriteMultipleCoils(a, bits) => self
                .write_bits(a, &bits)
                .map(|_| Response::WriteMultipleCoils(a, bits.len() as u16)),
            _ => Err(ExceptionCode::IllegalFunction),
        })
    }
}

/// A plausible engineering value for register `i` at time `t` seconds.
fn sample(ty: DataType, i: usize, t: f64) -> Value {
    let wave = (t / 6.0 + i as f64 * 0.9).sin();
    match ty {
        DataType::Bool => Value::Bool(((t / 4.0) as usize + i).is_multiple_of(3)),
        DataType::Float32 | DataType::Float64 => Value::Float(100.0 + 25.0 * wave),
        DataType::Int16 | DataType::Int32 | DataType::Int64 => Value::Int((200.0 * wave) as i64),
        DataType::Uint16 => Value::Uint((500.0 + 400.0 * wave) as u64),
        DataType::Uint32 | DataType::Uint64 => Value::Uint((t * 37.0) as u64 + i as u64 * 1000),
        DataType::String => Value::Text(format!("SIM-{i:04}")),
    }
}

/// The raw value a register's `reference` column names, for its type.
fn reference_value(r: &modbench_lib::mbmap::Register) -> Option<Value> {
    let text = r.reference.as_deref()?;
    let int = || -> Option<i128> {
        match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
            Some(hex) => i128::from_str_radix(hex, 16).ok(),
            None => text.parse().ok(),
        }
    };
    Some(match r.ty {
        DataType::Bool => Value::Bool(matches!(text, "1" | "true")),
        DataType::Float32 | DataType::Float64 => Value::Float(text.parse().ok()?),
        DataType::String => Value::Text(text.to_string()),
        DataType::Int16 | DataType::Int32 | DataType::Int64 => Value::Int(int()? as i64),
        _ => Value::Uint(int()? as u64),
    })
}

fn refresh(map: &DeviceMap, memory: &Mutex<Memory>, t: f64, first: bool) {
    let mut m = memory.lock().unwrap();
    for (i, r) in map.registers.iter().enumerate() {
        // Writable registers keep their written value after the first fill.
        if !first && r.access.writable() {
            continue;
        }
        let order = r.byte_order(&map.meta);
        // Scale factor registers hold a small fixed exponent (-2..0).
        let is_factor = map
            .registers
            .iter()
            .any(|x| x.scale_factor.as_deref() == Some(r.name.as_str()));
        let value = if is_factor {
            Value::Int(-((i % 3) as i64))
        } else if let Some(raw) = reference_value(r) {
            // A register with a reference value holds exactly that.
            codec::to_engineering(raw, r.scale, r.offset)
        } else {
            sample(r.ty, i, t)
        };
        match (r.area, r.ty) {
            (Area::Coil | Area::Discrete, _) => {
                if let Value::Bool(b) = value {
                    m.bits.insert((r.area, r.address), b);
                }
            }
            (_, DataType::Uint16) if r.bits.is_some() => {
                let (bit, width) = (r.bit.unwrap_or(0), r.bits.unwrap_or(16));
                let field = (i as u16 + (t as u16 / 5)) % (1u16 << width.min(15));
                let w = m.words.entry((r.area, r.address)).or_insert(0);
                *w = codec::insert_field(*w, bit, width, field);
            }
            (_, DataType::Bool) => {
                let (Value::Bool(b), Some(bit)) = (value, r.bit) else {
                    continue;
                };
                let w = m.words.entry((r.area, r.address)).or_insert(0);
                if b {
                    *w |= 1 << bit;
                } else {
                    *w &= !(1 << bit);
                }
            }
            _ => {
                let raw = codec::from_engineering(value, r.scale, r.offset);
                if let Ok(words) = codec::encode(&raw, r.ty, order, r.length) {
                    for (k, w) in words.into_iter().enumerate() {
                        m.words.insert((r.area, r.address + k as u16), w);
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("usage: simulator <map.mbmap> [port]");
    let port: u16 = args.next().map_or(5020, |p| p.parse().expect("port"));

    let parsed = mbmap::parse(&std::fs::read(&path).expect("read map"));
    let Some(map) = parsed.map else {
        for d in parsed.diagnostics {
            eprintln!("line {:?}: {} [{}]", d.line, d.message, d.code);
        }
        std::process::exit(1);
    };

    let memory = Arc::new(Mutex::new(Memory::default()));
    {
        let mut m = memory.lock().unwrap();
        for r in &map.registers {
            for k in 0..r.count() {
                m.mapped.insert((r.area, r.address + k));
            }
        }
    }
    refresh(&map, &memory, 0.0, true);

    let started = Instant::now();
    let (map_bg, memory_bg) = (map.clone(), memory.clone());
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(500)).await;
            refresh(&map_bg, &memory_bg, started.elapsed().as_secs_f64(), false);
        }
    });

    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let listener = TcpListener::bind(addr).await.expect("bind");
    println!(
        "Simulating {} {} ({} registers) on {addr}",
        map.meta.vendor,
        map.meta.model,
        map.registers.len()
    );
    let device = Device { memory };
    let new_service = move |_| Ok(Some(device.clone()));
    let on_connected = move |stream, socket_addr| {
        let new_service = new_service.clone();
        async move { accept_tcp_connection(stream, socket_addr, new_service) }
    };
    Server::new(listener)
        .serve(&on_connected, |e| eprintln!("connection error: {e}"))
        .await
        .expect("server");
}
