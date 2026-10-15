//! A simulated Modbus TCP device. It keeps the four data areas in memory and
//! answers any client, so maps, tools and other programs can be tried
//! without hardware. Values can be seeded from a device map, and changed on
//! their own by generators (see [`crate::generators`]).

use std::future::{Ready, ready};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::net::TcpListener;
use tokio_modbus::prelude::{
    ConformityLevel, DeviceIdObject, ReadCode, ReadDeviceIdentificationResponse,
};
use tokio_modbus::server::tcp::{Server, accept_tcp_connection};
use tokio_modbus::{ExceptionCode, Request, Response, SlaveRequest};

use crate::codec::{self, DataType, Value};
use crate::generators;
use crate::mbmap::{self, DeviceMap, Register};
use crate::modbus::RawData;
use crate::planner::Area;

const SIZE: usize = 0x1_0000;

/// The four data areas of the simulated device.
pub struct Memory {
    coils: Vec<bool>,
    discrete: Vec<bool>,
    input: Vec<u16>,
    holding: Vec<u16>,
    /// When set, only these addresses exist (by area); the others answer
    /// "illegal data address", like a real device.
    defined: Option<[Vec<bool>; 4]>,
    /// Vendor, product code and revision for device identification (FC43).
    identity: [String; 3],
    /// The map the memory was shaped after, for generators that name its
    /// registers.
    map: Option<DeviceMap>,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            coils: vec![false; SIZE],
            discrete: vec![false; SIZE],
            input: vec![0; SIZE],
            holding: vec![0; SIZE],
            defined: None,
            identity: [
                "Modbench".into(),
                "Simulator".into(),
                env!("CARGO_PKG_VERSION").into(),
            ],
            map: None,
        }
    }
}

fn slot(area: Area) -> usize {
    match area {
        Area::Coil => 0,
        Area::Discrete => 1,
        Area::Input => 2,
        Area::Holding => 3,
    }
}

impl Memory {
    /// Fresh memory shaped after a map: only its addresses exist when
    /// `strict`, its reference values are filled in, and it identifies
    /// itself with the map's vendor, model and firmware.
    pub fn from_map(map: &DeviceMap, strict: bool) -> Self {
        let mut memory = Self::default();
        if strict {
            let mut defined = [
                vec![false; SIZE],
                vec![false; SIZE],
                vec![false; SIZE],
                vec![false; SIZE],
            ];
            for r in &map.registers {
                let start = usize::from(r.address);
                let end = (start + usize::from(r.count())).min(SIZE);
                defined[slot(r.area)][start..end].fill(true);
            }
            memory.defined = Some(defined);
        }
        for r in &map.registers {
            memory.seed(r, map);
        }
        memory.identity = [
            map.meta.vendor.clone(),
            map.meta.model.clone(),
            map.meta.firmware.clone().unwrap_or_else(|| "1.0".into()),
        ];
        memory.map = Some(map.clone());
        memory
    }

    /// The map the memory was shaped after, if any.
    pub fn map(&self) -> Option<&DeviceMap> {
        self.map.as_ref()
    }

    /// Only the map's addresses exist.
    pub fn strict(&self) -> bool {
        self.defined.is_some()
    }

    /// Puts a register's reference value (the raw value a healthy device
    /// holds) in memory. Registers without one stay at zero.
    fn seed(&mut self, r: &Register, map: &DeviceMap) {
        let Some(text) = r.reference.as_deref() else {
            return;
        };
        let address = usize::from(r.address);
        let on = matches!(text.to_ascii_lowercase().as_str(), "1" | "true");
        match (r.area, r.ty, r.bit, r.bits) {
            (Area::Coil, ..) => self.coils[address] = on,
            (Area::Discrete, ..) => self.discrete[address] = on,
            (area, DataType::Bool, Some(bit), _) => {
                let words = self.words_mut(area);
                words[address] = if on {
                    words[address] | 1 << bit
                } else {
                    words[address] & !(1 << bit)
                };
            }
            (area, _, Some(bit), Some(width)) => {
                if let Some(n) = mbmap::parse_i128(text) {
                    let words = self.words_mut(area);
                    words[address] = codec::insert_field(words[address], bit, width, n as u16);
                }
            }
            (area, ty, ..) => {
                let value = match ty {
                    DataType::String => Value::Text(text.to_string()),
                    DataType::Float32 | DataType::Float64 => match text.parse::<f64>() {
                        Ok(f) => Value::Float(f),
                        Err(_) => return,
                    },
                    _ => match mbmap::parse_i128(text) {
                        Some(n) if n < 0 => Value::Int(n as i64),
                        Some(n) => Value::Uint(n as u64),
                        None => return,
                    },
                };
                if let Ok(regs) = codec::encode(&value, ty, r.byte_order(&map.meta), r.length) {
                    let words = self.words_mut(area);
                    for (i, w) in regs.into_iter().enumerate() {
                        if let Some(slot) = words.get_mut(address + i) {
                            *slot = w;
                        }
                    }
                }
            }
        }
    }

    fn words_mut(&mut self, area: Area) -> &mut Vec<u16> {
        match area {
            Area::Input => &mut self.input,
            _ => &mut self.holding,
        }
    }

    fn bits_mut(&mut self, area: Area) -> &mut Vec<bool> {
        match area {
            Area::Discrete => &mut self.discrete,
            _ => &mut self.coils,
        }
    }

    /// Checks that a request's addresses exist.
    fn check(&self, area: Area, address: u16, count: usize) -> Result<(), ExceptionCode> {
        let start = usize::from(address);
        if count == 0 || start + count > SIZE {
            return Err(ExceptionCode::IllegalDataAddress);
        }
        match &self.defined {
            Some(defined) if !defined[slot(area)][start..start + count].iter().all(|d| *d) => {
                Err(ExceptionCode::IllegalDataAddress)
            }
            _ => Ok(()),
        }
    }

    /// Reads values as the UI shows them; every address can be read here.
    pub fn read(&self, area: Area, address: u16, count: u16) -> RawData {
        let start = usize::from(address);
        let end = (start + usize::from(count)).min(SIZE);
        match area {
            Area::Coil => RawData::Bits(self.coils[start..end].to_vec()),
            Area::Discrete => RawData::Bits(self.discrete[start..end].to_vec()),
            Area::Input => RawData::Registers(self.input[start..end].to_vec()),
            Area::Holding => RawData::Registers(self.holding[start..end].to_vec()),
        }
    }

    /// Changes values from the UI, in any area (also the read-only ones).
    pub fn set(&mut self, area: Area, address: u16, data: &RawData) -> Result<(), String> {
        let start = usize::from(address);
        match (area.is_bit(), data) {
            (true, RawData::Bits(bits)) if start + bits.len() <= SIZE => {
                self.bits_mut(area)[start..start + bits.len()].copy_from_slice(bits);
            }
            (false, RawData::Registers(words)) if start + words.len() <= SIZE => {
                self.words_mut(area)[start..start + words.len()].copy_from_slice(words);
            }
            (_, _) => return Err("the values do not fit this area".into()),
        }
        Ok(())
    }

    /// Whether an address exists, for the UI (every one unless strict).
    pub fn defined(&self, area: Area, address: u16) -> bool {
        self.defined
            .as_ref()
            .is_none_or(|d| d[slot(area)][usize::from(address)])
    }

    /// Answers one request as a device would.
    pub fn answer(&mut self, request: Request<'_>) -> Result<Response, ExceptionCode> {
        use Request::*;
        let bits = |n: u16| (1..=2000).contains(&n);
        let words = |n: u16| (1..=125).contains(&n);
        Ok(match request {
            ReadCoils(a, n) | ReadDiscreteInputs(a, n) => {
                let area = if matches!(request, ReadCoils(..)) {
                    Area::Coil
                } else {
                    Area::Discrete
                };
                if !bits(n) {
                    return Err(ExceptionCode::IllegalDataValue);
                }
                self.check(area, a, n.into())?;
                let RawData::Bits(values) = self.read(area, a, n) else {
                    unreachable!("bit areas hold bits");
                };
                match area {
                    Area::Coil => Response::ReadCoils(values),
                    _ => Response::ReadDiscreteInputs(values),
                }
            }
            ReadInputRegisters(a, n) | ReadHoldingRegisters(a, n) => {
                let area = if matches!(request, ReadInputRegisters(..)) {
                    Area::Input
                } else {
                    Area::Holding
                };
                if !words(n) {
                    return Err(ExceptionCode::IllegalDataValue);
                }
                self.check(area, a, n.into())?;
                let RawData::Registers(values) = self.read(area, a, n) else {
                    unreachable!("register areas hold words");
                };
                match area {
                    Area::Input => Response::ReadInputRegisters(values),
                    _ => Response::ReadHoldingRegisters(values),
                }
            }
            WriteSingleCoil(a, on) => {
                self.check(Area::Coil, a, 1)?;
                self.coils[usize::from(a)] = on;
                Response::WriteSingleCoil(a, on)
            }
            WriteMultipleCoils(a, values) => {
                if !bits(values.len() as u16) {
                    return Err(ExceptionCode::IllegalDataValue);
                }
                self.check(Area::Coil, a, values.len())?;
                let start = usize::from(a);
                self.coils[start..start + values.len()].copy_from_slice(&values);
                Response::WriteMultipleCoils(a, values.len() as u16)
            }
            WriteSingleRegister(a, w) => {
                self.check(Area::Holding, a, 1)?;
                self.holding[usize::from(a)] = w;
                Response::WriteSingleRegister(a, w)
            }
            WriteMultipleRegisters(a, values) => {
                if !(1..=123).contains(&values.len()) {
                    return Err(ExceptionCode::IllegalDataValue);
                }
                self.check(Area::Holding, a, values.len())?;
                let start = usize::from(a);
                self.holding[start..start + values.len()].copy_from_slice(&values);
                Response::WriteMultipleRegisters(a, values.len() as u16)
            }
            MaskWriteRegister(a, and, or) => {
                self.check(Area::Holding, a, 1)?;
                let w = &mut self.holding[usize::from(a)];
                *w = (*w & and) | (or & !and);
                Response::MaskWriteRegister(a, and, or)
            }
            ReadDeviceIdentification(code, _) => {
                if code == ReadCode::Specific {
                    return Err(ExceptionCode::IllegalDataValue);
                }
                Response::ReadDeviceIdentification(ReadDeviceIdentificationResponse {
                    read_code: code,
                    conformity_level: ConformityLevel::BasicIdentificationStreamOnly,
                    more_follows: false,
                    next_object_id: 0,
                    device_id_objects: self
                        .identity
                        .iter()
                        .enumerate()
                        .map(|(id, text)| DeviceIdObject {
                            id: id as u8,
                            value: text.clone().into_bytes().into(),
                        })
                        .collect(),
                })
            }
            _ => return Err(ExceptionCode::IllegalFunction),
        })
    }
}

/// One request a client sent to the simulator, for the activity log.
#[derive(Debug, Clone, Serialize)]
pub struct SimEvent {
    pub time_ms: u64,
    pub peer: String,
    pub unit: u8,
    pub function: u8,
    /// First address and quantity, when the request has them.
    pub address: Option<u16>,
    pub count: Option<u16>,
    /// The exception sent back, or None for a normal answer.
    pub error: Option<String>,
    /// The request changed values.
    pub wrote: bool,
}

pub type SimLogger = Arc<dyn Fn(SimEvent) + Send + Sync>;

fn where_of(request: &Request<'_>) -> (Option<u16>, Option<u16>) {
    use Request::*;
    match request {
        ReadCoils(a, n)
        | ReadDiscreteInputs(a, n)
        | ReadInputRegisters(a, n)
        | ReadHoldingRegisters(a, n) => (Some(*a), Some(*n)),
        WriteSingleCoil(a, _) | WriteSingleRegister(a, _) | MaskWriteRegister(a, ..) => {
            (Some(*a), Some(1))
        }
        WriteMultipleCoils(a, v) => (Some(*a), Some(v.len() as u16)),
        WriteMultipleRegisters(a, v) => (Some(*a), Some(v.len() as u16)),
        _ => (None, None),
    }
}

fn is_write(request: &Request<'_>) -> bool {
    use Request::*;
    matches!(
        request,
        WriteSingleCoil(..)
            | WriteMultipleCoils(..)
            | WriteSingleRegister(..)
            | WriteMultipleRegisters(..)
            | MaskWriteRegister(..)
    )
}

struct Service {
    memory: Arc<Mutex<Memory>>,
    /// Answer only this unit ID; None answers all of them.
    unit: Option<u8>,
    peer: SocketAddr,
    logger: Option<SimLogger>,
}

impl tokio_modbus::server::Service for Service {
    type Request = SlaveRequest<'static>;
    type Response = Option<Response>;
    type Exception = ExceptionCode;
    type Future = Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        // Another unit ID behind the same address: stay silent, like a
        // gateway with nothing there.
        if self.unit.is_some_and(|u| u != req.slave) {
            return ready(Ok(None));
        }
        let function = req.request.function_code().value();
        let (address, count) = where_of(&req.request);
        let wrote = is_write(&req.request);
        let answer = self.memory.lock().unwrap().answer(req.request);
        if let Some(logger) = &self.logger {
            logger(SimEvent {
                time_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64),
                peer: self.peer.to_string(),
                unit: req.slave,
                function,
                address,
                count,
                error: answer.as_ref().err().map(|e| e.to_string()),
                wrote: wrote && answer.is_ok(),
            });
        }
        ready(answer.map(Some))
    }
}

/// A running simulator: stop it by dropping or aborting the tasks.
pub struct Running {
    pub addr: SocketAddr,
    /// The only unit ID it answers, or None for all of them.
    pub unit: Option<u8>,
    task: tokio::task::JoinHandle<()>,
    generators: Option<tokio::task::JoinHandle<()>>,
}

impl Running {
    pub fn stop(self) {
        drop(self);
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
        if let Some(g) = &self.generators {
            g.abort();
        }
    }
}

/// Starts answering Modbus TCP clients on `addr` (port 0 picks a free
/// one), and runs the generators while it does. Returns where it listens.
pub async fn start(
    addr: SocketAddr,
    memory: Arc<Mutex<Memory>>,
    unit: Option<u8>,
    logger: Option<SimLogger>,
    generators: Option<generators::Shared>,
) -> std::io::Result<Running> {
    let listener = TcpListener::bind(addr).await?;
    let addr = listener.local_addr()?;
    let server = Server::new(listener);
    let generators = generators.map(|g| tokio::spawn(generators::drive(memory.clone(), g)));
    let task = tokio::spawn(async move {
        let on_connected = |stream, peer| {
            let accepted = accept_tcp_connection(stream, peer, |peer| {
                Ok(Some(Service {
                    memory: memory.clone(),
                    unit,
                    peer,
                    logger: logger.clone(),
                }))
            });
            async move { accepted }
        };
        // A client that sends garbage only loses its own connection.
        let _ = server.serve(&on_connected, |_| {}).await;
    });
    Ok(Running {
        addr,
        unit,
        task,
        generators,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modbus::{AddressScan, ConnectionConfig, ModbusError, Session, Transport};
    use std::sync::atomic::AtomicBool;

    fn map() -> DeviceMap {
        let text = "@mbmap=1\n@vendor=ACME\n@model=PM-1\n\
                    name,area,address,type,access,reference\n\
                    Model,holding,0,uint16,R,0x16A\n\
                    Power,holding,10,float32,R,\n\
                    Ratio,holding,12,int16,RW,-5\n\
                    Alarm,coil,3,bool,RW,true\n";
        mbmap::parse(text.as_bytes()).map.unwrap()
    }

    async fn session(addr: SocketAddr, unit: u8) -> Session {
        let config = ConnectionConfig {
            transport: Transport::Tcp {
                host: addr.ip().to_string(),
                port: addr.port(),
            },
            unit_id: unit,
            timeout_ms: 300,
        };
        Session::connect(&config, None).await.unwrap()
    }

    #[test]
    fn a_map_seeds_references_and_shapes_the_memory() {
        let mut m = Memory::from_map(&map(), true);
        assert_eq!(
            m.read(Area::Holding, 0, 1),
            RawData::Registers(vec![0x016A])
        );
        assert_eq!(
            m.read(Area::Holding, 12, 1),
            RawData::Registers(vec![(-5i16) as u16])
        );
        assert_eq!(m.read(Area::Coil, 3, 1), RawData::Bits(vec![true]));
        assert!(m.defined(Area::Holding, 11) && !m.defined(Area::Holding, 5));
        let gap = m.answer(Request::ReadHoldingRegisters(0, 11));
        assert_eq!(gap, Err(ExceptionCode::IllegalDataAddress));
        assert!(m.answer(Request::ReadHoldingRegisters(10, 3)).is_ok());
        let loose = Memory::from_map(&map(), false);
        assert!(loose.defined(Area::Holding, 5));
    }

    #[test]
    fn writes_change_the_memory() {
        let mut m = Memory::default();
        m.answer(Request::WriteSingleRegister(4, 77)).unwrap();
        m.answer(Request::MaskWriteRegister(4, 0x00F0, 0x0003))
            .unwrap();
        assert_eq!(
            m.read(Area::Holding, 4, 1),
            RawData::Registers(vec![(77 & 0xF0) | 3])
        );
        assert_eq!(
            m.answer(Request::ReadHoldingRegisters(0, 200)),
            Err(ExceptionCode::IllegalDataValue)
        );
        assert_eq!(
            m.answer(Request::ReportServerId),
            Err(ExceptionCode::IllegalFunction)
        );
    }

    #[tokio::test]
    async fn clients_read_write_and_identify_over_tcp() {
        let memory = Arc::new(Mutex::new(Memory::from_map(&map(), true)));
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let logger: SimLogger = Arc::new(move |e| seen.lock().unwrap().push(e));
        let sim = start(
            "127.0.0.1:0".parse().unwrap(),
            memory.clone(),
            Some(1),
            Some(logger),
            None,
        )
        .await
        .unwrap();
        let mut s = session(sim.addr, 1).await;
        assert_eq!(
            s.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![0x016A])
        );
        s.write_registers(12, &[9]).await.unwrap();
        assert_eq!(
            memory.lock().unwrap().read(Area::Holding, 12, 1),
            RawData::Registers(vec![9])
        );
        let id = s.read_device_id(1).await.unwrap();
        let texts: Vec<_> = id.objects.iter().map(|o| o.value.as_str()).collect();
        assert_eq!(texts, ["ACME", "PM-1", "1.0"]);
        assert!(matches!(
            s.read(Area::Holding, 5, 1).await,
            Err(ModbusError::Exception { code: 2, .. })
        ));
        {
            let log = events.lock().unwrap();
            assert_eq!(log.len(), 4);
            assert!(log[1].wrote && log[3].error.is_some());
        }

        // Another unit ID gets no answer.
        let mut other = session(sim.addr, 2).await;
        assert!(matches!(
            other.read(Area::Holding, 0, 1).await,
            Err(ModbusError::Timeout(_))
        ));
        sim.stop();
    }

    #[tokio::test]
    async fn an_address_scan_finds_the_registers_of_a_sparse_map() {
        let memory = Arc::new(Mutex::new(Memory::from_map(&map(), true)));
        let sim = start("127.0.0.1:0".parse().unwrap(), memory, None, None, None)
            .await
            .unwrap();
        let mut s = session(sim.addr, 1).await;
        let scan = AddressScan {
            area: Area::Holding,
            from: 0,
            to: 99,
            block: 50,
            timeout_ms: 300,
        };
        let cancel = AtomicBool::new(false);
        let found = s.scan_addresses(&scan, &cancel, |_| {}).await.unwrap();
        let ranges: Vec<_> = found.iter().map(|r| (r.start, r.count)).collect();
        assert_eq!(ranges, [(0, 1), (10, 3)]);
    }
}
