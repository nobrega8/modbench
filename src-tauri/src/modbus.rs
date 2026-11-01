//! Modbus master session over TCP or RTU.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::timeout;
use tokio_modbus::client::{Client, Context, Reader, Writer, rtu, tcp};
use tokio_modbus::prelude::{ReadCode, SlaveContext};
use tokio_modbus::{ExceptionCode, Request, Response, Slave};
use tokio_serial::SerialPortBuilderExt;

use crate::ascii;
use crate::planner::Area;
use crate::tap::{Direction, RawSink, Tap};
use crate::udp::UdpStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Parity {
    N,
    E,
    O,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Transport {
    Tcp {
        host: String,
        port: u16,
    },
    /// RTU frames over a TCP socket, as used by many serial-to-Ethernet
    /// gateways ("Modbus over TCP").
    RtuOverTcp {
        host: String,
        port: u16,
    },
    /// Modbus TCP frames (MBAP) over UDP datagrams.
    Udp {
        host: String,
        port: u16,
    },
    /// RTU frames over UDP datagrams, as some serial gateways send them.
    RtuOverUdp {
        host: String,
        port: u16,
    },
    Rtu {
        port: String,
        baudrate: u32,
        parity: Parity,
        data_bits: u8,
        stop_bits: u8,
    },
    /// Modbus ASCII over a serial port: hex text frames with an LRC.
    Ascii {
        port: String,
        baudrate: u32,
        parity: Parity,
        data_bits: u8,
        stop_bits: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub transport: Transport,
    pub unit_id: u8,
    pub timeout_ms: u64,
    /// Local address to send from over TCP and UDP, to pick the network
    /// interface. None leaves it to the system's routing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_address: Option<String>,
    /// Modbus/TCP Security: TLS around Modbus TCP (see `tls`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<crate::tls::TlsConfig>,
}

/// Data returned by a read, matching the area that was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "values", rename_all = "lowercase")]
pub enum RawData {
    Bits(Vec<bool>),
    Registers(Vec<u16>),
}

#[derive(Debug, Error)]
pub enum ModbusError {
    #[error("not connected")]
    NotConnected,
    #[error("invalid address {0:?}")]
    BadAddress(String),
    #[error("timed out after {0} ms")]
    Timeout(u64),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serial port error: {0}")]
    Serial(#[from] tokio_serial::Error),
    #[error("protocol error: {0}")]
    Protocol(String),
    /// The device answered with an exception. `request` says what was
    /// asked, e.g. " to FC03 (read holding registers) for 10 from address
    /// 100, unit 1"; it is empty in the traffic log, whose rows show that.
    #[error("device exception 0x{code:02X} ({name}){request}")]
    Exception {
        code: u8,
        name: &'static str,
        request: String,
    },
    #[error("{0}")]
    Invalid(String),
    #[error("read-only mode is on: writes are blocked")]
    ReadOnly,
}

impl Serialize for ModbusError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

fn exception_name(code: ExceptionCode) -> &'static str {
    match code {
        ExceptionCode::IllegalFunction => "illegal function",
        ExceptionCode::IllegalDataAddress => "illegal data address",
        ExceptionCode::IllegalDataValue => "illegal data value",
        ExceptionCode::ServerDeviceFailure => "server device failure",
        ExceptionCode::Acknowledge => "acknowledge",
        ExceptionCode::ServerDeviceBusy => "server device busy",
        ExceptionCode::MemoryParityError => "memory parity error",
        ExceptionCode::GatewayPathUnavailable => "gateway path unavailable",
        ExceptionCode::GatewayTargetDevice => "gateway target device failed to respond",
        ExceptionCode::Custom(_) => "unknown exception",
    }
}

/// Name of a function code, as the traffic log and error messages give it.
pub fn function_name(function: u8) -> &'static str {
    match function {
        1 => "read coils",
        2 => "read discrete inputs",
        3 => "read holding registers",
        4 => "read input registers",
        5 => "write single coil",
        6 => "write single register",
        7 => "read exception status",
        8 => "diagnostics",
        11 => "get comm event counter",
        15 => "write multiple coils",
        16 => "write multiple registers",
        17 => "report server ID",
        20 => "read file record",
        22 => "mask write register",
        23 => "read/write multiple registers",
        24 => "read FIFO queue",
        43 => "read device identification",
        _ => "custom function",
    }
}

/// What a failed request asked for, appended to its exception message.
fn request_context(function: u8, address: u16, count: u16, unit: u8) -> String {
    let what = match function {
        1..=6 | 15 | 16 => format!(" for {count} from address {address}"),
        _ => String::new(),
    };
    format!(
        " to FC{function:02} ({}){what}, unit {unit}",
        function_name(function)
    )
}

impl From<ExceptionCode> for ModbusError {
    fn from(code: ExceptionCode) -> Self {
        ModbusError::Exception {
            code: code.into(),
            name: exception_name(code),
            request: String::new(),
        }
    }
}

impl From<tokio_modbus::Error> for ModbusError {
    fn from(err: tokio_modbus::Error) -> Self {
        match err {
            tokio_modbus::Error::Transport(e) => ModbusError::Io(e),
            tokio_modbus::Error::Protocol(e) => ModbusError::Protocol(e.to_string()),
        }
    }
}

fn resolve(host: &str, port: u16) -> Result<SocketAddr, ModbusError> {
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .ok_or_else(|| ModbusError::BadAddress(format!("{host}:{port}")))
}

/// Runs one Modbus call with a deadline and flattens its nested result.
async fn with_timeout<T>(
    wait: Duration,
    fut: impl std::future::Future<Output = tokio_modbus::Result<T>>,
) -> Result<T, ModbusError> {
    let ms = wait.as_millis() as u64;
    Ok(timeout(wait, fut)
        .await
        .map_err(|_| ModbusError::Timeout(ms))???)
}

pub(crate) fn open_serial(
    port: &str,
    baudrate: u32,
    parity: Parity,
    data_bits: u8,
    stop_bits: u8,
) -> Result<Box<dyn SerialLink>, ModbusError> {
    let real = real_port(port);
    if let Some(addr) = real.strip_prefix("tcp:") {
        let stream = std::net::TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        return Ok(Box::new(tokio::net::TcpStream::from_std(stream)?));
    }
    let builder = tokio_serial::new(real, baudrate)
        .parity(match parity {
            Parity::N => tokio_serial::Parity::None,
            Parity::E => tokio_serial::Parity::Even,
            Parity::O => tokio_serial::Parity::Odd,
        })
        .data_bits(match data_bits {
            7 => tokio_serial::DataBits::Seven,
            8 => tokio_serial::DataBits::Eight,
            n => {
                return Err(ModbusError::Invalid(format!("unsupported data bits: {n}")));
            }
        })
        .stop_bits(match stop_bits {
            1 => tokio_serial::StopBits::One,
            2 => tokio_serial::StopBits::Two,
            n => {
                return Err(ModbusError::Invalid(format!("unsupported stop bits: {n}")));
            }
        });
    Ok(Box::new(builder.open_native_async()?))
}

/// What a serial port reads and writes through: the port itself, or in debug
/// builds the socket a port alias names.
pub(crate) trait SerialLink:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + std::fmt::Debug
{
}

impl<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + std::fmt::Debug> SerialLink
    for T
{
}

/// Opens the transport a configuration describes.
/// A link that is not connected to anything: every request fails with an
/// I/O error. Holds the place of a link that was closed.
fn closed_context() -> Context {
    let (end, _) = tokio::io::duplex(1);
    rtu::attach(end)
}

/// The local address to send from, when one is chosen and the transport
/// is a network one.
fn local_ip(config: &ConnectionConfig) -> Result<Option<IpAddr>, ModbusError> {
    let network = !matches!(
        config.transport,
        Transport::Rtu { .. } | Transport::Ascii { .. }
    );
    match config.local_address.as_deref().map(str::trim) {
        Some(text) if network && !text.is_empty() => text.parse().map(Some).map_err(|_| {
            ModbusError::Invalid(format!("'{text}' is not an IP address of this computer"))
        }),
        _ => Ok(None),
    }
}

/// The address to bind a socket to before reaching `peer`: the chosen
/// local address (port 0), or any address of the peer's family.
fn local_for(peer: SocketAddr, local: Option<IpAddr>) -> Result<SocketAddr, ModbusError> {
    match local {
        Some(ip) if ip.is_ipv4() != peer.is_ipv4() => Err(ModbusError::Invalid(format!(
            "cannot reach {} from {ip}: one is IPv4 and the other IPv6",
            peer.ip()
        ))),
        Some(ip) => Ok(SocketAddr::new(ip, 0)),
        None if peer.is_ipv4() => Ok(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0)),
        None => Ok(SocketAddr::new(std::net::Ipv6Addr::UNSPECIFIED.into(), 0)),
    }
}

/// Explains an error that comes from the chosen local address: one that
/// is not on this computer, or that has no route to the peer.
fn bind_error(e: std::io::Error, local: Option<IpAddr>, peer: SocketAddr) -> ModbusError {
    use std::io::ErrorKind::*;
    match (local, e.kind()) {
        (Some(ip), AddrNotAvailable) => ModbusError::Invalid(format!(
            "cannot send from {ip}: it is not an address of this computer (was the interface turned off?)"
        )),
        (Some(ip), NetworkUnreachable | HostUnreachable) => ModbusError::Invalid(format!(
            "no route from {ip} to {peer}: the chosen interface cannot reach it ({e})"
        )),
        _ => ModbusError::Io(e),
    }
}

/// Opens a TCP connection within the timeout, from the chosen local
/// address when there is one.
async fn connect_tcp(
    peer: SocketAddr,
    local: Option<IpAddr>,
    timeout_ms: u64,
) -> Result<tokio::net::TcpStream, ModbusError> {
    let socket = match peer {
        SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4()?,
        SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6()?,
    };
    if local.is_some() {
        socket
            .bind(local_for(peer, local)?)
            .map_err(|e| bind_error(e, local, peer))?;
    }
    let wait = Duration::from_millis(timeout_ms.max(1));
    timeout(wait, socket.connect(peer))
        .await
        .map_err(|_| ModbusError::Timeout(timeout_ms))?
        .map_err(|e| bind_error(e, local, peer))
}

/// A network interface and one of its addresses, to send from.
#[derive(Debug, Clone, Serialize)]
pub struct LocalInterface {
    pub name: String,
    pub address: String,
    pub loopback: bool,
}

/// The addresses of this computer's network interfaces, IPv4 first.
pub fn local_interfaces() -> Result<Vec<LocalInterface>, ModbusError> {
    let mut list: Vec<LocalInterface> = if_addrs::get_if_addrs()?
        .into_iter()
        .filter(|i| !matches!(i.ip(), IpAddr::V6(v6) if v6.segments()[0] & 0xffc0 == 0xfe80))
        .map(|i| LocalInterface {
            loopback: i.is_loopback(),
            address: i.ip().to_string(),
            name: i.name,
        })
        .collect();
    list.sort_by_key(|i| (i.loopback, i.address.contains(':'), i.name.clone()));
    Ok(list)
}

async fn open(config: &ConnectionConfig, raw: Option<RawSink>) -> Result<Context, ModbusError> {
    let wait = Duration::from_millis(config.timeout_ms.max(1));
    let slave = Slave(config.unit_id);
    let local = local_ip(config)?;
    if config.tls.is_some() && !matches!(config.transport, Transport::Tcp { .. }) {
        return Err(ModbusError::Invalid(
            "TLS (Modbus/TCP Security) goes with Modbus TCP only".into(),
        ));
    }
    let ctx = match &config.transport {
        Transport::Tcp { host, port } => {
            let stream = connect_tcp(resolve(host, *port)?, local, config.timeout_ms).await?;
            match &config.tls {
                None => tcp::attach_slave(Tap::new(stream, raw), slave),
                Some(tls) => {
                    let connector = crate::tls::connector(tls)?;
                    let name = crate::tls::server_name(host)?;
                    let stream = timeout(wait, connector.connect(name, stream))
                        .await
                        .map_err(|_| ModbusError::Timeout(config.timeout_ms))?
                        .map_err(crate::tls::handshake_error)?;
                    tcp::attach_slave(Tap::new(stream, raw), slave)
                }
            }
        }
        Transport::RtuOverTcp { host, port } => {
            let stream = connect_tcp(resolve(host, *port)?, local, config.timeout_ms).await?;
            rtu::attach_slave(Tap::new(stream, raw), slave)
        }
        Transport::Udp { host, port } => {
            let addr = resolve(host, *port)?;
            let stream = UdpStream::connect(addr, local_for(addr, local)?, wait)
                .await
                .map_err(|e| bind_error(e, local, addr))?;
            tcp::attach_slave(Tap::new(stream, raw), slave)
        }
        Transport::RtuOverUdp { host, port } => {
            let addr = resolve(host, *port)?;
            let stream = UdpStream::connect(addr, local_for(addr, local)?, wait)
                .await
                .map_err(|e| bind_error(e, local, addr))?;
            rtu::attach_slave(Tap::new(stream, raw), slave)
        }
        Transport::Rtu {
            port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        } => {
            let stream = open_serial(port, *baudrate, *parity, *data_bits, *stop_bits)?;
            rtu::attach_slave(Tap::new(stream, raw), slave)
        }
        Transport::Ascii {
            port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        } => {
            let stream = open_serial(port, *baudrate, *parity, *data_bits, *stop_bits)?;
            ascii::attach_slave(Tap::new(stream, raw), slave)
        }
    };
    Ok(ctx)
}

/// One request as seen on the bus, for the traffic log.
#[derive(Debug, Clone, Serialize)]
pub struct TrafficEntry {
    /// Unix time in milliseconds when the request started.
    pub time_ms: u64,
    pub function: u8,
    pub address: u16,
    pub count: u16,
    pub duration_ms: f64,
    pub error: Option<String>,
    /// Set on lines that are not Modbus requests (raw bytes sent by hand).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

pub type TrafficLogger = Arc<dyn Fn(TrafficEntry) + Send + Sync>;

/// What to try when looking for a serial device's settings.
#[derive(Debug, Clone, Deserialize)]
pub struct SerialScan {
    pub port: String,
    /// Tried in this order, most likely first.
    pub baudrates: Vec<u32>,
    pub parities: Vec<Parity>,
    pub stop_bits: Vec<u8>,
    #[serde(default = "eight")]
    pub data_bits: u8,
    /// Unit IDs to ask, `from..=to`.
    pub from: u8,
    pub to: u8,
    /// The register to ask for (any answer counts, also an exception).
    pub area: Area,
    pub address: u16,
    pub timeout_ms: u64,
    /// Stop at the first setting that gets an answer.
    #[serde(default)]
    pub stop_at_first: bool,
}

fn eight() -> u8 {
    8
}

impl SerialScan {
    /// Every combination of line settings, in the order they are tried.
    pub fn combos(&self) -> Vec<(u32, Parity, u8)> {
        let mut out = Vec::new();
        for &baud in &self.baudrates {
            for &parity in &self.parities {
                for &stop in &self.stop_bits {
                    out.push((baud, parity, stop));
                }
            }
        }
        out
    }

    /// Requests to send in the worst case (nothing answers).
    pub fn total(&self) -> u32 {
        let units = u32::from(self.to.saturating_sub(self.from)) + 1;
        self.combos().len() as u32 * units
    }
}

/// A serial setting and unit ID that got an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SerialHit {
    pub baudrate: u32,
    pub parity: Parity,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub unit: u8,
    pub response: String,
}

/// Where a serial scan is, for a progress bar.
#[derive(Debug, Clone, Serialize)]
pub struct SerialProgress {
    pub baudrate: u32,
    pub parity: Parity,
    pub stop_bits: u8,
    pub unit: u8,
    /// Requests sent so far, and in the worst case.
    pub done: u32,
    pub total: u32,
}

/// Looks for the line settings of a serial device: opens the port with each
/// combination of baud rate, parity and stop bits and asks each unit ID for
/// one register. Only a frame with a valid CRC counts, so line noise at a
/// wrong baud rate is not taken for a device. Fails only when the port
/// cannot be opened.
pub async fn scan_serial(
    scan: &SerialScan,
    cancel: &AtomicBool,
    progress: impl Fn(&SerialProgress),
) -> Result<Vec<SerialHit>, ModbusError> {
    if scan.from > scan.to {
        return Err(ModbusError::Invalid(
            "the first unit ID is above the last".into(),
        ));
    }
    let total = scan.total();
    let mut done = 0;
    let mut hits = Vec::new();
    'combos: for (baudrate, parity, stop_bits) in scan.combos() {
        let config = ConnectionConfig {
            transport: Transport::Rtu {
                port: scan.port.clone(),
                baudrate,
                parity,
                data_bits: scan.data_bits,
                stop_bits,
            },
            unit_id: scan.from,
            timeout_ms: scan.timeout_ms,
            local_address: None,
            tls: None,
        };
        let mut session = Session::connect(&config, None).await?;
        let mut found = false;
        for unit in scan.from..=scan.to {
            if cancel.load(Ordering::Relaxed) {
                session.disconnect().await;
                break 'combos;
            }
            progress(&SerialProgress {
                baudrate,
                parity,
                stop_bits,
                unit,
                done,
                total,
            });
            done += 1;
            if let Some(response) = session.probe(unit, scan.area, scan.address).await {
                found = true;
                hits.push(SerialHit {
                    baudrate,
                    parity,
                    data_bits: scan.data_bits,
                    stop_bits,
                    unit,
                    response,
                });
            }
        }
        session.disconnect().await;
        if found && scan.stop_at_first {
            break;
        }
    }
    Ok(hits)
}

/// A unit ID that answered during a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanHit {
    pub unit: u8,
    /// What came back: a normal response, or the exception the device raised.
    pub response: String,
}

/// What an address scan looks at.
#[derive(Debug, Clone, Deserialize)]
pub struct AddressScan {
    pub area: Area,
    pub from: u16,
    pub to: u16,
    /// Addresses read at once before splitting.
    pub block: u16,
    /// How long to wait for each answer.
    pub timeout_ms: u64,
}

/// A run of addresses that answered during an address scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressRange {
    pub start: u16,
    pub count: u16,
}

/// Where an address scan is, for a progress bar.
#[derive(Debug, Clone, Serialize)]
pub struct AddressProgress {
    /// The first address of the piece being read.
    pub address: u16,
    /// Addresses settled so far, out of `total`, and how many answered.
    pub done: u32,
    pub total: u32,
    pub found: u32,
}

/// One identification object of a device (FC43).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceIdItem {
    pub id: u8,
    pub name: &'static str,
    pub value: String,
}

/// What a device says about itself (FC43 / MEI 14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceIdentity {
    /// Conformity level byte: which objects and access the device supports.
    pub conformity: u8,
    pub objects: Vec<DeviceIdItem>,
}

/// How a device answered a request with one function code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    /// It answered normally.
    Supported,
    /// It answered "illegal function" (exception 1).
    NotSupported,
    /// It knows the function but refused this request (another exception,
    /// e.g. an address it does not have).
    Refused,
    /// No answer, or one that could not be read.
    NoAnswer,
    /// Not tried: a write needs the value read first, and the read failed.
    Skipped,
}

/// The answer to one function code, from `Session::check_functions`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FunctionCheck {
    pub function: u8,
    pub name: &'static str,
    pub support: Support,
    /// The exception or error, when there was one.
    pub detail: Option<String>,
}

/// The function codes `check_functions` tries: all of them only read.
pub const READ_FUNCTIONS: [(u8, &str); 11] = [
    (1, "Read coils"),
    (2, "Read discrete inputs"),
    (3, "Read holding registers"),
    (4, "Read input registers"),
    (7, "Read exception status"),
    (8, "Diagnostics (return query data)"),
    (11, "Get comm event counter"),
    (17, "Report server ID"),
    (20, "Read file record"),
    (24, "Read FIFO queue"),
    (43, "Read device identification"),
];

/// The write function codes `check_functions` tries when allowed: each
/// writes back the value just read, or (FC22) a mask that keeps every bit.
pub const WRITE_FUNCTIONS: [(u8, &str); 6] = [
    (5, "Write single coil"),
    (6, "Write single register"),
    (15, "Write multiple coils"),
    (16, "Write multiple registers"),
    (22, "Mask write register"),
    (23, "Read/write multiple registers"),
];

/// A request with this function code that changes nothing on the device:
/// one item at `address`, the diagnostics echo, record 0 of file 1.
fn probe_request(function: u8, address: u16) -> Request<'static> {
    match function {
        1 => Request::ReadCoils(address, 1),
        2 => Request::ReadDiscreteInputs(address, 1),
        3 => Request::ReadHoldingRegisters(address, 1),
        4 => Request::ReadInputRegisters(address, 1),
        // Sub-function 0 echoes the data back
        8 => Request::Custom(8, vec![0x00, 0x00, 0xA5, 0x37].into()),
        17 => Request::ReportServerId,
        // One sub-request: reference type 6, file 1, record 0, 1 register
        20 => Request::Custom(20, vec![7, 6, 0, 1, 0, 0, 0, 1].into()),
        24 => Request::Custom(24, address.to_be_bytes().to_vec().into()),
        43 => Request::ReadDeviceIdentification(ReadCode::Basic, 0),
        other => Request::Custom(other, Vec::new().into()),
    }
}

/// tokio-modbus sends a custom request as `FunctionCode::Custom(7)` but
/// decodes the exception to it as `FunctionCode::ReadExceptionStatus`, and
/// then reports the two as different functions. The same number is the same
/// function: keep the device's exception.
async fn same_function(
    call: impl std::future::Future<Output = tokio_modbus::Result<Response>>,
) -> tokio_modbus::Result<Response> {
    match call.await {
        Err(tokio_modbus::Error::Protocol(tokio_modbus::ProtocolError::FunctionCodeMismatch {
            request,
            result: Err(answer),
        })) if request.value() == answer.function.value() => Ok(Err(answer.exception)),
        // A request built by hand (Custom) gets a typed answer for a known
        // function: the same function, so it is the answer.
        Err(tokio_modbus::Error::Protocol(tokio_modbus::ProtocolError::FunctionCodeMismatch {
            request,
            result: Ok(answer),
        })) if request.value() == answer.function_code().value() => Ok(Ok(answer)),
        other => other,
    }
}

/// Sends bytes exactly as given over a link opened for it (TCP, UDP or the
/// serial port of `config`), then collects what arrives until `wait_ms`
/// has passed, or 50 ms after the last byte. `seen` hears both directions.
pub async fn send_raw(
    config: &ConnectionConfig,
    bytes: &[u8],
    wait_ms: u64,
    seen: impl Fn(Direction, &[u8]),
) -> Result<Vec<u8>, ModbusError> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let wait = Duration::from_millis(wait_ms.max(1));
    let local = local_ip(config)?;
    let mut link: Box<dyn SerialLink> = match &config.transport {
        Transport::Tcp { host, port } | Transport::RtuOverTcp { host, port } => {
            Box::new(connect_tcp(resolve(host, *port)?, local, config.timeout_ms).await?)
        }
        Transport::Udp { host, port } | Transport::RtuOverUdp { host, port } => {
            let addr = resolve(host, *port)?;
            Box::new(
                UdpStream::connect(addr, local_for(addr, local)?, wait)
                    .await
                    .map_err(|e| bind_error(e, local, addr))?,
            )
        }
        Transport::Rtu {
            port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        }
        | Transport::Ascii {
            port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        } => open_serial(port, *baudrate, *parity, *data_bits, *stop_bits)?,
    };
    link.write_all(bytes).await?;
    link.flush().await?;
    seen(Direction::Out, bytes);
    let deadline = Instant::now() + wait;
    let mut back = Vec::new();
    let mut buf = [0u8; 512];
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        // After the first bytes, a 50 ms pause ends the answer
        let limit = if back.is_empty() {
            deadline - now
        } else {
            (deadline - now).min(Duration::from_millis(50))
        };
        match timeout(limit, link.read(&mut buf)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => {
                seen(Direction::In, &buf[..n]);
                back.extend_from_slice(&buf[..n]);
            }
            Ok(Err(e)) => return Err(e.into()),
        }
    }
    let _ = timeout(Duration::from_millis(100), link.shutdown()).await;
    Ok(back)
}

/// Whether a hand-built request can only read: the read functions, and the
/// diagnostics echo (FC08 sub-function 0). Anything else may change the
/// device and is refused in read-only mode.
pub fn pdu_only_reads(function: u8, data: &[u8]) -> bool {
    match function {
        1 | 2 | 3 | 4 | 7 | 11 | 12 | 17 | 20 | 24 | 43 => true,
        8 => data.len() >= 2 && data[0] == 0 && data[1] == 0,
        _ => false,
    }
}

/// Whether a decoded request can only read (see [`pdu_only_reads`]).
pub fn request_only_reads(request: &Request<'_>) -> bool {
    use Request::*;
    match request {
        ReadCoils(..)
        | ReadDiscreteInputs(..)
        | ReadInputRegisters(..)
        | ReadHoldingRegisters(..)
        | ReadDeviceIdentification(..)
        | ReportServerId => true,
        Custom(function, data) => pdu_only_reads(*function, data),
        _ => false,
    }
}

/// The largest read a device accepted, from `Session::largest_read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadLimit {
    pub largest: u16,
    /// Why one more failed; None when the protocol's maximum was read.
    pub limit: Option<String>,
}

/// The check for one function from its probe's result.
fn check(
    function: u8,
    name: &'static str,
    result: &Result<Response, ModbusError>,
) -> FunctionCheck {
    let (support, detail) = support_of(result);
    FunctionCheck {
        function,
        name,
        support,
        detail,
    }
}

/// What a probe's result says about the function.
fn support_of<T>(result: &Result<T, ModbusError>) -> (Support, Option<String>) {
    match result {
        Ok(_) => (Support::Supported, None),
        Err(ModbusError::Exception { code: 1, .. }) => (Support::NotSupported, None),
        Err(e @ ModbusError::Exception { .. }) => (Support::Refused, Some(e.to_string())),
        Err(e) => (Support::NoAnswer, Some(e.to_string())),
    }
}

/// Name of a standard identification object.
fn object_name(id: u8) -> &'static str {
    match id {
        0x00 => "Vendor name",
        0x01 => "Product code",
        0x02 => "Revision",
        0x03 => "Vendor URL",
        0x04 => "Product name",
        0x05 => "Model name",
        0x06 => "User application name",
        0x07..=0x7F => "Reserved",
        _ => "Private object",
    }
}

/// An object's value as text, or as hex when it is not printable text.
fn object_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes).map(|s| s.trim_end_matches('\0')) {
        Ok(s) if s.chars().all(|c| !c.is_control()) => s.to_string(),
        _ => bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub struct Session {
    ctx: Context,
    /// How the session was opened, to open it again after a lost link.
    config: ConnectionConfig,
    raw: Option<RawSink>,
    /// The last request failed with an I/O error (or, over TCP, a garbled
    /// frame): the link is gone until it is opened again.
    broken: bool,
    /// Timeouts since the last answer.
    timeouts_in_row: u32,
    unit: u8,
    timeout: Duration,
    logger: Option<TrafficLogger>,
    /// When set and true, every write is refused before it reaches the wire.
    read_only: Option<Arc<AtomicBool>>,
    /// When set and true, a single register is written with FC16 instead of
    /// FC06, for devices that only accept FC16.
    single_as_multiple: Option<Arc<AtomicBool>>,
    /// See [`Session::last_failure`].
    last_failure: Option<String>,
}

impl Session {
    /// Opens the connection. `raw`, when given, receives every byte sent
    /// and received.
    pub async fn connect(
        config: &ConnectionConfig,
        raw: Option<RawSink>,
    ) -> Result<Self, ModbusError> {
        let ctx = open(config, raw.clone()).await?;
        Ok(Self {
            ctx,
            config: config.clone(),
            raw,
            broken: false,
            timeouts_in_row: 0,
            unit: config.unit_id,
            timeout: Duration::from_millis(config.timeout_ms.max(1)),
            logger: None,
            read_only: None,
            single_as_multiple: None,
            last_failure: None,
        })
    }

    /// Whether the link looks lost: an I/O error, or over TCP a garbled
    /// frame or three timeouts in a row (a TCP link can die without an
    /// error). Opening it again with [`Session::reconnect`] may help; for a
    /// serial port, timeouts only mean the device is silent.
    pub fn link_lost(&self) -> bool {
        self.broken || (self.over_network() && self.timeouts_in_row >= 3)
    }

    fn over_network(&self) -> bool {
        !matches!(
            self.config.transport,
            Transport::Rtu { .. } | Transport::Ascii { .. }
        )
    }

    /// Closes the link and opens it again with the same settings, keeping
    /// the logger, the read-only switch and the unit ID.
    ///
    /// The old link is closed and dropped before the new one opens: a
    /// serial port is opened for exclusive use, so the old handle would
    /// make the port look busy. When opening fails, the session stays
    /// without a link (every request fails) and the port stays free.
    pub async fn reconnect(&mut self) -> Result<(), ModbusError> {
        self.release().await;
        let mut ctx = open(&self.config, self.raw.clone())
            .await
            .inspect_err(|e| {
                self.last_failure = Some(e.to_string());
            })?;
        ctx.set_slave(Slave(self.unit));
        self.ctx = ctx;
        self.broken = false;
        self.timeouts_in_row = 0;
        self.last_failure = None;
        Ok(())
    }

    /// Closes the link and drops it, so the port or socket is free, and
    /// leaves in its place one that fails every request.
    async fn release(&mut self) {
        // A dead link can hang on close; do not wait long for it.
        let _ = timeout(self.timeout, self.ctx.disconnect()).await;
        self.ctx = closed_context();
        self.broken = true;
    }

    /// Why the link was last seen failing (the error of a request or of an
    /// attempt to open it again), until it works again.
    pub fn last_failure(&self) -> Option<&str> {
        self.last_failure.as_deref()
    }

    /// Updates the link health after a request.
    fn track<T>(&mut self, result: &Result<T, ModbusError>) {
        match result {
            Ok(_) | Err(ModbusError::Exception { .. }) => {
                self.broken = false;
                self.timeouts_in_row = 0;
                self.last_failure = None;
            }
            Err(e @ ModbusError::Timeout(_)) => {
                self.timeouts_in_row += 1;
                self.last_failure = Some(e.to_string());
            }
            Err(e @ ModbusError::Io(_)) => {
                self.broken = true;
                self.last_failure = Some(e.to_string());
            }
            Err(e @ ModbusError::Protocol(_)) if self.over_network() => {
                self.broken = true;
                self.last_failure = Some(e.to_string());
            }
            Err(_) => {}
        }
    }

    pub fn set_logger(&mut self, logger: TrafficLogger) {
        self.logger = Some(logger);
    }

    /// Shares a read-only switch with the session: while it is on, writes
    /// fail with [`ModbusError::ReadOnly`] and nothing is sent.
    pub fn set_read_only(&mut self, switch: Arc<AtomicBool>) {
        self.read_only = Some(switch);
    }

    /// Shares the switch that makes single-register writes use FC16.
    pub fn set_single_as_multiple(&mut self, switch: Arc<AtomicBool>) {
        self.single_as_multiple = Some(switch);
    }

    fn check_writable(&self) -> Result<(), ModbusError> {
        match &self.read_only {
            Some(on) if on.load(Ordering::Relaxed) => Err(ModbusError::ReadOnly),
            _ => Ok(()),
        }
    }

    fn log<T>(
        &self,
        function: u8,
        address: u16,
        count: u16,
        started: (SystemTime, Instant),
        result: &mut Result<T, ModbusError>,
    ) {
        if let Some(logger) = &self.logger {
            logger(TrafficEntry {
                time_ms: started
                    .0
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64),
                function,
                address,
                count,
                duration_ms: started.1.elapsed().as_secs_f64() * 1000.0,
                error: result.as_ref().err().map(ToString::to_string),
                note: None,
            });
        }
        if let Err(ModbusError::Exception { request, .. }) = result {
            *request = request_context(function, address, count, self.unit);
        }
    }

    pub async fn read(
        &mut self,
        area: Area,
        address: u16,
        count: u16,
    ) -> Result<RawData, ModbusError> {
        if count == 0 {
            return Err(ModbusError::Invalid("count must be at least 1".into()));
        }
        let started = (SystemTime::now(), Instant::now());
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        let mut result = match area {
            Area::Coil => with_timeout(wait, ctx.read_coils(address, count))
                .await
                .map(RawData::Bits),
            Area::Discrete => with_timeout(wait, ctx.read_discrete_inputs(address, count))
                .await
                .map(RawData::Bits),
            Area::Input => with_timeout(wait, ctx.read_input_registers(address, count))
                .await
                .map(RawData::Registers),
            Area::Holding => with_timeout(wait, ctx.read_holding_registers(address, count))
                .await
                .map(RawData::Registers),
        };
        let function = match area {
            Area::Coil => 1,
            Area::Discrete => 2,
            Area::Holding => 3,
            Area::Input => 4,
        };
        self.log(function, address, count, started, &mut result);
        self.track(&result);
        let mut data = result?;
        // Coil responses are padded to a whole byte; keep only what was asked.
        if let RawData::Bits(bits) = &mut data {
            bits.truncate(count as usize);
        }
        Ok(data)
    }

    /// Reads `count` addresses in requests of at most `max` each, for devices
    /// that cannot answer a full-size read. The parts are joined in order;
    /// the first part that fails fails the whole read.
    pub async fn read_chunked(
        &mut self,
        area: Area,
        address: u16,
        count: u16,
        max: u16,
    ) -> Result<RawData, ModbusError> {
        let max = max.max(1);
        if count <= max {
            return self.read(area, address, count).await;
        }
        if u32::from(address) + u32::from(count) > 0x1_0000 {
            return Err(ModbusError::Invalid(format!(
                "{count} addresses from {address} go past 65535"
            )));
        }
        let mut data = match area.is_bit() {
            true => RawData::Bits(Vec::with_capacity(count.into())),
            false => RawData::Registers(Vec::with_capacity(count.into())),
        };
        let mut done = 0;
        while done < count {
            let n = max.min(count - done);
            match (&mut data, self.read(area, address + done, n).await?) {
                (RawData::Bits(all), RawData::Bits(part)) => all.extend(part),
                (RawData::Registers(all), RawData::Registers(part)) => all.extend(part),
                _ => unreachable!("one area returns one kind of data"),
            }
            done += n;
        }
        Ok(data)
    }

    /// Writes registers: FC06 for a single register, FC16 otherwise (or
    /// always FC16 when the session's switch says so).
    pub async fn write_registers(
        &mut self,
        address: u16,
        words: &[u16],
    ) -> Result<(), ModbusError> {
        self.check_writable()?;
        let started = (SystemTime::now(), Instant::now());
        let multiple = self
            .single_as_multiple
            .as_ref()
            .is_some_and(|on| on.load(Ordering::Relaxed));
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        let (function, mut result) = match words {
            [] => return Err(ModbusError::Invalid("nothing to write".into())),
            [word] if !multiple => (
                6,
                with_timeout(wait, ctx.write_single_register(address, *word)).await,
            ),
            _ => (
                16,
                with_timeout(wait, ctx.write_multiple_registers(address, words)).await,
            ),
        };
        self.log(function, address, words.len() as u16, started, &mut result);
        self.track(&result);
        result
    }

    /// Writes coils: FC05 for a single coil, FC15 otherwise.
    pub async fn write_coils(&mut self, address: u16, bits: &[bool]) -> Result<(), ModbusError> {
        self.check_writable()?;
        let started = (SystemTime::now(), Instant::now());
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        let (function, mut result) = match bits {
            [] => return Err(ModbusError::Invalid("nothing to write".into())),
            [bit] => (
                5,
                with_timeout(wait, ctx.write_single_coil(address, *bit)).await,
            ),
            _ => (
                15,
                with_timeout(wait, ctx.write_multiple_coils(address, bits)).await,
            ),
        };
        self.log(function, address, bits.len() as u16, started, &mut result);
        self.track(&result);
        result
    }

    /// Looks for devices by asking each unit ID in `from..=to` for one
    /// holding register. Any answer counts, including a Modbus exception:
    /// it still proves a device is there. Unit IDs that stay silent are
    /// skipped after `wait`. The session's own unit ID and timeout are
    /// restored afterwards. `progress` gets each unit ID before it is asked.
    pub async fn scan_units(
        &mut self,
        from: u8,
        to: u8,
        wait: Duration,
        cancel: &AtomicBool,
        progress: impl Fn(u8),
    ) -> Vec<ScanHit> {
        let timeout = self.timeout;
        self.timeout = wait;
        let mut hits = Vec::new();
        for id in from..=to {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            progress(id);
            if let Some(response) = self.probe(id, Area::Holding, 0).await {
                hits.push(ScanHit { unit: id, response });
            }
        }
        self.timeout = timeout;
        hits
    }

    /// Asks one unit ID for one register and says how it answered, or
    /// `None` when it did not (timeout, garbled frame, I/O error). The
    /// session's own unit ID is kept.
    pub async fn probe(&mut self, unit: u8, area: Area, address: u16) -> Option<String> {
        let own = self.unit;
        self.ctx.set_slave(Slave(unit));
        let answer = match self.read(area, address, 1).await {
            Ok(_) => Some("responds".into()),
            Err(ModbusError::Exception { code, name, .. }) => {
                Some(format!("responds with exception 0x{code:02X} ({name})"))
            }
            Err(_) => None,
        };
        self.ctx.set_slave(Slave(own));
        answer
    }

    /// Reads the device's identification objects (FC43 / MEI 14). `level`
    /// is the read code: 1 basic, 2 regular, 3 extended. Answers split over
    /// several responses are joined.
    pub async fn read_device_id(&mut self, level: u8) -> Result<DeviceIdentity, ModbusError> {
        let code = ReadCode::try_from_value(level)
            .filter(|c| *c != ReadCode::Specific)
            .ok_or_else(|| ModbusError::Invalid(format!("unknown identification level {level}")))?;
        let mut identity = DeviceIdentity {
            conformity: 0,
            objects: Vec::new(),
        };
        let mut next = 0u8;
        // A device that keeps saying "more follows" must not loop forever.
        for _ in 0..32 {
            let started = (SystemTime::now(), Instant::now());
            let mut result = with_timeout(
                self.timeout,
                self.ctx.call(Request::ReadDeviceIdentification(code, next)),
            )
            .await;
            self.log(43, next.into(), 1, started, &mut result);
            self.track(&result);
            let Response::ReadDeviceIdentification(part) = result? else {
                return Err(ModbusError::Protocol(
                    "the answer is not a device identification".into(),
                ));
            };
            identity.conformity = part.conformity_level.value();
            identity
                .objects
                .extend(part.device_id_objects.iter().map(|o| DeviceIdItem {
                    id: o.id,
                    name: object_name(o.id),
                    value: object_text(&o.value),
                }));
            if !part.more_follows || part.next_object_id <= next {
                break;
            }
            next = part.next_object_id;
        }
        Ok(identity)
    }

    /// Sends one harmless request per function code of `READ_FUNCTIONS`, at
    /// `address` where the function takes one, and says how the device
    /// answered each. With `writes`, also tries `WRITE_FUNCTIONS` by writing
    /// back what was just read there; refused in read-only mode.
    pub async fn check_functions(
        &mut self,
        address: u16,
        writes: bool,
    ) -> Result<Vec<FunctionCheck>, ModbusError> {
        if writes {
            self.check_writable()?;
        }
        let mut checks = Vec::new();
        for (function, name) in READ_FUNCTIONS {
            let result = self
                .send_probe(function, probe_request(function, address))
                .await;
            checks.push(check(function, name, &result));
        }
        if !writes {
            return Ok(checks);
        }

        // The values to write back
        let coil = match self.read(Area::Coil, address, 1).await {
            Ok(RawData::Bits(b)) => b.first().copied(),
            _ => None,
        };
        let register = match self.read(Area::Holding, address, 1).await {
            Ok(RawData::Registers(r)) => r.first().copied(),
            _ => None,
        };
        for (function, name) in WRITE_FUNCTIONS {
            let request = match (function, coil, register) {
                (5, Some(on), _) => Some(Request::WriteSingleCoil(address, on)),
                (15, Some(on), _) => Some(Request::WriteMultipleCoils(address, vec![on].into())),
                (6, _, Some(w)) => Some(Request::WriteSingleRegister(address, w)),
                (16, _, Some(w)) => Some(Request::WriteMultipleRegisters(address, vec![w].into())),
                // AND with all ones, OR with nothing: the register keeps its value
                (22, _, _) => Some(Request::MaskWriteRegister(address, 0xFFFF, 0x0000)),
                (23, _, Some(w)) => Some(Request::ReadWriteMultipleRegisters(
                    address,
                    1,
                    address,
                    vec![w].into(),
                )),
                _ => None,
            };
            let Some(request) = request else {
                let area = if matches!(function, 5 | 15) {
                    "coil"
                } else {
                    "holding register"
                };
                checks.push(FunctionCheck {
                    function,
                    name,
                    support: Support::Skipped,
                    detail: Some(format!(
                        "{area} {address} could not be read, so nothing was written"
                    )),
                });
                continue;
            };
            let result = self.send_probe(function, request).await;
            checks.push(check(function, name, &result));
        }
        Ok(checks)
    }

    /// Sends one probe request, logged and tracked like any other.
    /// Sends a request built by hand: a function code and the bytes after
    /// it. Functions that may write are refused in read-only mode (anything
    /// not known to only read). With `broadcast`, it goes to unit 0, where
    /// devices act but do not answer: no answer is then the expected result.
    pub async fn send_pdu(
        &mut self,
        function: u8,
        data: Vec<u8>,
        broadcast: bool,
    ) -> Result<(), ModbusError> {
        if !pdu_only_reads(function, &data) || broadcast {
            self.check_writable()?;
        }
        let own = self.unit;
        if broadcast {
            self.ctx.set_slave(Slave(0));
        }
        let result = self
            .send_probe(function, Request::Custom(function, data.into()))
            .await;
        if broadcast {
            self.ctx.set_slave(Slave(own));
            return match result {
                Ok(_) | Err(ModbusError::Timeout(_)) => Ok(()),
                Err(e) => Err(e),
            };
        }
        result.map(|_| ())
    }

    /// Sends a request as it came, to `unit`, and returns the answer: what a
    /// gateway does. Writes are refused in read-only mode. Unit 0 is a
    /// broadcast: devices act and none answers, so the answer is None.
    pub async fn forward(
        &mut self,
        unit: u8,
        request: Request<'static>,
    ) -> Result<Option<Response>, ModbusError> {
        if !request_only_reads(&request) {
            self.check_writable()?;
        }
        let own = self.unit;
        self.ctx.set_slave(Slave(unit));
        let function = request.function_code().value();
        let result = self.send_probe(function, request).await;
        self.ctx.set_slave(Slave(own));
        match result {
            Ok(response) => Ok(Some(response)),
            Err(ModbusError::Timeout(_)) if unit == 0 => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn send_probe(
        &mut self,
        function: u8,
        request: Request<'static>,
    ) -> Result<Response, ModbusError> {
        let started = (SystemTime::now(), Instant::now());
        let mut result = with_timeout(self.timeout, same_function(self.ctx.call(request))).await;
        self.log(function, 0, 1, started, &mut result);
        self.track(&result);
        result
    }

    /// The largest block of `area` the device returns in one read from
    /// `address`, found by halving: at most 125 registers or 2000 bits.
    /// Also says why the next size up failed. Fails when even one address
    /// cannot be read.
    pub async fn largest_read(
        &mut self,
        area: Area,
        address: u16,
    ) -> Result<ReadLimit, ModbusError> {
        let protocol = if matches!(area, Area::Coil | Area::Discrete) {
            2000
        } else {
            125
        };
        let max = protocol.min(65536 - u32::from(address)) as u16;
        let reason = |e: ModbusError| e.to_string();
        if self.read(area, address, max).await.is_ok() {
            return Ok(ReadLimit {
                largest: max,
                limit: None,
            });
        }
        self.read(area, address, 1).await?;
        // `ok` is known to read, `fail` known not to
        let (mut ok, mut fail) = (1u16, max);
        let mut why = None;
        while fail - ok > 1 {
            let mid = ok + (fail - ok) / 2;
            match self.read(area, address, mid).await {
                Ok(_) => ok = mid,
                Err(e) => {
                    fail = mid;
                    why = Some(reason(e));
                }
            }
        }
        if why.is_none() {
            why = self.read(area, address, fail).await.err().map(reason);
        }
        Ok(ReadLimit {
            largest: ok,
            limit: why,
        })
    }

    /// Finds which addresses of an area answer between `from` and `to`. Each
    /// block of `block` addresses is read at once; a block that fails is
    /// split in halves until the addresses that answer are found, so a
    /// sparse map costs few requests. Fails only when the link is lost.
    pub async fn scan_addresses(
        &mut self,
        scan: &AddressScan,
        cancel: &AtomicBool,
        progress: impl Fn(&AddressProgress),
    ) -> Result<Vec<AddressRange>, ModbusError> {
        if scan.from > scan.to {
            return Err(ModbusError::Invalid(
                "the first address is above the last".into(),
            ));
        }
        let limit = if scan.area.is_bit() { 2000 } else { 125 };
        let block = scan.block.clamp(1, limit);
        let timeout = self.timeout;
        self.timeout = Duration::from_millis(scan.timeout_ms.max(1));
        let total = u32::from(scan.to - scan.from) + 1;
        let mut done = 0u32;
        let mut found: Vec<AddressRange> = Vec::new();
        // Pieces still to try, the next one last.
        let mut pending: Vec<(u16, u16)> = Vec::new();
        let mut start = u32::from(scan.from);
        while start <= u32::from(scan.to) {
            let n = (u32::from(scan.to) - start + 1).min(block.into());
            pending.push((start as u16, n as u16));
            start += n;
        }
        pending.reverse();
        let result = loop {
            let Some((address, count)) = pending.pop() else {
                break Ok(());
            };
            if cancel.load(Ordering::Relaxed) {
                break Ok(());
            }
            progress(&AddressProgress {
                address,
                done,
                total,
                found: found.iter().map(|r| u32::from(r.count)).sum(),
            });
            match self.read(scan.area, address, count).await {
                Ok(_) => {
                    done += u32::from(count);
                    match found.last_mut() {
                        Some(r)
                            if u32::from(r.start) + u32::from(r.count) == u32::from(address) =>
                        {
                            r.count += count;
                        }
                        _ => found.push(AddressRange {
                            start: address,
                            count,
                        }),
                    }
                }
                Err(ModbusError::Io(e)) => break Err(ModbusError::Io(e)),
                Err(_) if count == 1 => done += 1,
                Err(_) => {
                    let half = count / 2;
                    pending.push((address + half, count - half));
                    pending.push((address, half));
                }
            }
        };
        self.timeout = timeout;
        result.map(|()| found)
    }

    /// Closes the link and frees the port or socket. Closing a dead link
    /// can fail or hang; the handle is dropped all the same, after at most
    /// the session's timeout.
    pub async fn disconnect(mut self) {
        self.release().await;
    }
}

/// Serial ports present on this machine.
pub fn list_serial_ports() -> Result<Vec<String>, ModbusError> {
    let names: Vec<String> = tokio_serial::available_ports()?
        .into_iter()
        .map(|p| p.port_name)
        .chain(port_aliases().into_iter().map(|(alias, _)| alias))
        .collect();
    Ok(without_dial_in(names))
}

/// Debug builds only: `MODBENCH_PORT_ALIASES="/dev/cu.usbserial-A=tcp:127.0.0.1:5031;…"`
/// lists made-up port names that open another port or a TCP socket (macOS
/// will not set a baud rate on a pty), so screenshots show real adapters.
fn port_aliases() -> Vec<(String, String)> {
    if !cfg!(debug_assertions) {
        return Vec::new();
    }
    std::env::var("MODBENCH_PORT_ALIASES")
        .unwrap_or_default()
        .split(';')
        .filter_map(|pair| pair.split_once('='))
        .map(|(alias, real)| (alias.trim().to_string(), real.trim().to_string()))
        .collect()
}

fn real_port(port: &str) -> String {
    port_aliases()
        .into_iter()
        .find(|(alias, _)| alias == port)
        .map_or_else(|| port.to_string(), |(_, real)| real)
}

/// macOS lists each port twice: `/dev/tty.*` waits for a modem's carrier
/// signal, `/dev/cu.*` does not and is the one to use. Keeps the `cu` one.
fn without_dial_in(names: Vec<String>) -> Vec<String> {
    names
        .iter()
        .filter(|n| match n.strip_prefix("/dev/tty.") {
            Some(rest) => !names.contains(&format!("/dev/cu.{rest}")),
            None => true,
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dial_in_ports_are_hidden_when_a_callout_port_exists() {
        let names = [
            "/dev/cu.usbserial-1",
            "/dev/tty.usbserial-1",
            "/dev/tty.other",
            "COM3",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            without_dial_in(names),
            ["/dev/cu.usbserial-1", "/dev/tty.other", "COM3"]
        );
    }

    #[test]
    fn identification_objects_read_as_text_or_hex() {
        assert_eq!(object_text(b"ACME\0\0"), "ACME");
        assert_eq!(object_text(&[0x01, 0xFF]), "01 FF");
        assert_eq!(object_name(0x01), "Product code");
        assert_eq!(object_name(0x90), "Private object");
    }

    #[test]
    fn serial_scan_tries_every_combination_in_order() {
        let scan = SerialScan {
            port: "COM1".into(),
            baudrates: vec![9600, 19200],
            parities: vec![Parity::E, Parity::N],
            stop_bits: vec![1, 2],
            data_bits: 8,
            from: 1,
            to: 3,
            area: Area::Holding,
            address: 0,
            timeout_ms: 100,
            stop_at_first: false,
        };
        let combos = scan.combos();
        assert_eq!(combos.len(), 8);
        assert_eq!(combos[0], (9600, Parity::E, 1));
        assert_eq!(combos[1], (9600, Parity::E, 2));
        assert_eq!(combos[7], (19200, Parity::N, 2));
        assert_eq!(scan.total(), 24);
    }
}
