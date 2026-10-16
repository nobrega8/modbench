//! Simulated Modbus devices. Each keeps the four data areas in memory and
//! answers its unit ID, so maps, tools and other programs can be tried
//! without hardware: over Modbus TCP or UDP, RTU over TCP or UDP, or a
//! serial port (RTU or ASCII). Values can be seeded from a device map, and changed on their own
//! by generators (see [`crate::generators`]).

use std::collections::HashSet;
use std::future::{Ready, ready};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::net::{TcpListener, UdpSocket};
use tokio_modbus::prelude::{
    ConformityLevel, DeviceIdObject, ReadCode, ReadDeviceIdentificationResponse,
};
use tokio_modbus::server::tcp::{Server, accept_tcp_connection};
use tokio_modbus::{ExceptionCode, Request, Response, SlaveRequest};

use crate::codec::{self, DataType, Value};
use crate::generators;
use crate::mbmap::{self, DeviceMap, Register};
use crate::modbus::{self, ModbusError, RawData, Transport};
use crate::planner::Area;
use crate::serve::{self, Framing};

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

/// One simulated device: the unit ID it answers, its memory, and the values
/// that change on their own.
#[derive(Clone)]
pub struct Device {
    /// None answers every unit ID that no other device has.
    pub unit: Option<u8>,
    pub memory: Arc<Mutex<Memory>>,
    pub generators: generators::Shared,
    /// The map file the memory was shaped after, if any.
    pub source: Option<String>,
}

impl Device {
    pub fn new(unit: Option<u8>, memory: Memory) -> Self {
        Self {
            unit,
            memory: Arc::new(Mutex::new(memory)),
            generators: Default::default(),
            source: None,
        }
    }
}

/// The simulated devices, shared with a running simulator: they can be
/// added, changed and removed while it runs.
pub type Devices = Arc<Mutex<Vec<Device>>>;

/// Why devices with these unit IDs cannot answer side by side, or None.
pub fn unit_problem(units: impl IntoIterator<Item = Option<u8>>) -> Option<String> {
    let mut seen = HashSet::new();
    let mut every = false;
    for unit in units {
        match unit {
            None if every => return Some("only one device can answer every unit ID".into()),
            None => every = true,
            Some(u) if !seen.insert(u) => return Some(format!("two devices have unit ID {u}")),
            Some(_) => {}
        }
    }
    None
}

/// The memory that answers a unit ID: the device with that ID, else the one
/// that answers every ID. None when no device is there.
fn memory_for(devices: &Devices, unit: u8) -> Option<Arc<Mutex<Memory>>> {
    let list = devices.lock().unwrap();
    list.iter()
        .find(|d| d.unit == Some(unit))
        .or_else(|| list.iter().find(|d| d.unit.is_none()))
        .map(|d| d.memory.clone())
}

/// Answers the requests of one client (or serial line) and logs them.
#[derive(Clone)]
struct Responder {
    devices: Devices,
    peer: String,
    logger: Option<SimLogger>,
    /// Unit ID 0 is a broadcast, as on a serial line: every device takes a
    /// write and none answers.
    broadcast: bool,
}

impl Responder {
    fn log(&self, unit: u8, request: &Request<'_>, error: Option<String>, wrote: bool) {
        let Some(logger) = &self.logger else {
            return;
        };
        let (address, count) = where_of(request);
        logger(SimEvent {
            time_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64),
            peer: self.peer.clone(),
            unit,
            function: request.function_code().value(),
            address,
            count,
            error,
            wrote,
        });
    }
}

impl serve::Handler for Responder {
    fn handle(
        &self,
        unit: u8,
        request: Request<'static>,
    ) -> Option<Result<Response, ExceptionCode>> {
        let write = is_write(&request);
        if self.broadcast && unit == 0 {
            // Only writes can be broadcast; nobody answers either way.
            if write {
                let memories: Vec<_> = {
                    let list = self.devices.lock().unwrap();
                    list.iter().map(|d| d.memory.clone()).collect()
                };
                let results: Vec<_> = memories
                    .iter()
                    .map(|m| m.lock().unwrap().answer(request.clone()))
                    .collect();
                let wrote = results.iter().any(Result::is_ok);
                let error = match results.iter().find_map(|r| r.as_ref().err()) {
                    Some(e) if !wrote => Some(e.to_string()),
                    _ => None,
                };
                self.log(unit, &request, error, wrote);
            }
            return None;
        }
        // No device with this unit ID: stay silent, like a missing device.
        let memory = memory_for(&self.devices, unit)?;
        let answer = memory.lock().unwrap().answer(request.clone());
        self.log(
            unit,
            &request,
            answer.as_ref().err().map(|e| e.to_string()),
            write && answer.is_ok(),
        );
        Some(answer)
    }
}

/// tokio-modbus's TCP server, answering through a [`Responder`].
struct TcpService(Responder);

impl tokio_modbus::server::Service for TcpService {
    type Request = SlaveRequest<'static>;
    type Response = Option<Response>;
    type Exception = ExceptionCode;
    type Future = Ready<Result<Self::Response, Self::Exception>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        use serve::Handler;
        ready(match self.0.handle(req.slave, req.request) {
            None => Ok(None),
            Some(answer) => answer.map(Some),
        })
    }
}

/// A running simulator: stop it by dropping or aborting the tasks.
pub struct Running {
    /// Where it listens, with the port it picked when asked for port 0.
    pub listen: Transport,
    task: tokio::task::JoinHandle<()>,
    generators: tokio::task::JoinHandle<()>,
}

impl Running {
    pub fn stop(self) {
        drop(self);
    }

    /// The TCP or UDP port it listens on; None on a serial port.
    pub fn port(&self) -> Option<u16> {
        match &self.listen {
            Transport::Tcp { port, .. }
            | Transport::RtuOverTcp { port, .. }
            | Transport::Udp { port, .. }
            | Transport::RtuOverUdp { port, .. } => Some(*port),
            _ => None,
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
        self.generators.abort();
    }
}

/// Starts answering clients the way `listen` says: Modbus TCP or UDP, or
/// RTU over TCP or UDP, on a host and port (port 0 picks a free one), or RTU
/// or ASCII on a serial port. Runs the devices' generators while it does.
pub async fn start(
    listen: Transport,
    devices: Devices,
    logger: Option<SimLogger>,
) -> Result<Running, ModbusError> {
    let responder = {
        let (devices, logger) = (devices.clone(), logger.clone());
        move |peer: String, broadcast: bool| Responder {
            devices: devices.clone(),
            peer,
            logger: logger.clone(),
            broadcast,
        }
    };
    let (listen, task) = match listen {
        Transport::Tcp { host, port } => {
            let listener = TcpListener::bind((host.as_str(), port)).await?;
            let port = listener.local_addr()?.port();
            let server = Server::new(listener);
            let task = tokio::spawn(async move {
                let on_connected = |stream, peer: SocketAddr| {
                    let accepted = accept_tcp_connection(stream, peer, |peer| {
                        Ok(Some(TcpService(responder(peer.to_string(), false))))
                    });
                    async move { accepted }
                };
                // A client that sends garbage only loses its own connection.
                let _ = server.serve(&on_connected, |_| {}).await;
            });
            (Transport::Tcp { host, port }, task)
        }
        Transport::RtuOverTcp { host, port } => {
            let listener = TcpListener::bind((host.as_str(), port)).await?;
            let port = listener.local_addr()?.port();
            let task = tokio::spawn(async move {
                // Dropped with the task, which ends every connection.
                let mut clients = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        accepted = listener.accept() => match accepted {
                            Ok((stream, peer)) => {
                                // A gateway passes broadcasts on to its line.
                                let r = responder(peer.to_string(), true);
                                clients.spawn(serve::serve(stream, Framing::Rtu, r));
                            }
                            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                        },
                        Some(_) = clients.join_next() => {}
                    }
                }
            });
            (Transport::RtuOverTcp { host, port }, task)
        }
        Transport::Udp { ref host, port } | Transport::RtuOverUdp { ref host, port } => {
            let socket = UdpSocket::bind((host.as_str(), port)).await?;
            let port = socket.local_addr()?.port();
            let rtu = matches!(listen, Transport::RtuOverUdp { .. });
            let task = tokio::spawn(async move {
                // One request per datagram; the longest ADU is 260 bytes.
                let mut buf = [0u8; 512];
                loop {
                    // On Windows a client that went away shows up here as
                    // an error: keep serving the others.
                    let Ok((n, peer)) = socket.recv_from(&mut buf).await else {
                        continue;
                    };
                    // RTU over UDP comes from gateways, which pass
                    // broadcasts on to their line.
                    let r = responder(peer.to_string(), rtu);
                    let reply = if rtu {
                        serve::rtu_frame(&r, &buf[..n])
                    } else {
                        serve::mbap_frame(&r, &buf[..n])
                    };
                    if let Some(reply) = reply {
                        let _ = socket.send_to(&reply, peer).await;
                    }
                }
            });
            let host = host.clone();
            let listen = if rtu {
                Transport::RtuOverUdp { host, port }
            } else {
                Transport::Udp { host, port }
            };
            (listen, task)
        }
        Transport::Rtu {
            ref port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        }
        | Transport::Ascii {
            ref port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        } => {
            let stream = modbus::open_serial(port, baudrate, parity, data_bits, stop_bits)?;
            let framing = match listen {
                Transport::Ascii { .. } => Framing::Ascii,
                _ => Framing::Rtu,
            };
            let r = responder(port.clone(), true);
            let task = tokio::spawn(async move {
                let _ = serve::serve(stream, framing, r).await;
            });
            (listen, task)
        }
    };
    let generators = tokio::spawn(generators::drive(devices));
    Ok(Running {
        listen,
        task,
        generators,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modbus::{AddressScan, ConnectionConfig, Session};
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

    async fn session(port: u16, unit: u8) -> Session {
        let config = ConnectionConfig {
            transport: Transport::Tcp {
                host: "127.0.0.1".into(),
                port,
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
    async fn function_checks_tell_supported_refused_and_missing() {
        use crate::modbus::Support::*;
        for listen in [
            tcp(),
            Transport::RtuOverTcp {
                host: "127.0.0.1".into(),
                port: 0,
            },
        ] {
            let rtu = matches!(listen, Transport::RtuOverTcp { .. });
            let device = Device::new(Some(1), Memory::from_map(&map(), true));
            let sim = start(listen, devices(vec![device]), None).await.unwrap();
            let transport = if rtu {
                Transport::RtuOverTcp {
                    host: "127.0.0.1".into(),
                    port: sim.port().unwrap(),
                }
            } else {
                Transport::Tcp {
                    host: "127.0.0.1".into(),
                    port: sim.port().unwrap(),
                }
            };
            let config = ConnectionConfig {
                transport,
                unit_id: 1,
                timeout_ms: 300,
            };
            let mut s = Session::connect(&config, None).await.unwrap();
            let checks = s.check_functions().await;
            let got: Vec<_> = checks.iter().map(|c| (c.function, c.support)).collect();
            // Address 0 is mapped only in the holding registers
            assert_eq!(
                got,
                [
                    (1, Refused),
                    (2, Refused),
                    (3, Supported),
                    (4, Refused),
                    (7, NotSupported),
                    (8, NotSupported),
                    (11, NotSupported),
                    (17, NotSupported),
                    (20, NotSupported),
                    (24, NotSupported),
                    (43, Supported),
                ],
                "over {}",
                if rtu { "RTU over TCP" } else { "TCP" }
            );
            assert!(
                checks[0]
                    .detail
                    .as_deref()
                    .unwrap()
                    .contains("illegal data address")
            );
        }
    }

    fn tcp() -> Transport {
        Transport::Tcp {
            host: "127.0.0.1".into(),
            port: 0,
        }
    }

    fn devices(list: Vec<Device>) -> Devices {
        Arc::new(Mutex::new(list))
    }

    #[test]
    fn unit_ids_must_not_clash() {
        assert_eq!(unit_problem([Some(1), Some(2), None]), None);
        assert!(unit_problem([Some(1), Some(1)]).is_some());
        assert!(unit_problem([None, None]).is_some());
    }

    #[tokio::test]
    async fn clients_read_write_and_identify_over_tcp() {
        let device = Device::new(Some(1), Memory::from_map(&map(), true));
        let memory = device.memory.clone();
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let logger: SimLogger = Arc::new(move |e| seen.lock().unwrap().push(e));
        let sim = start(tcp(), devices(vec![device]), Some(logger))
            .await
            .unwrap();
        let port = sim.port().unwrap();
        let mut s = session(port, 1).await;
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
        let mut other = session(port, 2).await;
        assert!(matches!(
            other.read(Area::Holding, 0, 1).await,
            Err(ModbusError::Timeout(_))
        ));
        sim.stop();
    }

    #[tokio::test]
    async fn devices_answer_their_own_unit_ids() {
        let mut empty = Memory::default();
        empty
            .set(Area::Holding, 0, &RawData::Registers(vec![7]))
            .unwrap();
        let list = devices(vec![
            Device::new(Some(1), Memory::from_map(&map(), true)),
            Device::new(Some(5), empty),
        ]);
        let sim = start(tcp(), list.clone(), None).await.unwrap();
        let port = sim.port().unwrap();
        let mut one = session(port, 1).await;
        let mut five = session(port, 5).await;
        assert_eq!(
            one.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![0x016A])
        );
        assert_eq!(
            five.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![7])
        );
        // Unit 5 has no map: every address exists.
        assert!(five.read(Area::Holding, 5, 1).await.is_ok());

        // The unit ID scan finds exactly the simulated devices.
        let cancel = AtomicBool::new(false);
        let hits = one
            .scan_units(1, 8, Duration::from_millis(100), &cancel, |_| {})
            .await;
        let units: Vec<_> = hits.iter().map(|h| h.unit).collect();
        assert_eq!(units, [1, 5]);

        // A device added while running answers at once.
        list.lock()
            .unwrap()
            .push(Device::new(Some(8), Memory::default()));
        let mut eight = session(port, 8).await;
        assert!(eight.read(Area::Holding, 0, 1).await.is_ok());
    }

    #[tokio::test]
    async fn rtu_over_tcp_answers_like_a_gateway() {
        let list = devices(vec![
            Device::new(Some(1), Memory::from_map(&map(), true)),
            Device::new(Some(2), Memory::default()),
        ]);
        let listen = Transport::RtuOverTcp {
            host: "127.0.0.1".into(),
            port: 0,
        };
        let sim = start(listen, list.clone(), None).await.unwrap();
        let config = |unit| ConnectionConfig {
            transport: Transport::RtuOverTcp {
                host: "127.0.0.1".into(),
                port: sim.port().unwrap(),
            },
            unit_id: unit,
            timeout_ms: 300,
        };
        let mut s = Session::connect(&config(1), None).await.unwrap();
        assert_eq!(
            s.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![0x016A])
        );
        s.write_coils(3, &[false]).await.unwrap();
        assert_eq!(
            s.read(Area::Coil, 3, 1).await.unwrap(),
            RawData::Bits(vec![false])
        );
        assert!(matches!(
            s.read(Area::Holding, 5, 1).await,
            Err(ModbusError::Exception { code: 2, .. })
        ));
        let id = s.read_device_id(1).await.unwrap();
        assert_eq!(id.objects[0].value, "ACME");

        // A broadcast write reaches every device and gets no answer.
        let mut all = Session::connect(&config(0), None).await.unwrap();
        assert!(all.write_registers(12, &[4]).await.is_err());
        for device in list.lock().unwrap().iter() {
            let memory = device.memory.lock().unwrap();
            assert_eq!(
                memory.read(Area::Holding, 12, 1),
                RawData::Registers(vec![4])
            );
        }
    }

    #[tokio::test]
    async fn clients_read_and_write_over_udp() {
        for rtu in [false, true] {
            let list = devices(vec![
                Device::new(Some(1), Memory::from_map(&map(), true)),
                Device::new(Some(2), Memory::default()),
            ]);
            let host = "127.0.0.1".to_string();
            let listen = if rtu {
                Transport::RtuOverUdp { host, port: 0 }
            } else {
                Transport::Udp { host, port: 0 }
            };
            let sim = start(listen, list.clone(), None).await.unwrap();
            let config = |unit| {
                let (host, port) = ("127.0.0.1".to_string(), sim.port().unwrap());
                ConnectionConfig {
                    transport: if rtu {
                        Transport::RtuOverUdp { host, port }
                    } else {
                        Transport::Udp { host, port }
                    },
                    unit_id: unit,
                    timeout_ms: 300,
                }
            };
            let mut s = Session::connect(&config(1), None).await.unwrap();
            assert_eq!(
                s.read(Area::Holding, 0, 1).await.unwrap(),
                RawData::Registers(vec![0x016A])
            );
            s.write_registers(12, &[3]).await.unwrap();
            assert_eq!(
                s.read(Area::Holding, 12, 1).await.unwrap(),
                RawData::Registers(vec![3])
            );
            assert!(matches!(
                s.read(Area::Holding, 5, 1).await,
                Err(ModbusError::Exception { code: 2, .. })
            ));
            let mut two = Session::connect(&config(2), None).await.unwrap();
            assert!(two.read(Area::Holding, 5, 1).await.is_ok());
            let mut none = Session::connect(&config(9), None).await.unwrap();
            assert!(matches!(
                none.read(Area::Holding, 0, 1).await,
                Err(ModbusError::Timeout(_))
            ));
            // After a timeout the next answer is still the right one.
            assert_eq!(
                s.read(Area::Holding, 12, 1).await.unwrap(),
                RawData::Registers(vec![3])
            );
        }
    }

    #[tokio::test]
    async fn ascii_frames_are_served_over_a_stream() {
        use tokio_modbus::prelude::Reader;
        let (client, server) = tokio::io::duplex(1024);
        let r = Responder {
            devices: devices(vec![Device::new(Some(3), Memory::from_map(&map(), true))]),
            peer: "test".into(),
            logger: None,
            broadcast: true,
        };
        tokio::spawn(serve::serve(server, Framing::Ascii, r));
        let mut ctx = crate::ascii::attach_slave(client, tokio_modbus::Slave(3));
        let words = ctx.read_holding_registers(12, 1).await.unwrap().unwrap();
        assert_eq!(words, [(-5i16) as u16]);
    }

    #[tokio::test]
    async fn an_address_scan_finds_the_registers_of_a_sparse_map() {
        let list = devices(vec![Device::new(None, Memory::from_map(&map(), true))]);
        let sim = start(tcp(), list, None).await.unwrap();
        let mut s = session(sim.port().unwrap(), 1).await;
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
