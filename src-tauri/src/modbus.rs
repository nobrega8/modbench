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

/// A unit ID that answered during a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScanHit {
    pub unit: u8,
    /// What came back: a normal response, or the exception the device raised.
    pub response: String,
}

pub struct Session {
    ctx: Context,
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
                            return Err(ModbusError::Invalid(format!(
                                "unsupported data bits: {n}"
                            )));
                        }
                    })
                    .stop_bits(match stop_bits {
                        1 => tokio_serial::StopBits::One,
                        2 => tokio_serial::StopBits::Two,
                        n => {
                            return Err(ModbusError::Invalid(format!(
                                "unsupported stop bits: {n}"
                            )));
                        }
                    });
                rtu::attach_slave(Tap::new(builder.open_native_async()?, raw), slave)
            }
        };
        Ok(Self {
            ctx,
            unit: config.unit_id,
            timeout: wait,
            logger: None,
            read_only: None,
        })
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
        let (unit, timeout) = (self.unit, self.timeout);
        self.timeout = wait;
        let mut hits = Vec::new();
        for id in from..=to {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            progress(id);
            self.ctx.set_slave(Slave(id));
            match self.read(Area::Holding, 0, 1).await {
                Ok(_) => hits.push(ScanHit {
                    unit: id,
                    response: "responds".into(),
                }),
                Err(ModbusError::Exception { name, .. }) => hits.push(ScanHit {
                    unit: id,
                    response: format!("responds with exception: {name}"),
                }),
                Err(_) => {}
            }
        }
        self.ctx.set_slave(Slave(unit));
        self.timeout = timeout;
        hits
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
