//! Tauri commands exposed to the UI.

use std::sync::Arc;

use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

use crate::check;
use crate::codec::{self, ByteOrder, DataType, Value};
use crate::export;
use crate::gateway;
use crate::generators::{self, Generator};
use crate::library::{self, ImportResult, LibraryEntry};
use crate::mbmap::{self, DeviceMap, Diagnostic, Parsed, Register};
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
    /// Writes a single holding register with FC16 instead of FC06.
    single_as_multiple: Arc<std::sync::atomic::AtomicBool>,
    /// The simulated devices: their memory and generators live on while
    /// the simulator is stopped.
    sim_devices: SimDevices,
    sim: std::sync::Mutex<Option<simulator::Running>>,
    gateway: std::sync::Mutex<Option<gateway::Running>>,
    /// What the map builder's window should open next; it takes it on load.
    builder_target: std::sync::Mutex<Option<BuilderTarget>>,
}

/// A map to open in the map builder, and the register to show in it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuilderTarget {
    pub path: String,
    /// The register to scroll to, found by area, address and name.
    pub register: Option<BuilderRegister>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuilderRegister {
    pub area: Area,
    pub address: u16,
    pub name: String,
}

/// Tells every window the simulator changed (started, stopped, loaded, new
/// generators, values set by hand), so a popped-out simulator and the main
/// window show the same thing.
fn sim_changed(app: &AppHandle) {
    let _ = app.emit("sim-state", ());
}

/// Label of the simulator's own window.
pub const SIM_WINDOW: &str = "simulator";

/// Label of the map builder's own window.
pub const BUILDER_WINDOW: &str = "map-builder";

impl AppState {
    fn stop_poller(&self) {
        if let Some(handle) = self.poller.lock().unwrap().take() {
            handle.abort();
        }
    }

    /// Stops polling and closes the connection, freeing its port. Called
    /// when the app exits.
    pub async fn shut_down(&self) {
        self.stop_poller();
        if let Some(session) = self.session.lock().await.take() {
            session.disconnect().await;
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

/// The package manager that owns this copy, so updates come from it rather than
/// the in-app updater: "flatpak" inside a Flatpak sandbox, "apt" when installed
/// from the .deb, none otherwise. An AppImage sets `APPIMAGE` and updates
/// itself, even on a system with the .deb.
#[tauri::command]
pub fn package_manager() -> Option<&'static str> {
    if !cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some() {
        None
    } else if std::path::Path::new("/.flatpak-info").exists() {
        Some("flatpak")
    } else if std::path::Path::new("/var/lib/dpkg/info/modbench.list").exists() {
        Some("apt")
    } else {
        None
    }
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
    // Free the old port before opening: it may be the same one.
    if let Some(old) = slot.take() {
        old.disconnect().await;
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
    session.set_single_as_multiple(state.single_as_multiple.clone());
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

/// Makes single-register writes use FC16 (on) or FC06 (off, the default).
/// Applies at once, also to a connection that is already open.
#[tauri::command]
pub fn set_single_write_fc16(state: State<'_, AppState>, on: bool) {
    state
        .single_as_multiple
        .store(on, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), ModbusError> {
    state.stop_poller();
    if let Some(session) = state.session.lock().await.take() {
        session.disconnect().await;
    }
    Ok(())
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

/// Reads every register of a map once and checks the results (see
/// `check`). Nothing is written to the device.
#[tauri::command]
pub async fn check_device(
    state: State<'_, AppState>,
    map: DeviceMap,
    request: PollRequest,
) -> Result<check::CheckReport, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    let (results, _) = poller::poll_once(session, &request).await;
    Ok(check::evaluate(&map, &results))
}

/// The commissioning report as one HTML page.
#[tauri::command]
pub fn check_html(report: check::CheckReport, context: check::ReportContext) -> String {
    check::html(&report, &context)
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

/// A map file written as the configuration of other software, and the file
/// name it is usually saved as (see `export`).
#[derive(serde::Serialize)]
pub struct Exported {
    text: String,
    file_name: String,
}

#[tauri::command]
pub async fn export_map(
    path: String,
    format: export::Format,
    options: export::Options,
) -> Result<Exported, ModbusError> {
    let bytes = tokio::fs::read(&path).await?;
    let map = mbmap::parse(&bytes)
        .map
        .ok_or_else(|| ModbusError::Invalid("the map has errors".into()))?;
    Ok(Exported {
        text: export::export(&map, format, &options),
        file_name: format.file_name(&map),
    })
}

/// Checks map files against the connected device by their identity
/// registers, and lists the maps that fit, best first.
#[tauri::command]
pub async fn find_maps(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<crate::mapfind::MapSearch, ModbusError> {
    let mut maps = Vec::new();
    for path in paths {
        if let Ok(bytes) = tokio::fs::read(&path).await
            && let Some(map) = mbmap::parse(&bytes).map
        {
            maps.push((path, map));
        }
    }
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    Ok(crate::mapfind::find_maps(session, &maps).await)
}

/// Sends a request built by hand (function code and data) over the open
/// connection. The frames show on the Raw data tab and in the log.
#[tauri::command]
pub async fn send_pdu(
    state: State<'_, AppState>,
    function: u8,
    data: Vec<u8>,
    broadcast: bool,
) -> Result<(), ModbusError> {
    let mut slot = state.session.lock().await;
    slot.as_mut()
        .ok_or(ModbusError::NotConnected)?
        .send_pdu(function, data, broadcast)
        .await
}

/// Sends bytes exactly as given, without framing, over a link of its own
/// (the open connection is closed first: a serial port can only be opened
/// once), and returns what comes back within `wait_ms`. Refused in
/// read-only mode: raw bytes can do anything.
#[tauri::command]
pub async fn send_raw(
    app: AppHandle,
    state: State<'_, AppState>,
    config: ConnectionConfig,
    bytes: Vec<u8>,
    wait_ms: u64,
) -> Result<Vec<u8>, ModbusError> {
    if state.read_only.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(ModbusError::ReadOnly);
    }
    state.stop_poller();
    if let Some(old) = state.session.lock().await.take() {
        old.disconnect().await;
    }
    let emit = |direction, bytes: &[u8]| {
        let _ = app.emit(
            "raw",
            RawChunk {
                time_ms: now_ms(),
                direction,
                bytes: bytes.to_vec(),
            },
        );
    };
    let started = std::time::Instant::now();
    let answer = modbus::send_raw(&config, &bytes, wait_ms, emit).await;
    let note = match &answer {
        Ok(back) => format!(
            "Raw bytes: {} sent, {} received in {} ms",
            bytes.len(),
            back.len(),
            started.elapsed().as_millis()
        ),
        Err(e) => format!("Raw bytes: {} sent, failed: {e}", bytes.len()),
    };
    let _ = app.emit(
        "traffic",
        modbus::TrafficEntry {
            time_ms: now_ms(),
            function: 0,
            address: 0,
            count: 0,
            duration_ms: started.elapsed().as_secs_f64() * 1000.0,
            error: None,
            note: Some(note),
        },
    );
    answer
}

/// The map of the demo's simulated meter, built into the app.
#[tauri::command]
pub fn demo_map() -> Parsed {
    mbmap::parse(include_bytes!("demo.mbmap.json"))
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

/// Opens a map of the library in the system's text editor.
#[tauri::command]
pub fn library_edit(app: AppHandle, path: String) -> Result<(), ModbusError> {
    let target = library::library_map(&library_root(&app)?, &path)?;
    // On macOS `open -t` picks the default text editor; elsewhere the app
    // that opens JSON files.
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-t")
            .arg(&target)
            .spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Ok(open_with_system(target.as_os_str())?)
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
        "support" => "https://buymeacoffee.com/nobrega8",
        "manual" => {
            "https://raw.githubusercontent.com/nobrega8/modbench/main/manual/modbench-manual.pdf"
        }
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

/// Saves a map from the builder into the library (see `library::save_from_builder`)
/// and tells the other windows that the library changed.
#[tauri::command]
pub async fn library_save_map(
    app: AppHandle,
    map: DeviceMap,
    path: Option<String>,
    replace: bool,
) -> Result<library::SaveOutcome, ModbusError> {
    let root = library_root(&app)?;
    let outcome = tokio::task::spawn_blocking(move || {
        library::save_from_builder(&root, &map, path.as_deref(), replace)
    })
    .await
    .map_err(|e| ModbusError::Invalid(e.to_string()))?
    .map_err(ModbusError::Invalid)?;
    if !outcome.exists {
        let _ = app.emit("library-changed", ());
    }
    Ok(outcome)
}

/// Copies a library map as "<model> copy"; returns the new map's path.
#[tauri::command]
pub async fn library_duplicate(app: AppHandle, path: String) -> Result<String, ModbusError> {
    let root = library_root(&app)?;
    tokio::task::spawn_blocking(move || library::duplicate(&root, &path))
        .await
        .map_err(|e| ModbusError::Invalid(e.to_string()))?
        .map_err(ModbusError::Invalid)
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

/// Downloads one map of the online library and parses it, without adding it
/// to the local library (to show its details).
#[tauri::command]
pub async fn library_online_map(path: String) -> Result<Parsed, ModbusError> {
    let bytes = online::download(&path)
        .await
        .map_err(ModbusError::Invalid)?;
    Ok(mbmap::parse(&bytes))
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
            Ok(bytes) => library::store(&root, &bytes, library::Origin::Online),
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

/// A file Modbench opens when given one: a map, or a simulator setup.
pub fn is_openable_file(name: &str) -> bool {
    mbmap::is_map_file(name) || name.to_ascii_lowercase().ends_with(".mbsim")
}

/// A map or simulator setup passed on the command line (e.g. by
/// double-clicking a file).
#[tauri::command]
pub fn startup_file() -> Option<String> {
    std::env::args().skip(1).find(|a| is_openable_file(a))
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

/// Writes bytes to a file chosen by the user (chart image export).
#[tauri::command]
pub async fn save_bytes(path: String, bytes: Vec<u8>) -> Result<(), ModbusError> {
    tokio::fs::write(&path, bytes).await?;
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
        old.disconnect().await;
    }
    state
        .scan_cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    modbus::scan_serial(&scan, &state.scan_cancel, |p| {
        let _ = app.emit("serial-scan-progress", p);
    })
    .await
}

/// Looks for Modbus TCP devices on a range of addresses. Each address that
/// accepts the connection is sent as a `net-scan-hit` event when found, and
/// progress as `net-scan-progress`; `cancel_scan` stops it. It needs no
/// connection and leaves the open one alone.
#[tauri::command]
pub async fn scan_network(
    app: AppHandle,
    state: State<'_, AppState>,
    scan: crate::netscan::NetScan,
) -> Result<Vec<crate::netscan::NetHit>, ModbusError> {
    state
        .scan_cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let hit_app = app.clone();
    crate::netscan::scan(
        &scan,
        &state.scan_cancel,
        |hit| {
            let _ = hit_app.emit("net-scan-hit", hit);
        },
        |p| {
            let _ = app.emit("net-scan-progress", p);
        },
    )
    .await
}

/// This computer's network interfaces, to choose one to send from.
#[tauri::command]
pub fn local_interfaces() -> Result<Vec<modbus::LocalInterface>, ModbusError> {
    modbus::local_interfaces()
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

/// Tries each read function code once at `address`, and with `writes` the
/// write function codes too (writing back what was read), and says how the
/// device answered.
#[tauri::command]
pub async fn check_functions(
    state: State<'_, AppState>,
    address: u16,
    writes: bool,
) -> Result<Vec<modbus::FunctionCheck>, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session.check_functions(address, writes).await
}

/// The largest block of an area the device returns in one read.
#[tauri::command]
pub async fn largest_read(
    state: State<'_, AppState>,
    area: Area,
    address: u16,
) -> Result<modbus::ReadLimit, ModbusError> {
    let mut slot = state.session.lock().await;
    let session = slot.as_mut().ok_or(ModbusError::NotConnected)?;
    session.largest_read(area, address).await
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

/// The simulated devices: one that answers every unit ID to begin with.
pub struct SimDevices(simulator::Devices);

impl Default for SimDevices {
    fn default() -> Self {
        Self(Arc::new(std::sync::Mutex::new(vec![
            simulator::Device::new(None, simulator::Memory::default()),
        ])))
    }
}

impl AppState {
    /// One simulated device (its memory and generators are shared).
    fn sim_device(&self, device: usize) -> Result<simulator::Device, ModbusError> {
        self.sim_devices
            .0
            .lock()
            .unwrap()
            .get(device)
            .cloned()
            .ok_or_else(|| ModbusError::Invalid(format!("no simulated device {device}")))
    }
}

/// Starts the simulator the way `listen` says: Modbus TCP or UDP, or RTU
/// over TCP or UDP, on a port (host 127.0.0.1 for this computer only, 0.0.0.0 for every
/// interface), or RTU or ASCII on a serial port. Each request it answers is
/// sent to the UI as a `sim` event. Returns where it listens.
#[tauri::command]
pub async fn sim_start(
    app: AppHandle,
    state: State<'_, AppState>,
    listen: modbus::Transport,
) -> Result<modbus::Transport, ModbusError> {
    if let Some(old) = state.sim.lock().unwrap().take() {
        old.stop();
    }
    let port = match &listen {
        modbus::Transport::Tcp { port, .. }
        | modbus::Transport::RtuOverTcp { port, .. }
        | modbus::Transport::Udp { port, .. }
        | modbus::Transport::RtuOverUdp { port, .. } => *port,
        _ => 0,
    };
    let emitter = app.clone();
    let logger: simulator::SimLogger = Arc::new(move |event| {
        let _ = emitter.emit("sim", event);
    });
    let running = simulator::start(listen, state.sim_devices.0.clone(), Some(logger))
        .await
        .map_err(|e| listen_error(port, e))?;
    let listen = running.listen.clone();
    *state.sim.lock().unwrap() = Some(running);
    sim_changed(&app);
    Ok(listen)
}

/// Why listening on `port` failed, in words a person can act on.
fn listen_error(port: u16, e: ModbusError) -> ModbusError {
    match e {
        ModbusError::Io(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            ModbusError::Invalid(format!("port {port} is already in use; choose another"))
        }
        ModbusError::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            ModbusError::Invalid(format!(
                "this system does not allow port {port}; use one above 1023"
            ))
        }
        e => e,
    }
}

/// Starts the gateway (see `gateway`), in place of one already running.
/// Each request it passes on is sent to every window as a "gateway" event.
/// Returns where it listens.
#[tauri::command]
pub async fn gateway_start(
    app: AppHandle,
    state: State<'_, AppState>,
    options: gateway::Options,
) -> Result<modbus::Transport, ModbusError> {
    state.gateway.lock().unwrap().take();
    let port = match &options.listen {
        modbus::Transport::Tcp { port, .. }
        | modbus::Transport::RtuOverTcp { port, .. }
        | modbus::Transport::Udp { port, .. }
        | modbus::Transport::RtuOverUdp { port, .. } => *port,
        _ => 0,
    };
    let emitter = app.clone();
    let logger: gateway::GatewayLogger = Arc::new(move |event| {
        let _ = emitter.emit("gateway", event);
    });
    let running = gateway::start(options, Some(logger))
        .await
        .map_err(|e| listen_error(port, e))?;
    let listen = running.listen.clone();
    *state.gateway.lock().unwrap() = Some(running);
    Ok(listen)
}

/// Stops the gateway and closes its upstream link.
#[tauri::command]
pub fn gateway_stop(state: State<'_, AppState>) {
    state.gateway.lock().unwrap().take();
}

#[tauri::command]
pub fn sim_stop(app: AppHandle, state: State<'_, AppState>) {
    if let Some(running) = state.sim.lock().unwrap().take() {
        running.stop();
    }
    sim_changed(&app);
}

/// Adds a device that answers `unit` (None: every unit ID no other device
/// has), with empty memory. Returns its index.
#[tauri::command]
pub fn sim_add_device(
    app: AppHandle,
    state: State<'_, AppState>,
    unit: Option<u8>,
) -> Result<usize, ModbusError> {
    let mut list = state.sim_devices.0.lock().unwrap();
    let units = list.iter().map(|d| d.unit).chain([unit]);
    if let Some(problem) = simulator::unit_problem(units) {
        return Err(ModbusError::Invalid(problem));
    }
    list.push(simulator::Device::new(unit, simulator::Memory::default()));
    let index = list.len() - 1;
    drop(list);
    sim_changed(&app);
    Ok(index)
}

/// Removes a device; the last one stays.
#[tauri::command]
pub fn sim_remove_device(
    app: AppHandle,
    state: State<'_, AppState>,
    device: usize,
) -> Result<(), ModbusError> {
    let mut list = state.sim_devices.0.lock().unwrap();
    if list.len() < 2 || device >= list.len() {
        return Err(ModbusError::Invalid(
            "the simulator keeps at least one device".into(),
        ));
    }
    list.remove(device);
    drop(list);
    sim_changed(&app);
    Ok(())
}

/// Changes the unit ID a device answers (None: every one no other has).
#[tauri::command]
pub fn sim_set_unit(
    app: AppHandle,
    state: State<'_, AppState>,
    device: usize,
    unit: Option<u8>,
) -> Result<(), ModbusError> {
    let mut list = state.sim_devices.0.lock().unwrap();
    let units = list
        .iter()
        .enumerate()
        .map(|(i, d)| if i == device { unit } else { d.unit });
    if let Some(problem) = simulator::unit_problem(units) {
        return Err(ModbusError::Invalid(problem));
    }
    list.get_mut(device)
        .ok_or_else(|| ModbusError::Invalid(format!("no simulated device {device}")))?
        .unit = unit;
    drop(list);
    sim_changed(&app);
    Ok(())
}

/// Clears a device's memory, or shapes it after a map (see
/// `Memory::from_map`).
#[tauri::command]
pub fn sim_load(
    app: AppHandle,
    state: State<'_, AppState>,
    device: usize,
    map: Option<DeviceMap>,
    strict: bool,
    source: Option<String>,
) -> Result<(), ModbusError> {
    let loaded = map.is_some();
    let memory = state.sim_device(device)?.memory;
    *memory.lock().unwrap() = match map {
        Some(map) => simulator::Memory::from_map(&map, strict),
        None => simulator::Memory::default(),
    };
    if let Some(d) = state.sim_devices.0.lock().unwrap().get_mut(device) {
        d.source = source.filter(|_| loaded);
    }
    sim_changed(&app);
    Ok(())
}

/// One simulated device as a window shows it.
#[derive(serde::Serialize)]
pub struct SimDeviceInfo {
    unit: Option<u8>,
    map: Option<DeviceMap>,
    source: Option<String>,
    strict: bool,
    generators: Vec<Generator>,
    problems: Vec<Option<String>>,
}

/// Everything about the simulator a window needs to show it: whether and
/// where it listens, its devices, and whether it has a window of its own.
#[derive(serde::Serialize)]
pub struct SimInfo {
    listen: Option<modbus::Transport>,
    devices: Vec<SimDeviceInfo>,
    window_open: bool,
}

#[tauri::command]
pub fn sim_info(app: AppHandle, state: State<'_, AppState>) -> SimInfo {
    let listen = state.sim.lock().unwrap().as_ref().map(|r| r.listen.clone());
    let list = state.sim_devices.0.lock().unwrap().clone();
    let devices = list
        .iter()
        .map(|d| {
            let memory = d.memory.lock().unwrap();
            let generators = d.generators.lock().unwrap().clone();
            SimDeviceInfo {
                unit: d.unit,
                map: memory.map().cloned(),
                source: d.source.clone(),
                strict: memory.strict(),
                problems: generators
                    .iter()
                    .map(|g| generators::problem(&memory, g))
                    .collect(),
                generators,
            }
        })
        .collect();
    SimInfo {
        listen,
        devices,
        window_open: app.get_webview_window(SIM_WINDOW).is_some(),
    }
}

/// Opens one of the app's tools in a window of its own, or brings that window
/// to the front when it is open. `query` tells the page which tool to show.
// Async callers only: building a window inside a synchronous command
// deadlocks on Windows.
fn open_tool_window(
    app: &AppHandle,
    label: &str,
    query: &str,
    title: &str,
    size: (f64, f64),
) -> Result<(), ModbusError> {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let url = tauri::WebviewUrl::App(query.into());
    let builder = tauri::WebviewWindowBuilder::new(app, label, url)
        .title(title)
        .inner_size(size.0, size.1)
        .min_inner_size(640.0, 480.0);
    // Like the main window: macOS keeps its own buttons over the content,
    // elsewhere the page draws them.
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(18.0, 24.0));
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);
    builder
        .build()
        .map_err(|e| ModbusError::Invalid(format!("could not open the window: {e}")))?;
    Ok(())
}

/// Shows the simulator in a window of its own (or brings that window to
/// the front), so it can sit next to the main window.
#[tauri::command]
pub async fn sim_open_window(app: AppHandle) -> Result<(), ModbusError> {
    open_tool_window(
        &app,
        SIM_WINDOW,
        "?window=simulator",
        "Modbench Simulator",
        (1040.0, 780.0),
    )?;
    let _ = app.emit("sim-window", true);
    Ok(())
}

/// Closes the simulator's own window; the main window shows it again.
#[tauri::command]
pub fn sim_close_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(SIM_WINDOW) {
        let _ = window.close();
    }
}

/// Shows the map builder in a window of its own (or brings that window to
/// the front), so it can sit next to the main window and the manual.
#[tauri::command]
pub async fn builder_open_window(
    app: AppHandle,
    state: State<'_, AppState>,
    target: Option<BuilderTarget>,
) -> Result<(), ModbusError> {
    let open = app.get_webview_window(BUILDER_WINDOW).is_some();
    *state.builder_target.lock().unwrap() = target;
    open_tool_window(
        &app,
        BUILDER_WINDOW,
        "?window=builder",
        "Modbench Map builder",
        (1180.0, 820.0),
    )?;
    // A window that is already open does not load again: tell it to look.
    if open {
        let _ = app.emit_to(BUILDER_WINDOW, "builder-load", ());
    }
    Ok(())
}

/// The map the builder was asked to open, once; None when it was not asked.
#[tauri::command]
pub fn builder_take_target(state: State<'_, AppState>) -> Option<BuilderTarget> {
    state.builder_target.lock().unwrap().take()
}

/// Closes the map builder's own window.
#[tauri::command]
pub fn builder_close_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window(BUILDER_WINDOW) {
        let _ = window.close();
    }
}

/// Checks a map that is being built with the rules a file gets: the map is
/// written as a version 2 document and read back, so the diagnostics point
/// into `/registers/N` like a file's do.
#[tauri::command]
pub fn validate_map(map: DeviceMap) -> Vec<Diagnostic> {
    mbmap::parse(mbmap::to_string(&map).as_bytes()).diagnostics
}

/// Replaces a device's generators. Returns, for each one, why it cannot run
/// against the device's memory (e.g. a register its map does not have), or
/// None; those are kept and skipped until they can.
#[tauri::command]
pub fn sim_set_generators(
    app: AppHandle,
    state: State<'_, AppState>,
    device: usize,
    list: Vec<Generator>,
) -> Result<Vec<Option<String>>, ModbusError> {
    let device = state.sim_device(device)?;
    let problems = {
        let memory = device.memory.lock().unwrap();
        list.iter()
            .map(|g| generators::problem(&memory, g))
            .collect()
    };
    *device.generators.lock().unwrap() = list;
    sim_changed(&app);
    Ok(problems)
}

/// The value of every register of the map a device was shaped after, in
/// engineering units and map order; empty without a map.
#[tauri::command]
pub fn sim_values(
    state: State<'_, AppState>,
    device: usize,
) -> Result<Vec<Option<crate::codec::Value>>, ModbusError> {
    let memory = state.sim_device(device)?.memory;
    let values = generators::values(&memory.lock().unwrap());
    Ok(values)
}

/// Sets a register of a device's map, in engineering units.
#[tauri::command]
pub fn sim_set_value(
    app: AppHandle,
    state: State<'_, AppState>,
    device: usize,
    register: String,
    value: f64,
) -> Result<(), ModbusError> {
    let memory = state.sim_device(device)?.memory;
    let target = generators::Target::Register { register };
    generators::write(&mut memory.lock().unwrap(), &target, value).map_err(ModbusError::Invalid)?;
    sim_changed(&app);
    Ok(())
}

/// Values held by a simulated device, and which of those addresses exist.
#[derive(serde::Serialize)]
pub struct SimView {
    data: RawData,
    defined: Vec<bool>,
}

#[tauri::command]
pub fn sim_read(
    state: State<'_, AppState>,
    device: usize,
    area: Area,
    address: u16,
    count: u16,
) -> Result<SimView, ModbusError> {
    let memory = state.sim_device(device)?.memory;
    let memory = memory.lock().unwrap();
    let data = memory.read(area, address, count);
    let n = match &data {
        RawData::Bits(b) => b.len(),
        RawData::Registers(w) => w.len(),
    };
    Ok(SimView {
        defined: (0..n)
            .map(|i| memory.defined(area, address + i as u16))
            .collect(),
        data,
    })
}

/// Changes values in a simulated device, in any area.
#[tauri::command]
pub fn sim_write(
    app: AppHandle,
    state: State<'_, AppState>,
    device: usize,
    area: Area,
    address: u16,
    data: RawData,
) -> Result<(), ModbusError> {
    let memory = state.sim_device(device)?.memory;
    memory
        .lock()
        .unwrap()
        .set(area, address, &data)
        .map_err(ModbusError::Invalid)?;
    sim_changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests_demo {
    #[test]
    fn the_demo_map_is_valid() {
        let parsed = super::demo_map();
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let map = parsed.map.unwrap();
        assert!(map.meta.model.contains("simulated"));
    }

    #[test]
    fn a_map_being_built_is_checked_like_a_file() {
        let mut map = super::demo_map().map.unwrap();
        assert!(super::validate_map(map.clone()).is_empty());
        // Two registers with the same name: the message points at a row.
        let first = map.registers[0].clone();
        map.registers.push(first);
        let found = super::validate_map(map.clone());
        let last = map.registers.len() - 1;
        assert!(
            found.iter().any(|d| d
                .pointer
                .as_deref()
                .is_some_and(|p| p.starts_with(&format!("/registers/{last}")))),
            "{found:?}"
        );
    }
}
