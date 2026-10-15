//! Tauri commands exposed to the UI.

use std::sync::Arc;

use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::codec::{self, ByteOrder, DataType, Value};
use crate::generators::{self, Generator};
use crate::library::{self, ImportResult, LibraryEntry};
use crate::mbmap::{self, DeviceMap, Parsed, Register};
use crate::modbus::{self, ConnectionConfig, ModbusError, RawData, Session};
use crate::online;
use crate::planner::Area;
use crate::poller::{self, PollCycle, PollRequest};
use crate::simulator;
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
    /// Set to stop a scan that is running.
    scan_cancel: Arc<std::sync::atomic::AtomicBool>,
    /// Read-only mode: shared with every session, which refuses writes while
    /// it is on.
    read_only: Arc<std::sync::atomic::AtomicBool>,
    /// The simulated device: its memory lives on while it is stopped.
    sim_memory: Arc<std::sync::Mutex<simulator::Memory>>,
    sim: std::sync::Mutex<Option<simulator::Running>>,
    /// Values the simulator changes on its own; kept across stop and start.
    sim_generators: generators::Shared,
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

/// True when Modbench was installed from the .deb package, so dpkg/apt owns the
/// files and updates come from the APT repository rather than the in-app updater.
/// An AppImage sets `APPIMAGE` and updates itself, even on a system with the .deb.
#[tauri::command]
pub fn installed_with_apt() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("APPIMAGE").is_none()
        && std::path::Path::new("/var/lib/dpkg/info/modbench.list").exists()
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
    session.set_read_only(state.read_only.clone());
    session.set_logger(Arc::new(move |entry| {
        let _ = app.emit("traffic", entry);
    }));
    *slot = Some(session);
    Ok(())
}

/// Whether the open connection looks lost (see `Session::link_lost`).
#[tauri::command]
pub async fn link_lost(state: State<'_, AppState>) -> Result<bool, ModbusError> {
    let slot = state.session.lock().await;
    Ok(slot.as_ref().ok_or(ModbusError::NotConnected)?.link_lost())
}

/// Opens the connection again with the same settings.
#[tauri::command]
pub async fn reconnect(state: State<'_, AppState>) -> Result<(), ModbusError> {
    let mut slot = state.session.lock().await;
    slot.as_mut()
        .ok_or(ModbusError::NotConnected)?
        .reconnect()
        .await
}

/// Turns read-only mode on or off. Applies at once, also to a connection
/// that is already open.
#[tauri::command]
pub fn set_read_only(state: State<'_, AppState>, on: bool) {
    state
        .read_only
        .store(on, std::sync::atomic::Ordering::Relaxed);
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
    max_per_request: Option<u16>,
) -> Result<RawData, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    match max_per_request {
        Some(max) => session.read_chunked(area, address, count, max).await,
        None => session.read(area, address, count).await,
    }
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
        poller::run(
            session,
            request,
            |cycle| match cycle {
                Ok(c) => {
                    let _ = app.emit("poll", c);
                }
                Err(reason) => {
                    let _ = app.emit("poll-stopped", reason);
                }
            },
            |event| {
                let _ = app.emit("link", event);
            },
        )
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
    scale_register: Option<Register>,
) -> Result<(), ModbusError> {
    if state.read_only.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ModbusError::ReadOnly);
    }
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
    check_write_limits(&register, &value)?;
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    // A value with a scale factor is converted with the factor the device
    // reports right now.
    let mut scale = register.scale;
    if let Some(name) = &register.scale_factor {
        let sf = scale_register.filter(|r| &r.name == name).ok_or_else(|| {
            ModbusError::Invalid(format!("scale factor {name:?} is not in the map"))
        })?;
        let exp = read_exponent(session, &sf, byte_order).await?;
        scale *= 10f64.powi(exp);
    }
    let raw = codec::from_engineering(value, scale, register.offset);
    write_typed(session, &target, &raw).await
}

/// Refuses a write outside the register's declared limits (spec 4.5). The
/// limits are in engineering units, like the value.
fn check_write_limits(register: &Register, value: &Value) -> Result<(), ModbusError> {
    let v = match value {
        Value::Int(n) => *n as f64,
        Value::Uint(n) => *n as f64,
        Value::Float(f) => *f,
        _ => return Ok(()),
    };
    let unit = if register.unit.is_empty() {
        String::new()
    } else {
        format!(" {}", register.unit)
    };
    if let Some(lo) = register.write_min
        && v < lo
    {
        return Err(ModbusError::Invalid(format!(
            "{} cannot be below {lo}{unit} (asked for {v})",
            register.name
        )));
    }
    if let Some(hi) = register.write_max
        && v > hi
    {
        return Err(ModbusError::Invalid(format!(
            "{} cannot be above {hi}{unit} (asked for {v})",
            register.name
        )));
    }
    Ok(())
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    fn limited(lo: Option<f64>, hi: Option<f64>) -> Register {
        let text =
            "@mbmap=1\n@vendor=T\n@model=T\nname,area,address,type,access\nA,holding,0,uint16,RW\n";
        let mut r = mbmap::parse(text.as_bytes())
            .map
            .unwrap()
            .registers
            .remove(0);
        r.write_min = lo;
        r.write_max = hi;
        r.unit = "C".into();
        r
    }

    #[test]
    fn write_limits_are_inclusive_and_optional() {
        let r = limited(Some(5.0), Some(30.0));
        assert!(check_write_limits(&r, &Value::Float(5.0)).is_ok());
        assert!(check_write_limits(&r, &Value::Float(30.0)).is_ok());
        let below = check_write_limits(&r, &Value::Float(4.9)).unwrap_err();
        assert!(below.to_string().contains("below 5 C"), "{below}");
        let above = check_write_limits(&r, &Value::Uint(31)).unwrap_err();
        assert!(above.to_string().contains("above 30 C"), "{above}");
        // One side open, and non-numeric values are not limited.
        assert!(check_write_limits(&limited(Some(0.0), None), &Value::Int(1_000)).is_ok());
        assert!(check_write_limits(&r, &Value::Bool(true)).is_ok());
    }
}

/// Reads a scale factor register and returns its exponent.
async fn read_exponent(
    session: &mut Session,
    sf: &Register,
    byte_order: ByteOrder,
) -> Result<i32, ModbusError> {
    let RawData::Registers(words) = session.read(sf.area, sf.address, sf.count()).await? else {
        return Err(ModbusError::Invalid(format!(
            "{} is not a register",
            sf.name
        )));
    };
    let order = sf.byte_order.unwrap_or(byte_order);
    let exp = match codec::decode(&words, sf.ty, order, None)
        .map_err(|e| ModbusError::Invalid(e.to_string()))?
    {
        Value::Int(n) => n as f64,
        Value::Uint(n) => n as f64,
        _ => f64::NAN,
    };
    if !(-20.0..=20.0).contains(&exp) {
        return Err(ModbusError::Invalid(format!(
            "scale factor {:?} reads {exp}, not a usable exponent",
            sf.name
        )));
    }
    Ok(exp as i32)
}

/// Reads and checks a map file of either version.
#[tauri::command]
pub async fn open_map(path: String) -> Result<Parsed, ModbusError> {
    let bytes = tokio::fs::read(&path).await?;
    Ok(mbmap::parse(&bytes))
}

/// Writes a map to a file, in format version 2.
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
        "support" => "https://ko-fi.com/nobrega",
        _ => return Err(ModbusError::Invalid(format!("unknown link: {link}"))),
    };
    Ok(open_with_system(std::ffi::OsStr::new(url))?)
}

#[tauri::command]
pub async fn library_list(app: AppHandle) -> Result<Vec<LibraryEntry>, ModbusError> {
    let root = library_root(&app)?;
    Ok(tokio::task::spawn_blocking(move || {
        // Maps stored by older releases are in the deprecated version 1.
        library::upgrade(&root)?;
        library::list(&root)
    })
    .await
    .map_err(|e| ModbusError::Invalid(e.to_string()))??)
}

/// Copies map files into the library as version 2 (invalid ones are reported, not copied).
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
    std::env::args().skip(1).find(|a| mbmap::is_map_file(a))
}

/// Converts a version 1 map file to version 2, next to it (`x.mbmap` ->
/// `x.mbmap.json`). Returns the new path; the original is kept.
#[tauri::command]
pub async fn convert_map(path: String) -> Result<String, ModbusError> {
    let parsed = mbmap::parse(&tokio::fs::read(&path).await?);
    let Some(map) = parsed.map else {
        return Err(ModbusError::Invalid(
            "the map has errors; fix them before converting it".into(),
        ));
    };
    let target = mbmap::v2_file_name(&path);
    tokio::fs::write(&target, mbmap::to_string(&map)).await?;
    Ok(target)
}

/// Writes text to a file chosen by the user (traffic log export).
#[tauri::command]
pub async fn save_text(path: String, text: String) -> Result<(), ModbusError> {
    tokio::fs::write(&path, text).await?;
    Ok(())
}

/// Adds text to the end of a file, creating it if needed (CSV recording).
#[tauri::command]
pub async fn append_text(path: String, text: String) -> Result<(), ModbusError> {
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    file.write_all(text.as_bytes()).await?;
    Ok(())
}

/// Asks every unit ID in `from..=to` whether a device answers. Each unit ID
/// about to be asked is sent to the UI as a `scan-progress` event.
#[tauri::command]
pub async fn scan_units(
    app: AppHandle,
    state: State<'_, AppState>,
    from: u8,
    to: u8,
    timeout_ms: u64,
) -> Result<Vec<modbus::ScanHit>, ModbusError> {
    if from > to {
        return Err(ModbusError::Invalid(
            "the first unit ID is above the last".into(),
        ));
    }
    state.stop_poller();
    state
        .scan_cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    let wait = std::time::Duration::from_millis(timeout_ms.max(1));
    Ok(session
        .scan_units(from, to, wait, &state.scan_cancel, |unit| {
            let _ = app.emit("scan-progress", unit);
        })
        .await)
}

/// Looks for the line settings of a serial device. Progress is sent to the
/// UI as `serial-scan-progress` events; `cancel_scan` stops it.
#[tauri::command]
pub async fn scan_serial(
    app: AppHandle,
    state: State<'_, AppState>,
    scan: modbus::SerialScan,
) -> Result<Vec<modbus::SerialHit>, ModbusError> {
    state.stop_poller();
    // The port must be free: close any connection the app holds.
    if let Some(old) = state.session.lock().await.take() {
        let _ = old.disconnect().await;
    }
    state
        .scan_cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    modbus::scan_serial(&scan, &state.scan_cancel, |p| {
        let _ = app.emit("serial-scan-progress", p);
    })
    .await
}

/// Stops a running scan after the unit ID being asked.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    state
        .scan_cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Reads a text file chosen by the user (a value template).
#[tauri::command]
pub async fn read_text(path: String) -> Result<String, ModbusError> {
    Ok(tokio::fs::read_to_string(&path).await?)
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

/// Reads the device's identification objects (FC43). `level`: 1 basic,
/// 2 regular, 3 extended.
#[tauri::command]
pub async fn read_device_id(
    state: State<'_, AppState>,
    level: u8,
) -> Result<modbus::DeviceIdentity, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session.read_device_id(level).await
}

/// Finds which addresses of an area answer. Progress is sent to the UI as
/// `address-scan-progress` events; `cancel_scan` stops it.
#[tauri::command]
pub async fn scan_addresses(
    app: AppHandle,
    state: State<'_, AppState>,
    scan: modbus::AddressScan,
) -> Result<Vec<modbus::AddressRange>, ModbusError> {
    state.stop_poller();
    state
        .scan_cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session
        .scan_addresses(&scan, &state.scan_cancel, |p| {
            let _ = app.emit("address-scan-progress", p);
        })
        .await
}

/// Starts the simulated device on a TCP port, on this computer only unless
/// `public`. Each request it answers is sent to the UI as a `sim` event.
/// Returns the port.
#[tauri::command]
pub async fn sim_start(
    app: AppHandle,
    state: State<'_, AppState>,
    port: u16,
    public: bool,
    unit: Option<u8>,
) -> Result<u16, ModbusError> {
    if let Some(old) = state.sim.lock().unwrap().take() {
        old.stop();
    }
    let ip = if public { [0, 0, 0, 0] } else { [127, 0, 0, 1] };
    let logger: simulator::SimLogger = Arc::new(move |event| {
        let _ = app.emit("sim", event);
    });
    let running = simulator::start(
        (ip, port).into(),
        state.sim_memory.clone(),
        unit,
        Some(logger),
        Some(state.sim_generators.clone()),
    )
    .await
    .map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => {
            ModbusError::Invalid(format!("port {port} is already in use; choose another"))
        }
        std::io::ErrorKind::PermissionDenied => ModbusError::Invalid(format!(
            "this system does not allow port {port}; use one above 1023"
        )),
        _ => ModbusError::Io(e),
    })?;
    let port = running.addr.port();
    *state.sim.lock().unwrap() = Some(running);
    Ok(port)
}

#[tauri::command]
pub fn sim_stop(state: State<'_, AppState>) {
    if let Some(running) = state.sim.lock().unwrap().take() {
        running.stop();
    }
}

/// The port the simulator listens on, or None when it is stopped.
#[tauri::command]
pub fn sim_status(state: State<'_, AppState>) -> Option<u16> {
    state.sim.lock().unwrap().as_ref().map(|r| r.addr.port())
}

/// Clears the simulator's memory, or shapes it after a map (see
/// `Memory::from_map`).
#[tauri::command]
pub fn sim_load(state: State<'_, AppState>, map: Option<DeviceMap>, strict: bool) {
    *state.sim_memory.lock().unwrap() = match map {
        Some(map) => simulator::Memory::from_map(&map, strict),
        None => simulator::Memory::default(),
    };
}

/// Replaces the simulator's generators. Returns, for each one, why it cannot
/// run against the current memory (e.g. a register the map does not have),
/// or None; those are kept and skipped until they can.
#[tauri::command]
pub fn sim_set_generators(state: State<'_, AppState>, list: Vec<Generator>) -> Vec<Option<String>> {
    let problems = {
        let memory = state.sim_memory.lock().unwrap();
        list.iter()
            .map(|g| generators::problem(&memory, g))
            .collect()
    };
    *state.sim_generators.lock().unwrap() = list;
    problems
}

/// The value of every register of the map the simulator was shaped after,
/// in engineering units and map order; empty without a map.
#[tauri::command]
pub fn sim_values(state: State<'_, AppState>) -> Vec<Option<crate::codec::Value>> {
    generators::values(&state.sim_memory.lock().unwrap())
}

/// Sets a register of the loaded map, in engineering units.
#[tauri::command]
pub fn sim_set_value(
    state: State<'_, AppState>,
    register: String,
    value: f64,
) -> Result<(), ModbusError> {
    let target = generators::Target::Register { register };
    generators::write(&mut state.sim_memory.lock().unwrap(), &target, value)
        .map_err(ModbusError::Invalid)
}

/// Values held by the simulator, and which of those addresses exist.
#[derive(serde::Serialize)]
pub struct SimView {
    data: RawData,
    defined: Vec<bool>,
}

#[tauri::command]
pub fn sim_read(state: State<'_, AppState>, area: Area, address: u16, count: u16) -> SimView {
    let memory = state.sim_memory.lock().unwrap();
    let data = memory.read(area, address, count);
    let n = match &data {
        RawData::Bits(b) => b.len(),
        RawData::Registers(w) => w.len(),
    };
    SimView {
        defined: (0..n)
            .map(|i| memory.defined(area, address + i as u16))
            .collect(),
        data,
    }
}

/// Changes values in the simulator, in any area.
#[tauri::command]
pub fn sim_write(
    state: State<'_, AppState>,
    area: Area,
    address: u16,
    data: RawData,
) -> Result<(), ModbusError> {
    state
        .sim_memory
        .lock()
        .unwrap()
        .set(area, address, &data)
        .map_err(ModbusError::Invalid)
}
