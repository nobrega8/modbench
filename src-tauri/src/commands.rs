//! Tauri commands exposed to the UI.

use tauri::State;
use tokio::sync::Mutex;

use crate::codec::{self, ByteOrder, DataType, Value};
use crate::mbmap::{self, DeviceMap, Parsed};
use crate::modbus::{self, ConnectionConfig, ModbusError, RawData, Session};
use crate::planner::Area;

#[derive(Default)]
pub struct AppState {
    session: Mutex<Option<Session>>,
}

/// Where a typed value lives and how it is encoded.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TypedAddress {
    pub area: Area,
    pub address: u16,
    #[serde(rename = "type")]
    pub ty: DataType,
    #[serde(default)]
    pub byte_order: ByteOrder,
    /// Register count, only for strings.
    pub length: Option<u16>,
    /// Bit index, only for bool in input/holding registers.
    pub bit: Option<u8>,
}

impl TypedAddress {
    fn register_count(&self) -> Result<u16, ModbusError> {
        match self.ty.register_count() {
            Some(0) => Ok(1),
            Some(n) => Ok(n),
            None => self
                .length
                .filter(|&n| n > 0)
                .ok_or_else(|| ModbusError::Invalid("string needs a length".into())),
        }
    }
}

fn codec_err(e: codec::CodecError) -> ModbusError {
    ModbusError::Invalid(e.to_string())
}

#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<String>, ModbusError> {
    modbus::list_serial_ports()
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    config: ConnectionConfig,
) -> Result<(), ModbusError> {
    let mut slot = state.session.lock().await;
    if let Some(old) = slot.take() {
        let _ = old.disconnect().await;
    }
    *slot = Some(Session::connect(&config).await?);
    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), ModbusError> {
    match state.session.lock().await.take() {
        Some(session) => session.disconnect().await,
        None => Ok(()),
    }
}

#[tauri::command]
pub async fn read_raw(
    state: State<'_, AppState>,
    area: Area,
    address: u16,
    count: u16,
) -> Result<RawData, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session.read(area, address, count).await
}

#[tauri::command]
pub async fn write_registers(
    state: State<'_, AppState>,
    address: u16,
    words: Vec<u16>,
) -> Result<(), ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session.write_registers(address, &words).await
}

#[tauri::command]
pub async fn write_coils(
    state: State<'_, AppState>,
    address: u16,
    bits: Vec<bool>,
) -> Result<(), ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session.write_coils(address, &bits).await
}

/// Reads and checks a `.mbmap` file.
#[tauri::command]
pub async fn open_map(path: String) -> Result<Parsed, ModbusError> {
    let bytes = tokio::fs::read(&path).await?;
    Ok(mbmap::parse(&bytes))
}

/// Writes a map to a `.mbmap` file.
#[tauri::command]
pub async fn save_map(path: String, map: DeviceMap) -> Result<(), ModbusError> {
    tokio::fs::write(&path, mbmap::to_string(&map)).await?;
    Ok(())
}

/// Reads one typed value, without a map.
#[tauri::command]
pub async fn read_value(
    state: State<'_, AppState>,
    target: TypedAddress,
) -> Result<Value, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    read_typed(session, &target).await
}

/// Writes one typed value, without a map.
#[tauri::command]
pub async fn write_value(
    state: State<'_, AppState>,
    target: TypedAddress,
    value: Value,
) -> Result<(), ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    write_typed(session, &target, &value).await
}

pub async fn read_typed(session: &mut Session, t: &TypedAddress) -> Result<Value, ModbusError> {
    let count = t.register_count()?;
    match session.read(t.area, t.address, count).await? {
        RawData::Bits(bits) => Ok(Value::Bool(bits.first().copied().unwrap_or(false))),
        RawData::Registers(regs) => {
            codec::decode(&regs, t.ty, t.byte_order, t.bit).map_err(codec_err)
        }
    }
}

pub async fn write_typed(
    session: &mut Session,
    t: &TypedAddress,
    value: &Value,
) -> Result<(), ModbusError> {
    match (t.area, t.ty) {
        (Area::Discrete | Area::Input, _) => {
            Err(ModbusError::Invalid(format!("{:?} is read-only", t.area)))
        }
        (Area::Coil, _) => {
            let Value::Bool(bit) = value else {
                return Err(ModbusError::Invalid("coils take true/false".into()));
            };
            session.write_coils(t.address, &[*bit]).await
        }
        (Area::Holding, DataType::Bool) => {
            // Change one bit and keep the others: read, modify, write.
            let Value::Bool(on) = value else {
                return Err(ModbusError::Invalid("bits take true/false".into()));
            };
            let bit = t.bit.filter(|&b| b <= 15).ok_or_else(|| {
                ModbusError::Invalid("bool in a register needs a bit (0-15)".into())
            })?;
            let RawData::Registers(regs) = session.read(Area::Holding, t.address, 1).await? else {
                unreachable!("holding registers return words");
            };
            let mask = 1u16 << bit;
            let word = if *on { regs[0] | mask } else { regs[0] & !mask };
            session.write_registers(t.address, &[word]).await
        }
        (Area::Holding, ty) => {
            let words = codec::encode(value, ty, t.byte_order, t.length).map_err(codec_err)?;
            session.write_registers(t.address, &words).await
        }
    }
}
