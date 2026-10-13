//! Runs a `Session` against an in-process Modbus TCP server.

use std::collections::HashMap;
use std::future;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use modbench_lib::codec::{self, ByteOrder, DataType, Value};
use modbench_lib::modbus::{ConnectionConfig, ModbusError, RawData, Session, Transport};
use modbench_lib::planner::Area;
use tokio::net::TcpListener;
use tokio_modbus::prelude::*;
use tokio_modbus::server::Service;
use tokio_modbus::server::tcp::{Server, accept_tcp_connection};

/// Device with 100 holding registers, 100 input registers and 100 coils.
/// Anything beyond address 99 answers with exception 02.
#[derive(Clone, Default)]
struct FakeDevice {
    holding: Arc<Mutex<HashMap<u16, u16>>>,
    input: Arc<Mutex<HashMap<u16, u16>>>,
    coils: Arc<Mutex<HashMap<u16, bool>>>,
}

const SIZE: u32 = 100;

fn in_range(addr: u16, count: usize) -> Result<(), ExceptionCode> {
    if addr as u32 + count as u32 > SIZE {
        Err(ExceptionCode::IllegalDataAddress)
    } else {
        Ok(())
    }
}

impl Service for FakeDevice {
    type Request = Request<'static>;
    type Response = Response;
    type Exception = ExceptionCode;
    type Future = future::Ready<Result<Response, ExceptionCode>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let read = |map: &Mutex<HashMap<u16, u16>>, a: u16, n: u16| {
            let m = map.lock().unwrap();
            (a..a + n)
                .map(|i| *m.get(&i).unwrap_or(&0))
                .collect::<Vec<_>>()
        };
        let res = match req {
            Request::ReadHoldingRegisters(a, n) => in_range(a, n as usize)
                .map(|_| Response::ReadHoldingRegisters(read(&self.holding, a, n))),
            Request::ReadInputRegisters(a, n) => in_range(a, n as usize)
                .map(|_| Response::ReadInputRegisters(read(&self.input, a, n))),
            Request::ReadCoils(a, n) => in_range(a, n as usize).map(|_| {
                let m = self.coils.lock().unwrap();
                Response::ReadCoils((a..a + n).map(|i| *m.get(&i).unwrap_or(&false)).collect())
            }),
            Request::WriteSingleRegister(a, w) => in_range(a, 1).map(|_| {
                self.holding.lock().unwrap().insert(a, w);
                Response::WriteSingleRegister(a, w)
            }),
            Request::WriteMultipleRegisters(a, words) => in_range(a, words.len()).map(|_| {
                let mut m = self.holding.lock().unwrap();
                for (i, w) in words.iter().enumerate() {
                    m.insert(a + i as u16, *w);
                }
                Response::WriteMultipleRegisters(a, words.len() as u16)
            }),
            Request::WriteSingleCoil(a, c) => in_range(a, 1).map(|_| {
                self.coils.lock().unwrap().insert(a, c);
                Response::WriteSingleCoil(a, c)
            }),
            Request::WriteMultipleCoils(a, coils) => in_range(a, coils.len()).map(|_| {
                let mut m = self.coils.lock().unwrap();
                for (i, c) in coils.iter().enumerate() {
                    m.insert(a + i as u16, *c);
                }
                Response::WriteMultipleCoils(a, coils.len() as u16)
            }),
            _ => Err(ExceptionCode::IllegalFunction),
        };
        future::ready(res)
    }
}

async fn start_device(device: FakeDevice) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = Server::new(listener);
    tokio::spawn(async move {
        let new_service = move |_| Ok(Some(device.clone()));
        let on_connected = move |stream, socket_addr| {
            let new_service = new_service.clone();
            async move { accept_tcp_connection(stream, socket_addr, new_service) }
        };
        let _ = server.serve(&on_connected, |_| {}).await;
    });
    addr
}

fn config(addr: SocketAddr) -> ConnectionConfig {
    ConnectionConfig {
        transport: Transport::Tcp {
            host: addr.ip().to_string(),
            port: addr.port(),
        },
        unit_id: 1,
        timeout_ms: 1000,
    }
}

#[tokio::test]
async fn reads_and_writes_registers() {
    let device = FakeDevice::default();
    device
        .input
        .lock()
        .unwrap()
        .extend([(10, 0x42F6), (11, 0xE979)]);
    let mut s = Session::connect(&config(start_device(device).await))
        .await
        .unwrap();

    let RawData::Registers(regs) = s.read(Area::Input, 10, 2).await.unwrap() else {
        panic!("expected registers");
    };
    let Value::Float(v) = codec::decode(&regs, DataType::Float32, ByteOrder::ABCD, None).unwrap()
    else {
        panic!("expected float");
    };
    assert!((v - 123.456).abs() < 1e-4);

    // FC06 then FC16
    s.write_registers(5, &[7]).await.unwrap();
    s.write_registers(20, &[1, 2, 3]).await.unwrap();
    assert_eq!(
        s.read(Area::Holding, 5, 1).await.unwrap(),
        RawData::Registers(vec![7])
    );
    assert_eq!(
        s.read(Area::Holding, 20, 3).await.unwrap(),
        RawData::Registers(vec![1, 2, 3])
    );
    s.disconnect().await.unwrap();
}

#[tokio::test]
async fn coils_are_trimmed_to_the_requested_count() {
    let mut s = Session::connect(&config(start_device(FakeDevice::default()).await))
        .await
        .unwrap();
    s.write_coils(0, &[true, false, true]).await.unwrap();
    s.write_coils(3, &[true]).await.unwrap();
    assert_eq!(
        s.read(Area::Coil, 0, 5).await.unwrap(),
        RawData::Bits(vec![true, false, true, true, false])
    );
}

#[tokio::test]
async fn device_exceptions_are_reported() {
    let mut s = Session::connect(&config(start_device(FakeDevice::default()).await))
        .await
        .unwrap();
    let err = s.read(Area::Holding, 99, 2).await.unwrap_err();
    assert!(
        matches!(err, ModbusError::Exception { code: 0x02, .. }),
        "{err}"
    );
    assert_eq!(
        err.to_string(),
        "device exception 0x02 (illegal data address)"
    );
    // The session is still usable after an exception.
    assert!(s.read(Area::Holding, 0, 1).await.is_ok());

    let err = s.read(Area::Discrete, 0, 1).await.unwrap_err();
    assert!(
        matches!(err, ModbusError::Exception { code: 0x01, .. }),
        "{err}"
    );
}

#[tokio::test]
async fn connection_refused_is_an_io_error() {
    // Bind and drop to get a port nothing listens on.
    let addr = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    let err = Session::connect(&config(addr))
        .await
        .err()
        .expect("should fail");
    assert!(
        matches!(err, ModbusError::Io(_) | ModbusError::Timeout(_)),
        "{err}"
    );
}
