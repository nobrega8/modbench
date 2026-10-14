//! Runs a `Session` against an in-process Modbus TCP server.

use std::collections::HashMap;
use std::future;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
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
    let mut s = Session::connect(&config(start_device(device).await), None)
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
    let mut s = Session::connect(&config(start_device(FakeDevice::default()).await), None)
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
async fn read_only_mode_blocks_writes_but_not_reads() {
    let device = FakeDevice::default();
    device.holding.lock().unwrap().insert(5, 7);
    let mut s = Session::connect(&config(start_device(device.clone()).await), None)
        .await
        .unwrap();
    let switch = Arc::new(AtomicBool::new(true));
    s.set_read_only(switch.clone());

    assert!(matches!(
        s.write_registers(5, &[9]).await,
        Err(ModbusError::ReadOnly)
    ));
    assert!(matches!(
        s.write_coils(0, &[true]).await,
        Err(ModbusError::ReadOnly)
    ));
    assert_eq!(
        s.read(Area::Holding, 5, 1).await.unwrap(),
        RawData::Registers(vec![7])
    );
    assert!(device.coils.lock().unwrap().is_empty());

    // Switching it off applies to the open session.
    switch.store(false, Ordering::Relaxed);
    s.write_registers(5, &[9]).await.unwrap();
    assert_eq!(device.holding.lock().unwrap()[&5], 9);
}

#[tokio::test]
async fn device_exceptions_are_reported() {
    let mut s = Session::connect(&config(start_device(FakeDevice::default()).await), None)
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
    let err = Session::connect(&config(addr), None)
        .await
        .err()
        .expect("should fail");
    assert!(
        matches!(err, ModbusError::Io(_) | ModbusError::Timeout(_)),
        "{err}"
    );
}

fn register(name: &str, area: Area, address: u16, ty: DataType) -> modbench_lib::mbmap::Register {
    modbench_lib::mbmap::Register {
        name: name.into(),
        group: String::new(),
        area,
        address,
        ty,
        length: None,
        scale: 1.0,
        scale_factor: None,
        offset: 0.0,
        unit: String::new(),
        access: modbench_lib::mbmap::Access::R,
        write_min: None,
        write_max: None,
        reference: None,
        byte_order: None,
        bit: None,
        bits: None,
        enum_labels: Vec::new(),
        enum_ref: None,
        description: String::new(),
        extra: Vec::new(),
    }
}

#[tokio::test]
async fn poller_decodes_scales_and_retries_failed_blocks() {
    use modbench_lib::planner::PlanLimits;
    use modbench_lib::poller::{PollRequest, poll_once};

    let device = FakeDevice::default();
    device
        .holding
        .lock()
        .unwrap()
        .extend([(0, 0xE979), (1, 0x42F6), (2, 2305), (98, 7)]);
    device.coils.lock().unwrap().insert(5, true);
    let mut s = Session::connect(&config(start_device(device).await), None)
        .await
        .unwrap();

    let mut temp = register("Temp", Area::Holding, 2, DataType::Int16);
    temp.scale = 0.1;
    let request = PollRequest {
        registers: vec![
            register("Power", Area::Holding, 0, DataType::Float32),
            temp,
            register("Ok", Area::Holding, 98, DataType::Uint16),
            // Past the end of the fake device: exception 02.
            register("Missing", Area::Holding, 100, DataType::Uint16),
            register("Relay", Area::Coil, 5, DataType::Bool),
        ],
        byte_order: ByteOrder::CDAB,
        byte_order_override: None,
        interval_ms: 1000,
        delay_ms: 0,
        limits: PlanLimits {
            max_gap: 1,
            ..Default::default()
        },
        indexes: Vec::new(),
        sentinels: Vec::new(),
        scale_registers: Vec::new(),
    };
    let (mut results, requests) = poll_once(&mut s, &request).await;
    results.sort_by_key(|r| r.index);

    let Some(Value::Float(power)) = results[0].value else {
        panic!("{:?}", results[0]);
    };
    assert!((power - 123.456).abs() < 1e-4);
    let Some(Value::Float(t)) = results[1].value else {
        panic!("{:?}", results[1]);
    };
    assert!((t - 230.5).abs() < 1e-9);
    assert_eq!(results[2].value, Some(Value::Uint(7)));
    assert!(results[3].error.as_deref().unwrap().contains("0x02"));
    assert_eq!(results[4].value, Some(Value::Bool(true)));
    // holding 0-2, holding 98-100 (fails), 98 and 100 alone, coil 5
    assert_eq!(requests, 5);

    // Results carry the caller's map indexes when given.
    let mut request = request;
    request.indexes = vec![10, 11, 12, 13, 14];
    let (results, _) = poll_once(&mut s, &request).await;
    let mut got: Vec<usize> = results.iter().map(|r| r.index).collect();
    got.sort();
    assert_eq!(got, [10, 11, 12, 13, 14]);
}

#[tokio::test]
async fn poller_flags_sentinels_unless_an_enum_names_the_value() {
    use modbench_lib::planner::PlanLimits;
    use modbench_lib::poller::{PollRequest, poll_once};

    let device = FakeDevice::default();
    device
        .holding
        .lock()
        .unwrap()
        .extend([(0, 0xFFFF), (1, 0xFFFF), (2, 230)]);
    let mut s = Session::connect(&config(start_device(device).await), None)
        .await
        .unwrap();

    let mut mode = register("Mode", Area::Holding, 1, DataType::Uint16);
    mode.enum_labels = vec![(65535, "Not fitted".into())];
    let request = PollRequest {
        registers: vec![
            register("Pressure", Area::Holding, 0, DataType::Uint16),
            mode,
            register("Voltage", Area::Holding, 2, DataType::Uint16),
        ],
        byte_order: ByteOrder::ABCD,
        byte_order_override: None,
        interval_ms: 0,
        delay_ms: 0,
        limits: PlanLimits::default(),
        indexes: Vec::new(),
        sentinels: vec![(DataType::Uint16, vec![(0xFFFF, "Unimplemented".into())])],
        scale_registers: Vec::new(),
    };
    let (mut results, _) = poll_once(&mut s, &request).await;
    results.sort_by_key(|r| r.index);
    assert_eq!(results[0].sentinel.as_deref(), Some("Unimplemented"));
    assert_eq!(results[1].sentinel, None);
    assert_eq!(results[2].sentinel, None);
    assert_eq!(results[2].value, Some(Value::Uint(230)));
}

#[tokio::test]
async fn poller_reports_reference_checks() {
    use modbench_lib::planner::PlanLimits;
    use modbench_lib::poller::{PollRequest, poll_once};

    let device = FakeDevice::default();
    device
        .holding
        .lock()
        .unwrap()
        .extend([(0, 0x016A), (1, 0x0001), (99, 5)]);
    let mut s = Session::connect(&config(start_device(device).await), None)
        .await
        .unwrap();

    let mut good = register("Code", Area::Holding, 0, DataType::Uint16);
    good.reference = Some("0x016A".into());
    let mut wrong = register("Other", Area::Holding, 1, DataType::Uint16);
    wrong.reference = Some("2".into());
    let none = register("Plain", Area::Holding, 99, DataType::Uint16);
    let mut unread = register("Gone", Area::Holding, 100, DataType::Uint16);
    unread.reference = Some("1".into());
    let request = PollRequest {
        registers: vec![good, wrong, none, unread],
        byte_order: ByteOrder::ABCD,
        byte_order_override: None,
        interval_ms: 0,
        delay_ms: 0,
        limits: PlanLimits::default(),
        indexes: Vec::new(),
        sentinels: Vec::new(),
        scale_registers: Vec::new(),
    };
    let (mut results, _) = poll_once(&mut s, &request).await;
    results.sort_by_key(|r| r.index);
    assert_eq!(results[0].reference, Some("match"));
    assert_eq!(results[1].reference, Some("mismatch"));
    assert_eq!(results[2].reference, None);
    // A failed read is reported as an error, not as a mismatch.
    assert!(results[3].error.is_some());
    assert_eq!(results[3].reference, None);
}

#[tokio::test]
async fn poller_applies_scale_factors_read_in_the_same_cycle() {
    use modbench_lib::planner::PlanLimits;
    use modbench_lib::poller::{PollRequest, poll_once};

    let device = FakeDevice::default();
    // W = 2345 with W_SF = -1; A = 12 with A_SF = 0x8000 (not implemented)
    device
        .holding
        .lock()
        .unwrap()
        .extend([(0, 2345), (1, 0xFFFF), (2, 12), (3, 0x8000)]);
    let mut s = Session::connect(&config(start_device(device).await), None)
        .await
        .unwrap();

    let mut watts = register("W", Area::Holding, 0, DataType::Int16);
    watts.scale_factor = Some("W_SF".into());
    let mut amps = register("A", Area::Holding, 2, DataType::Uint16);
    amps.scale_factor = Some("A_SF".into());
    let request = PollRequest {
        registers: vec![watts, amps],
        byte_order: ByteOrder::ABCD,
        byte_order_override: None,
        interval_ms: 0,
        delay_ms: 0,
        limits: PlanLimits::default(),
        indexes: Vec::new(),
        sentinels: vec![(DataType::Int16, vec![(-32768, "Not implemented".into())])],
        // The factors are read but not reported.
        scale_registers: vec![
            register("W_SF", Area::Holding, 1, DataType::Int16),
            register("A_SF", Area::Holding, 3, DataType::Int16),
        ],
    };
    let (mut results, _) = poll_once(&mut s, &request).await;
    results.sort_by_key(|r| r.index);
    assert_eq!(results.len(), 2);
    let Some(Value::Float(w)) = results[0].value else {
        panic!("{:?}", results[0]);
    };
    assert!((w - 234.5).abs() < 1e-9);
    assert_eq!(results[1].value, None);
    assert!(results[1].error.as_deref().unwrap().contains("A_SF"));
}
