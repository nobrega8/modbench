//! Modbus master session over TCP or RTU.

use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::timeout;
use tokio_modbus::client::{Client, Context, Reader, Writer, rtu, tcp};
use tokio_modbus::{ExceptionCode, Slave};
use tokio_serial::SerialPortBuilderExt;

use crate::planner::Area;

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

pub struct Session {
    ctx: Context,
    timeout: Duration,
}

impl Session {
    pub async fn connect(config: &ConnectionConfig) -> Result<Self, ModbusError> {
        let wait = Duration::from_millis(config.timeout_ms.max(1));
        let slave = Slave(config.unit_id);
        let ctx = match &config.transport {
            Transport::Tcp { host, port } => {
                let addr: SocketAddr = (host.as_str(), *port)
                    .to_socket_addrs()
                    .map_err(|_| ModbusError::BadAddress(format!("{host}:{port}")))?
                    .next()
                    .ok_or_else(|| ModbusError::BadAddress(format!("{host}:{port}")))?;
                timeout(wait, tcp::connect_slave(addr, slave))
                    .await
                    .map_err(|_| ModbusError::Timeout(config.timeout_ms))??
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
                rtu::attach_slave(builder.open_native_async()?, slave)
            }
        };
        Ok(Self { ctx, timeout: wait })
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
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        let mut data = match area {
            Area::Coil => RawData::Bits(with_timeout(wait, ctx.read_coils(address, count)).await?),
            Area::Discrete => {
                RawData::Bits(with_timeout(wait, ctx.read_discrete_inputs(address, count)).await?)
            }
            Area::Input => RawData::Registers(
                with_timeout(wait, ctx.read_input_registers(address, count)).await?,
            ),
            Area::Holding => RawData::Registers(
                with_timeout(wait, ctx.read_holding_registers(address, count)).await?,
            ),
        };
        // Coil responses are padded to a whole byte; keep only what was asked.
        if let RawData::Bits(bits) = &mut data {
            bits.truncate(count as usize);
        }
        Ok(data)
    }

    /// Writes registers: FC06 for a single register, FC16 otherwise.
    pub async fn write_registers(
        &mut self,
        address: u16,
        words: &[u16],
    ) -> Result<(), ModbusError> {
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        match words {
            [] => Err(ModbusError::Invalid("nothing to write".into())),
            [word] => with_timeout(wait, ctx.write_single_register(address, *word)).await,
            _ => with_timeout(wait, ctx.write_multiple_registers(address, words)).await,
        }
    }

    /// Writes coils: FC05 for a single coil, FC15 otherwise.
    pub async fn write_coils(&mut self, address: u16, bits: &[bool]) -> Result<(), ModbusError> {
        let (ctx, wait) = (&mut self.ctx, self.timeout);
        match bits {
            [] => Err(ModbusError::Invalid("nothing to write".into())),
            [bit] => with_timeout(wait, ctx.write_single_coil(address, *bit)).await,
            _ => with_timeout(wait, ctx.write_multiple_coils(address, bits)).await,
        }
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
