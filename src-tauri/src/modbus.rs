//! Modbus master session over TCP or RTU.

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::timeout;
use tokio_modbus::client::{Client, Context, Reader, Writer, rtu, tcp};
use tokio_modbus::prelude::SlaveContext;
use tokio_modbus::{ExceptionCode, Slave};
use tokio_serial::SerialPortBuilderExt;

use crate::planner::Area;
use crate::tap::{RawSink, Tap};

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
    Rtu {
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
    #[error("device exception 0x{code:02X} ({name})")]
    Exception { code: u8, name: &'static str },
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

impl From<ExceptionCode> for ModbusError {
    fn from(code: ExceptionCode) -> Self {
        ModbusError::Exception {
            code: code.into(),
            name: exception_name(code),
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

/// Opens the transport a configuration describes.
async fn open(config: &ConnectionConfig, raw: Option<RawSink>) -> Result<Context, ModbusError> {
    let wait = Duration::from_millis(config.timeout_ms.max(1));
    let slave = Slave(config.unit_id);
    let ctx = match &config.transport {
        Transport::Tcp { host, port } => {
            let addr = resolve(host, *port)?;
            let stream = timeout(wait, tokio::net::TcpStream::connect(addr))
                .await
                .map_err(|_| ModbusError::Timeout(config.timeout_ms))??;
            tcp::attach_slave(Tap::new(stream, raw), slave)
        }
        Transport::RtuOverTcp { host, port } => {
            let addr = resolve(host, *port)?;
            let stream = timeout(wait, tokio::net::TcpStream::connect(addr))
                .await
                .map_err(|_| ModbusError::Timeout(config.timeout_ms))??;
            rtu::attach_slave(Tap::new(stream, raw), slave)
        }
        Transport::Rtu {
            port,
            baudrate,
            parity,
            data_bits,
            stop_bits,
        } => {
            let builder = tokio_serial::new(port, *baudrate)
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
            rtu::attach_slave(Tap::new(builder.open_native_async()?, raw), slave)
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
        };
        let mut session = Session::connect(&config, None).await?;
        let mut found = false;
        for unit in scan.from..=scan.to {
            if cancel.load(Ordering::Relaxed) {
                let _ = session.disconnect().await;
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
        let _ = session.disconnect().await;
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
        !matches!(self.config.transport, Transport::Rtu { .. })
    }

    /// Closes the link and opens it again with the same settings, keeping
    /// the logger, the read-only switch and the unit ID.
    pub async fn reconnect(&mut self) -> Result<(), ModbusError> {
        // A dead link can hang on close; do not wait long for it.
        let _ = timeout(self.timeout, self.ctx.disconnect()).await;
        let mut ctx = open(&self.config, self.raw.clone()).await?;
        ctx.set_slave(Slave(self.unit));
        self.ctx = ctx;
        self.broken = false;
        self.timeouts_in_row = 0;
        Ok(())
    }

    /// Updates the link health after a request.
    fn track<T>(&mut self, result: &Result<T, ModbusError>) {
        match result {
            Ok(_) | Err(ModbusError::Exception { .. }) => {
                self.broken = false;
                self.timeouts_in_row = 0;
            }
            Err(ModbusError::Timeout(_)) => self.timeouts_in_row += 1,
            Err(ModbusError::Io(_)) => self.broken = true,
            Err(ModbusError::Protocol(_)) if self.over_network() => self.broken = true,
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
        result: &Result<T, ModbusError>,
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
            });
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
        let result = match area {
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
        self.log(function, address, count, started, &result);
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

    /// Writes registers: FC06 for a single register, FC16 otherwise.
    pub async fn write_registers(
        &mut self,
        address: u16,
        words: &[u16],
    ) -> Result<(), ModbusError> {
        self.check_writable()?;
        let started = (SystemTime::now(), Instant::now());
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        let (function, result) = match words {
            [] => return Err(ModbusError::Invalid("nothing to write".into())),
            [word] => (
                6,
                with_timeout(wait, ctx.write_single_register(address, *word)).await,
            ),
            _ => (
                16,
                with_timeout(wait, ctx.write_multiple_registers(address, words)).await,
            ),
        };
        self.log(function, address, words.len() as u16, started, &result);
        self.track(&result);
        result
    }

    /// Writes coils: FC05 for a single coil, FC15 otherwise.
    pub async fn write_coils(&mut self, address: u16, bits: &[bool]) -> Result<(), ModbusError> {
        self.check_writable()?;
        let started = (SystemTime::now(), Instant::now());
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        let (function, result) = match bits {
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
        self.log(function, address, bits.len() as u16, started, &result);
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
            Err(ModbusError::Exception { name, .. }) => {
                Some(format!("responds with exception: {name}"))
            }
            Err(_) => None,
        };
        self.ctx.set_slave(Slave(own));
        answer
    }

    pub async fn disconnect(mut self) -> Result<(), ModbusError> {
        self.ctx.disconnect().await?;
        Ok(())
    }
}

/// Serial ports present on this machine.
pub fn list_serial_ports() -> Result<Vec<String>, ModbusError> {
    Ok(tokio_serial::available_ports()?
        .into_iter()
        .map(|p| p.port_name)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

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
