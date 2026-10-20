//! Control channel between the running Modbench window and the MCP server
//! (`modbench-cli mcp`), so an assistant can drive the GUI the operator is
//! looking at.
//!
//! Off unless the operator turns it on in Settings. The app listens on a
//! loopback port only and writes the port and a random token to a file in its
//! data folder (readable by the user only on Unix; the per-user data folder on
//! Windows). Every request carries the token. Requests are lines of JSON:
//!
//! ```text
//! -> {"token": "…", "tool": "get_state", "args": {}}
//! <- {"ok": true, "result": …}   or   {"ok": false, "error": "…"}
//! ```
//!
//! The app does not act on a request itself: it passes it to the window, which
//! applies it to the state the operator sees and answers (see
//! `src/lib/gui-control.svelte.ts`).

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

/// The window must answer within this time.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(30);
const IDENTIFIER: &str = "io.github.nobrega8.modbench";

/// Where the endpoint is written for the MCP server to find.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

/// The file holding the endpoint: in the app's data folder, the same one
/// Tauri uses for the identifier.
pub fn control_file() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base =
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(not(any(windows, target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    base.map(|b| b.join(IDENTIFIER).join("gui-control.json"))
}

fn write_endpoint(path: &std::path::Path, endpoint: &Endpoint) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_vec(endpoint)?;
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        // Removed first so the file is created with the mode below.
        let _ = std::fs::remove_file(path);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(&text)
    }
    #[cfg(not(unix))]
    std::fs::write(path, text)
}

fn random_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("no random numbers: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compares in time that does not depend on where the texts differ.
fn same_token(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= (*a.get(i).unwrap_or(&0) ^ *b.get(i).unwrap_or(&0)) as usize;
    }
    diff == 0
}

type Answer = Pin<Box<dyn Future<Output = Result<Value, String>> + Send>>;
/// Applies one request (tool, arguments) and answers it.
pub type Handler = Arc<dyn Fn(String, Value) -> Answer + Send + Sync>;
/// Told how many clients are connected, whenever it changes.
pub type OnClients = Arc<dyn Fn(usize) + Send + Sync>;

#[derive(Deserialize)]
struct Request {
    token: String,
    tool: String,
    #[serde(default)]
    args: Value,
}

async fn reply(stream: &mut BufReader<TcpStream>, body: Value) {
    let _ = stream
        .get_mut()
        .write_all(format!("{body}\n").as_bytes())
        .await;
}

async fn serve_connection(
    stream: TcpStream,
    token: String,
    handler: Handler,
    clients: Arc<AtomicUsize>,
    on_clients: OnClients,
) {
    let mut stream = BufReader::new(stream);
    let mut counted = false;
    let mut line = String::new();
    loop {
        line.clear();
        match stream.read_line(&mut line).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if line.trim().is_empty() {
            continue;
        }
        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let body = json!({"ok": false, "error": format!("bad request: {e}")});
                reply(&mut stream, body).await;
                continue;
            }
        };
        if !same_token(&request.token, &token) {
            reply(&mut stream, json!({"ok": false, "error": "wrong token"})).await;
            break;
        }
        if !counted {
            counted = true;
            on_clients(clients.fetch_add(1, Ordering::SeqCst) + 1);
        }
        let body = match handler(request.tool, request.args).await {
            Ok(result) => json!({"ok": true, "result": result}),
            Err(error) => json!({"ok": false, "error": error}),
        };
        reply(&mut stream, body).await;
    }
    if counted {
        on_clients(clients.fetch_sub(1, Ordering::SeqCst) - 1);
    }
}

/// Accepts connections until the task is aborted (which ends them too).
async fn accept_loop(
    listener: TcpListener,
    token: String,
    handler: Handler,
    clients: Arc<AtomicUsize>,
    on_clients: OnClients,
) {
    let mut open = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                open.spawn(serve_connection(
                    stream,
                    token.clone(),
                    handler.clone(),
                    clients.clone(),
                    on_clients.clone(),
                ));
            }
            Some(_) = open.join_next(), if !open.is_empty() => {}
        }
    }
}

/// Requests waiting for the window's answer, by id.
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

/// What the app keeps while the channel is on.
struct Running {
    port: u16,
    task: JoinHandle<()>,
    file: Option<PathBuf>,
}

/// State of the channel in the app.
#[derive(Default)]
pub struct Control {
    running: Mutex<Option<Running>>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
    clients: Arc<AtomicUsize>,
}

#[derive(Serialize)]
pub struct Status {
    pub enabled: bool,
    pub port: Option<u16>,
    pub clients: usize,
}

impl Control {
    fn status(&self) -> Status {
        let running = self.running.lock().unwrap();
        Status {
            enabled: running.is_some(),
            port: running.as_ref().map(|r| r.port),
            clients: self.clients.load(Ordering::SeqCst),
        }
    }

    fn stop(&self, app: &AppHandle) {
        if let Some(r) = self.running.lock().unwrap().take() {
            r.task.abort();
            if let Some(file) = r.file {
                let _ = std::fs::remove_file(file);
            }
        }
        self.clients.store(0, Ordering::SeqCst);
        // Requests the window never answered fail.
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(Err("GUI control was turned off".into()));
        }
        let _ = app.emit("gui-control-clients", 0);
    }

    async fn start(&self, app: &AppHandle) -> Result<(), String> {
        self.stop(app);
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| format!("could not listen: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let token = random_token()?;
        let file = control_file();
        if let Some(path) = &file {
            let endpoint = Endpoint {
                port,
                token: token.clone(),
                pid: std::process::id(),
            };
            write_endpoint(path, &endpoint).map_err(|e| format!("{}: {e}", path.display()))?;
        }

        let handler: Handler = {
            let app = app.clone();
            let pending = self.pending.clone();
            let next_id = self.next_id.clone();
            Arc::new(move |tool, args| {
                let app = app.clone();
                let pending = pending.clone();
                let id = next_id.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    let (tx, rx) = oneshot::channel();
                    pending.lock().unwrap().insert(id, tx);
                    let request = json!({"id": id, "tool": tool, "args": args});
                    if app.emit("gui-control-request", request).is_err() {
                        pending.lock().unwrap().remove(&id);
                        return Err("the window is not available".into());
                    }
                    match tokio::time::timeout(ANSWER_TIMEOUT, rx).await {
                        Ok(Ok(answer)) => answer,
                        Ok(Err(_)) => Err("GUI control was turned off".into()),
                        Err(_) => {
                            pending.lock().unwrap().remove(&id);
                            Err("the window did not answer in time".into())
                        }
                    }
                })
            })
        };
        let on_clients: OnClients = {
            let app = app.clone();
            Arc::new(move |n| {
                let _ = app.emit("gui-control-clients", n);
            })
        };
        let task = tauri::async_runtime::spawn(accept_loop(
            listener,
            token,
            handler,
            self.clients.clone(),
            on_clients,
        ));
        *self.running.lock().unwrap() = Some(Running { port, task, file });
        Ok(())
    }

    /// Hands the window's answer to the request waiting for it.
    fn answer(&self, id: u64, answer: Result<Value, String>) {
        if let Some(tx) = self.pending.lock().unwrap().remove(&id) {
            let _ = tx.send(answer);
        }
    }
}

/// Turns the channel on or off.
#[tauri::command]
pub async fn gui_control_enable(
    app: AppHandle,
    control: State<'_, Control>,
    enabled: bool,
) -> Result<Status, String> {
    if enabled {
        control.start(&app).await?;
    } else {
        control.stop(&app);
    }
    Ok(control.status())
}

/// Drops the connected assistants and gives the channel a new port and
/// token. An assistant that reads the endpoint file again can connect again;
/// to keep it out, the operator turns the setting off.
#[tauri::command]
pub async fn gui_control_disconnect(
    app: AppHandle,
    control: State<'_, Control>,
) -> Result<Status, String> {
    if control.status().enabled {
        control.start(&app).await?;
    }
    Ok(control.status())
}

#[tauri::command]
pub fn gui_control_status(control: State<'_, Control>) -> Status {
    control.status()
}

/// The window's answer to a request.
#[tauri::command]
pub fn gui_control_reply(control: State<'_, Control>, id: u64, ok: bool, value: Value) {
    control.answer(
        id,
        if ok {
            Ok(value)
        } else {
            Err(value.as_str().unwrap_or("failed").to_string())
        },
    );
}

/// Removes the endpoint file on the way out.
pub fn shut_down(control: &Control, app: &AppHandle) {
    control.stop(app);
}

// ---- client side, for the MCP server

const OFF: &str =
    "Modbench is not running, or GUI control is off (turn it on in Settings > Connection)";

/// Sends one request to the running app and returns its answer.
pub async fn call(tool: &str, args: &Value) -> Result<Value, String> {
    let path = control_file().ok_or(OFF)?;
    let text = std::fs::read_to_string(&path).map_err(|_| OFF.to_string())?;
    let endpoint: Endpoint = serde_json::from_str(&text).map_err(|_| OFF.to_string())?;
    let stream = tokio::time::timeout(
        Duration::from_secs(2),
        TcpStream::connect(("127.0.0.1", endpoint.port)),
    )
    .await
    .map_err(|_| OFF.to_string())?
    .map_err(|_| OFF.to_string())?;
    let mut stream = BufReader::new(stream);
    let body = json!({"token": endpoint.token, "tool": tool, "args": args});
    stream
        .get_mut()
        .write_all(format!("{body}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut line = String::new();
    tokio::time::timeout(
        ANSWER_TIMEOUT + Duration::from_secs(5),
        stream.read_line(&mut line),
    )
    .await
    .map_err(|_| "Modbench did not answer in time".to_string())?
    .map_err(|e| e.to_string())?;
    let answer: Value =
        serde_json::from_str(&line).map_err(|_| "Modbench closed the connection".to_string())?;
    if answer["ok"].as_bool() == Some(true) {
        Ok(answer["result"].clone())
    } else {
        Err(answer["error"].as_str().unwrap_or("failed").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_compare_whole() {
        assert!(same_token("abc", "abc"));
        assert!(!same_token("abc", "abd"));
        assert!(!same_token("abc", "abcd"));
        assert!(!same_token("", "a"));
    }

    #[test]
    fn tokens_are_random_hex() {
        let (a, b) = (random_token().unwrap(), random_token().unwrap());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    /// A server whose handler answers `echo` and fails any other tool.
    async fn server() -> (u16, JoinHandle<()>, Arc<AtomicUsize>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handler: Handler = Arc::new(|tool, args| {
            Box::pin(async move {
                match tool.as_str() {
                    "echo" => Ok(args),
                    _ => Err(format!("unknown tool {tool}")),
                }
            })
        });
        let clients = Arc::new(AtomicUsize::new(0));
        let task = tauri::async_runtime::spawn(accept_loop(
            listener,
            "secret".into(),
            handler,
            clients.clone(),
            Arc::new(|_| {}),
        ));
        (port, task, clients)
    }

    async fn send(s: &mut BufReader<TcpStream>, token: &str, tool: &str) -> Value {
        let body = json!({"token": token, "tool": tool, "args": {"n": 1}});
        s.get_mut()
            .write_all(format!("{body}\n").as_bytes())
            .await
            .unwrap();
        let mut line = String::new();
        s.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }

    async fn connect(port: u16) -> BufReader<TcpStream> {
        BufReader::new(TcpStream::connect(("127.0.0.1", port)).await.unwrap())
    }

    #[tokio::test]
    async fn answers_with_the_right_token_only() {
        let (port, task, _) = server().await;
        let ok = send(&mut connect(port).await, "secret", "echo").await;
        assert_eq!(ok, json!({"ok": true, "result": {"n": 1}}));
        let err = send(&mut connect(port).await, "secret", "nope").await;
        assert_eq!(err["ok"], false);
        let wrong = send(&mut connect(port).await, "guess", "echo").await;
        assert_eq!(wrong, json!({"ok": false, "error": "wrong token"}));
        task.abort();
    }

    #[tokio::test]
    async fn counts_only_authenticated_clients() {
        let (port, task, clients) = server().await;
        let _ = send(&mut connect(port).await, "guess", "echo").await;
        assert_eq!(clients.load(Ordering::SeqCst), 0);
        let mut s = connect(port).await;
        let _ = send(&mut s, "secret", "echo").await;
        assert_eq!(clients.load(Ordering::SeqCst), 1);
        drop(s);
        for _ in 0..50 {
            if clients.load(Ordering::SeqCst) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(clients.load(Ordering::SeqCst), 0);
        task.abort();
    }
}
