//! A Modbus gateway: answers clients on one link (Modbus TCP or UDP, RTU over
//! TCP or UDP, or a serial port) by passing each request on to a device on
//! another, and its answer back. Like a serial-to-Ethernet gateway on the
//! bench, with every exchange logged.
//!
//! Requests go out one at a time, as a serial line needs. The upstream link
//! is opened on the first request, and again whenever it is lost. A device
//! that does not answer is reported to the client as exception 0x0B (gateway
//! target device failed to respond), a link that cannot be opened or used as
//! 0x0A (gateway path unavailable). Unit ID 0 is passed on as a broadcast,
//! unanswered.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_modbus::{ExceptionCode, Request, Response};

use crate::modbus::{self, ConnectionConfig, ModbusError, Session, Transport};
use crate::serve;
use crate::simulator::where_of;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    /// Where clients connect.
    pub listen: Transport,
    /// The device (or line) requests are passed on to.
    pub upstream: ConnectionConfig,
    /// Sends every request to this unit ID; None keeps the client's.
    #[serde(default)]
    pub unit: Option<u8>,
    /// Refuses writes with exception 0x01 instead of passing them on.
    #[serde(default)]
    pub read_only: bool,
}

/// One request passed on, and how it went.
#[derive(Debug, Clone, Serialize)]
pub struct GatewayEvent {
    pub time_ms: u64,
    /// The client (address:port, or the serial port).
    pub peer: String,
    /// The unit ID the client asked for, and the one the request went to.
    pub unit: u8,
    pub target: u8,
    pub function: u8,
    pub address: Option<u16>,
    pub count: Option<u16>,
    /// The exception sent back, if any.
    pub exception: Option<u8>,
    /// Why, when the gateway made the exception up (no answer, no link,
    /// read-only).
    pub error: Option<String>,
    /// Time until the answer, in milliseconds.
    pub ms: u64,
}

pub type GatewayLogger = Arc<dyn Fn(GatewayEvent) + Send + Sync>;

/// Passes one client's requests on.
#[derive(Clone)]
struct Forwarder {
    config: Arc<ConnectionConfig>,
    /// None until the link is first opened.
    upstream: Arc<Mutex<Option<Session>>>,
    unit: Option<u8>,
    read_only: bool,
    peer: String,
    logger: Option<GatewayLogger>,
}

impl serve::Handler for Forwarder {
    async fn handle(
        &self,
        unit: u8,
        request: Request<'static>,
    ) -> Option<Result<Response, ExceptionCode>> {
        let started = Instant::now();
        let target = self.unit.unwrap_or(unit);
        let function = request.function_code().value();
        let (address, count) = where_of(&request);
        let (answer, error) = if self.read_only && !modbus::request_only_reads(&request) {
            (
                Some(Err(ExceptionCode::IllegalFunction)),
                Some(ModbusError::ReadOnly.to_string()),
            )
        } else {
            // One request at a time on the upstream link.
            let mut slot = self.upstream.lock().await;
            let session = match slot.as_mut() {
                Some(s) => {
                    if s.link_lost() {
                        let _ = s.reconnect().await;
                    }
                    Ok(s)
                }
                None => match Session::connect(&self.config, None).await {
                    Ok(s) => Ok(slot.insert(s)),
                    Err(e) => Err(e),
                },
            };
            let result = match session {
                Ok(s) => s.forward(target, request).await,
                Err(e) => Err(e),
            };
            match result {
                Ok(response) => (response.map(Ok), None),
                Err(ModbusError::Exception { code, .. }) => {
                    (Some(Err(ExceptionCode::new(code))), None)
                }
                Err(e @ ModbusError::Timeout(_)) => (
                    Some(Err(ExceptionCode::GatewayTargetDevice)),
                    Some(e.to_string()),
                ),
                Err(e @ ModbusError::ReadOnly) => (
                    Some(Err(ExceptionCode::IllegalFunction)),
                    Some(e.to_string()),
                ),
                Err(e) => (
                    Some(Err(ExceptionCode::GatewayPathUnavailable)),
                    Some(e.to_string()),
                ),
            }
        };
        if let Some(logger) = &self.logger {
            logger(GatewayEvent {
                time_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as u64),
                peer: self.peer.clone(),
                unit,
                target,
                function,
                address,
                count,
                exception: match &answer {
                    Some(Err(code)) => Some(u8::from(*code)),
                    _ => None,
                },
                error,
                ms: started.elapsed().as_millis() as u64,
            });
        }
        answer
    }
}

/// A running gateway: stop it by dropping it, which also closes the
/// upstream link.
pub struct Running {
    /// Where it listens, with the port it picked when asked for port 0.
    pub listen: Transport,
    task: JoinHandle<()>,
}

impl Running {
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
    }
}

/// Starts answering clients on `listen`; the upstream link opens with the
/// first request.
pub async fn start(
    options: Options,
    logger: Option<GatewayLogger>,
) -> Result<Running, ModbusError> {
    let config = Arc::new(options.upstream);
    let upstream = Arc::new(Mutex::new(None));
    let (unit, read_only) = (options.unit, options.read_only);
    let forwarder = move |peer: String, _broadcast: bool| Forwarder {
        config: config.clone(),
        upstream: upstream.clone(),
        unit,
        read_only,
        peer,
        logger: logger.clone(),
    };
    let (listen, task) = serve::listen(options.listen, forwarder).await?;
    Ok(Running { listen, task })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modbus::RawData;
    use crate::planner::Area;
    use crate::simulator::{self, Device, Memory};

    fn tcp(port: u16) -> Transport {
        Transport::Tcp {
            host: "127.0.0.1".into(),
            port,
        }
    }

    fn config(transport: Transport, unit_id: u8, timeout_ms: u64) -> ConnectionConfig {
        ConnectionConfig {
            transport,
            unit_id,
            timeout_ms,
            local_address: None,
        }
    }

    /// A simulated device with unit ID 1 and holding registers 0-2 = 7, 8, 9.
    async fn device() -> simulator::Running {
        let mut memory = Memory::default();
        memory
            .set(Area::Holding, 0, &RawData::Registers(vec![7, 8, 9]))
            .unwrap();
        let devices = Arc::new(std::sync::Mutex::new(vec![Device::new(Some(1), memory)]));
        simulator::start(tcp(0), devices, None).await.unwrap()
    }

    async fn gateway(
        listen: Transport,
        upstream_port: u16,
        read_only: bool,
        unit: Option<u8>,
    ) -> (Running, Arc<std::sync::Mutex<Vec<GatewayEvent>>>) {
        let events = Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen = events.clone();
        let logger: GatewayLogger = Arc::new(move |e| seen.lock().unwrap().push(e));
        let options = Options {
            listen,
            upstream: config(tcp(upstream_port), 1, 300),
            unit,
            read_only,
        };
        (start(options, Some(logger)).await.unwrap(), events)
    }

    #[tokio::test]
    async fn passes_reads_and_writes_on_and_logs_them() {
        let sim = device().await;
        let (gw, events) = gateway(tcp(0), sim.port().unwrap(), false, None).await;
        let mut client = Session::connect(&config(tcp(gw.port().unwrap()), 1, 1000), None)
            .await
            .unwrap();
        let read = client.read(Area::Holding, 0, 3).await.unwrap();
        assert_eq!(read, RawData::Registers(vec![7, 8, 9]));
        client.write_registers(1, &[42]).await.unwrap();
        let read = client.read(Area::Holding, 1, 1).await.unwrap();
        assert_eq!(read, RawData::Registers(vec![42]));
        let log = events.lock().unwrap();
        assert_eq!(log.len(), 3);
        assert_eq!(
            (log[0].function, log[0].address, log[0].count),
            (3, Some(0), Some(3))
        );
        assert!(log.iter().all(|e| e.exception.is_none()));
    }

    #[tokio::test]
    async fn a_silent_device_is_exception_0x0b_and_writes_can_be_refused() {
        let sim = device().await;
        let (gw, events) = gateway(tcp(0), sim.port().unwrap(), true, None).await;
        let mut client = Session::connect(&config(tcp(gw.port().unwrap()), 5, 2000), None)
            .await
            .unwrap();
        // Unit 5 is not on the line: the device is silent.
        let err = client.read(Area::Holding, 0, 1).await.unwrap_err();
        assert!(
            matches!(err, ModbusError::Exception { code: 0x0B, .. }),
            "{err}"
        );
        // Read-only: the write never reaches the device.
        let mut client = Session::connect(&config(tcp(gw.port().unwrap()), 1, 2000), None)
            .await
            .unwrap();
        let err = client.write_registers(0, &[1]).await.unwrap_err();
        assert!(
            matches!(err, ModbusError::Exception { code: 0x01, .. }),
            "{err}"
        );
        assert_eq!(
            client.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![7])
        );
        let log = events.lock().unwrap();
        assert_eq!(log[0].exception, Some(0x0B));
        assert!(log[1].error.as_deref().unwrap().contains("read-only"));
    }

    #[tokio::test]
    async fn rewrites_the_unit_id_and_frames_rtu_over_tcp() {
        let sim = device().await;
        let listen = Transport::RtuOverTcp {
            host: "127.0.0.1".into(),
            port: 0,
        };
        let (gw, events) = gateway(listen, sim.port().unwrap(), false, Some(1)).await;
        let port = gw.port().unwrap();
        let transport = Transport::RtuOverTcp {
            host: "127.0.0.1".into(),
            port,
        };
        let mut client = Session::connect(&config(transport, 9, 1000), None)
            .await
            .unwrap();
        let read = client.read(Area::Holding, 2, 1).await.unwrap();
        assert_eq!(read, RawData::Registers(vec![9]));
        let log = events.lock().unwrap();
        assert_eq!((log[0].unit, log[0].target), (9, 1));
    }

    #[tokio::test]
    async fn no_link_upstream_is_exception_0x0a_until_there_is_one() {
        // Nothing listens on this port yet.
        let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = free.local_addr().unwrap().port();
        drop(free);
        let (gw, _) = gateway(tcp(0), port, false, None).await;
        let mut client = Session::connect(&config(tcp(gw.port().unwrap()), 1, 2000), None)
            .await
            .unwrap();
        let err = client.read(Area::Holding, 0, 1).await.unwrap_err();
        assert!(
            matches!(err, ModbusError::Exception { code: 0x0A, .. }),
            "{err}"
        );
        // The device comes up on that port: the next request reaches it.
        let mut memory = Memory::default();
        memory
            .set(Area::Holding, 0, &RawData::Registers(vec![3]))
            .unwrap();
        let devices = Arc::new(std::sync::Mutex::new(vec![Device::new(Some(1), memory)]));
        let _sim = simulator::start(tcp(port), devices, None).await.unwrap();
        assert_eq!(
            client.read(Area::Holding, 0, 1).await.unwrap(),
            RawData::Registers(vec![3])
        );
    }
}
