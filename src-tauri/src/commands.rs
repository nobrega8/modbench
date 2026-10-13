//! Tauri commands exposed to the UI.

use std::sync::Arc;

use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::codec::{self, ByteOrder, DataType, Value};
use crate::library::{self, ImportResult, LibraryEntry};
use crate::mbmap::{self, DeviceMap, Parsed, Register};
use crate::modbus::{self, ConnectionConfig, ModbusError, RawData, Session};
use crate::online;
use crate::planner::Area;
use crate::poller::{self, PollCycle, PollRequest};
use crate::tap::{Direction, RawSink};

/// Bytes seen on the wire, sent to the UI as a `raw` event.
#[derive(Debug, Clone, serde::Serialize)]
struct RawChunk {
    time_ms: u64,
    direction: Direction,
    bytes: Vec<u8>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[derive(Default)]
pub struct AppState {
    session: Arc<Mutex<Option<Session>>>,
    poller: std::sync::Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    fn stop_poller(&self) {
        if let Some(handle) = self.poller.lock().unwrap().take() {
            handle.abort();
        }
    }
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
    /// Width of a uint16 bit field starting at `bit`.
    #[serde(default)]
    pub bits: Option<u8>,
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
    app: AppHandle,
    state: State<'_, AppState>,
    config: ConnectionConfig,
) -> Result<(), ModbusError> {
    state.stop_poller();
    let mut slot = state.session.lock().await;
    if let Some(old) = slot.take() {
        let _ = old.disconnect().await;
    }
    let raw_app = app.clone();
    let raw: RawSink = Arc::new(move |direction, bytes: &[u8]| {
        let _ = raw_app.emit(
            "raw",
            RawChunk {
                time_ms: now_ms(),
                direction,
                bytes: bytes.to_vec(),
            },
        );
    });
    let mut session = Session::connect(&config, Some(raw)).await?;
    session.set_logger(Arc::new(move |entry| {
        let _ = app.emit("traffic", entry);
    }));
    *slot = Some(session);
    Ok(())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), ModbusError> {
    state.stop_poller();
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

/// Starts polling a map's registers; each cycle is emitted as a `poll`
/// event. Replaces any polling already running.
#[tauri::command]
pub fn start_polling(app: AppHandle, state: State<'_, AppState>, request: PollRequest) {
    state.stop_poller();
    let session = state.session.clone();
    let handle = tauri::async_runtime::spawn(async move {
        poller::run(session, request, |cycle| match cycle {
            Ok(c) => {
                let _ = app.emit("poll", c);
            }
            Err(reason) => {
                let _ = app.emit("poll-stopped", reason);
            }
        })
        .await;
    });
    *state.poller.lock().unwrap() = Some(handle);
}

/// Reads every register of a map once.
#[tauri::command]
pub async fn poll_map_once(
    state: State<'_, AppState>,
    request: PollRequest,
) -> Result<PollCycle, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    let started = std::time::Instant::now();
    let (results, requests) = poller::poll_once(session, &request).await;
    Ok(PollCycle {
        cycle: 1,
        duration_ms: started.elapsed().as_secs_f64() * 1000.0,
        requests,
        results,
    })
}

#[tauri::command]
pub fn stop_polling(state: State<'_, AppState>) {
    state.stop_poller();
}

/// Writes an engineering value to a map register (scale and offset are
/// undone before encoding).
#[tauri::command]
pub async fn write_map_value(
    state: State<'_, AppState>,
    register: Register,
    byte_order: ByteOrder,
    value: Value,
) -> Result<(), ModbusError> {
    if !register.access.writable() {
        return Err(ModbusError::Invalid(format!(
            "{} is read-only",
            register.name
        )));
    }
    let target = TypedAddress {
        area: register.area,
        address: register.address,
        ty: register.ty,
        byte_order: register.byte_order.unwrap_or(byte_order),
        length: register.length,
        bit: register.bit,
        bits: register.bits,
    };
    let raw = codec::from_engineering(value, register.scale, register.offset);
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    write_typed(session, &target, &raw).await
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

fn library_root(app: &AppHandle) -> Result<std::path::PathBuf, ModbusError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ModbusError::Invalid(format!("no data folder: {e}")))?;
    Ok(dir.join("library"))
}

/// Folder where the library keeps its maps.
#[tauri::command]
pub fn library_folder(app: AppHandle) -> Result<String, ModbusError> {
    Ok(library_root(&app)?.display().to_string())
}

/// Opens a folder or a URL with the system's default app.
fn open_with_system(target: &std::ffi::OsStr) -> std::io::Result<()> {
    #[cfg(windows)]
    let mut command = {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");
    command.arg(target).spawn()?;
    Ok(())
}

/// Shows the library folder in the system file manager.
#[tauri::command]
pub fn library_open_folder(app: AppHandle) -> Result<(), ModbusError> {
    let root = library_root(&app)?;
    std::fs::create_dir_all(&root)?;
    Ok(open_with_system(root.as_os_str())?)
}

/// Opens one of the project's web pages in the browser. Only these fixed
/// pages can be opened, never a URL from the page.
#[tauri::command]
pub fn open_link(link: String) -> Result<(), ModbusError> {
    let url = match link.as_str() {
        "repo" => "https://github.com/nobrega8/modbench",
        "issues" => "https://github.com/nobrega8/modbench/issues/new/choose",
        "releases" => "https://github.com/nobrega8/modbench/releases",
        "library" => "https://github.com/nobrega8/mbmap-library",
        "spec" => "https://github.com/nobrega8/mbmap-library/blob/main/SPEC.md",
        _ => return Err(ModbusError::Invalid(format!("unknown link: {link}"))),
    };
    Ok(open_with_system(std::ffi::OsStr::new(url))?)
}

#[tauri::command]
pub async fn library_list(app: AppHandle) -> Result<Vec<LibraryEntry>, ModbusError> {
    let root = library_root(&app)?;
    Ok(tokio::task::spawn_blocking(move || library::list(&root))
        .await
        .map_err(|e| ModbusError::Invalid(e.to_string()))??)
}

/// Copies `.mbmap` files into the library (invalid ones are reported, not copied).
#[tauri::command]
pub async fn library_import(
    app: AppHandle,
    paths: Vec<String>,
) -> Result<Vec<ImportResult>, ModbusError> {
    let root = library_root(&app)?;
    tokio::task::spawn_blocking(move || library::import(&root, &paths))
        .await
        .map_err(|e| ModbusError::Invalid(e.to_string()))
}

#[tauri::command]
pub async fn library_delete(app: AppHandle, path: String) -> Result<(), ModbusError> {
    let root = library_root(&app)?;
    Ok(library::delete(&root, &path)?)
}

/// The maps published in the online library (the mbmap-library repository).
#[tauri::command]
pub async fn library_online_index() -> Result<Vec<online::OnlineEntry>, ModbusError> {
    online::index().await.map_err(ModbusError::Invalid)
}

/// Downloads maps of the online library into the local library.
#[tauri::command]
pub async fn library_online_import(
    app: AppHandle,
    paths: Vec<String>,
) -> Result<Vec<ImportResult>, ModbusError> {
    let root = library_root(&app)?;
    let mut results = Vec::new();
    for path in paths {
        let outcome = match online::download(&path).await {
            Ok(bytes) => library::store(&root, &bytes),
            Err(e) => Err(e),
        };
        results.push(ImportResult::new(&path, outcome));
    }
    Ok(results)
}

/// The icon of a vendor's website as a `data:` URL (cached on disk).
#[tauri::command]
pub async fn vendor_icon(app: AppHandle, website: String) -> Option<String> {
    let dir = app.path().app_cache_dir().ok()?.join("vendor-icons");
    online::vendor_icon(&dir, &website).await
}

/// A `.mbmap` passed on the command line (e.g. by double-clicking a file).
#[tauri::command]
pub fn startup_file() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|a| a.to_lowercase().ends_with(".mbmap"))
}

/// Writes text to a file chosen by the user (traffic log export).
#[tauri::command]
pub async fn save_text(path: String, text: String) -> Result<(), ModbusError> {
    tokio::fs::write(&path, text).await?;
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
        RawData::Registers(regs) => match (t.bit, t.bits) {
            (Some(bit), Some(width)) => Ok(Value::Uint(
                codec::extract_field(regs[0], bit, width).into(),
            )),
            _ => codec::decode(&regs, t.ty, t.byte_order, t.bit).map_err(codec_err),
        },
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
        (Area::Holding, DataType::Uint16) if t.bits.is_some() => {
            // Change one bit field and keep the rest: read, modify, write.
            let (bit, width) = (t.bit.unwrap_or(0), t.bits.unwrap_or(16));
            let field = codec::encode(value, DataType::Uint16, ByteOrder::ABCD, None)
                .map_err(codec_err)?[0];
            if u32::from(field) >= 1u32 << width {
                return Err(ModbusError::Invalid(format!(
                    "{field} does not fit in {width} bits"
                )));
            }
            let RawData::Registers(regs) = session.read(Area::Holding, t.address, 1).await? else {
                unreachable!("holding registers return words");
            };
            let word = codec::insert_field(regs[0], bit, width, field);
            session.write_registers(t.address, &[word]).await
        }
        (Area::Holding, ty) => {
            let words = codec::encode(value, ty, t.byte_order, t.length).map_err(codec_err)?;
            session.write_registers(t.address, &words).await
        }
    }
}
