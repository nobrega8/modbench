//! Command-line and MCP access to the Modbench core, without the UI.
//!
//! Read-only on purpose: it lists ports, reads registers, reads every
//! register of a `.mbmap` once, and validates map files. `simulate` runs a
//! simulated device that others can write to; it never writes to a device.

use std::path::Path;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use serde::Serialize;
use serde_json::{Value as Json, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use modbench_lib::check::{self, format_raw, format_value};
use modbench_lib::mbmap::{self, DeviceMap, Severity};
use modbench_lib::modbus::{self, ConnectionConfig, Parity, RawData, Session, Transport};
use modbench_lib::planner::{Area, PlanLimits};
use modbench_lib::poller::{self, PollRequest};

#[derive(Parser)]
#[command(
    name = "modbench-cli",
    version,
    about = "Read Modbus devices from the command line"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List serial ports
    Ports,
    /// Read registers or bits
    Read {
        #[command(flatten)]
        conn: Conn,
        /// coil, discrete, input or holding
        area: String,
        /// Start address (decimal, 0x10 or 10h)
        address: String,
        /// Number of registers or bits
        #[arg(default_value = "1")]
        count: String,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Read every register of a .mbmap file once
    Map {
        #[command(flatten)]
        conn: Conn,
        /// Path to the .mbmap file
        map: String,
        /// Only registers whose name or group contains this text
        #[arg(long)]
        filter: Option<String>,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Check a device against its .mbmap: read every register once and
    /// report pass, fail or not read; exits with 1 when any check fails
    Check {
        #[command(flatten)]
        conn: Conn,
        /// Path to the .mbmap file
        map: String,
        /// Write the report to this HTML file
        #[arg(long)]
        report: Option<String>,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Find which unit IDs answer on a connection
    Scan {
        #[command(flatten)]
        conn: Conn,
        /// First unit ID to try
        #[arg(long, default_value_t = 1)]
        from: u8,
        /// Last unit ID to try
        #[arg(long, default_value_t = 247)]
        to: u8,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Find the baud rate, parity and stop bits of a serial device
    ScanSerial {
        /// Serial port, e.g. COM3 or /dev/ttyUSB0
        #[arg(long)]
        rtu: String,
        /// Baud rates to try, in order
        #[arg(long, value_delimiter = ',', default_value = "9600,19200,38400,115200")]
        bauds: Vec<u32>,
        /// Parities to try, in order: letters from N, E, O
        #[arg(long, default_value = "ENO")]
        parities: String,
        /// Stop bits to try, in order
        #[arg(long, value_delimiter = ',', default_value = "1,2")]
        stop_bits: Vec<u8>,
        /// First unit ID to ask
        #[arg(long, default_value_t = 1)]
        from: u8,
        /// Last unit ID to ask
        #[arg(long, default_value_t = 1)]
        to: u8,
        /// Area of the register to ask for: holding or input
        #[arg(long, default_value = "holding")]
        area: String,
        /// Address of the register to ask for
        #[arg(long, default_value = "0")]
        address: String,
        /// How long to wait for each answer, in milliseconds
        #[arg(long, default_value_t = 200)]
        timeout: u64,
        /// Stop at the first setting that gets an answer
        #[arg(long)]
        first: bool,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Read what a device says about itself (FC43 device identification)
    Id {
        #[command(flatten)]
        conn: Conn,
        /// 1 basic, 2 regular or 3 extended
        #[arg(long, default_value_t = 1)]
        level: u8,
        /// Print JSON
        #[arg(long)]
        json: bool,
    },
    /// Run a simulated Modbus device (TCP, RTU over TCP, or RTU or ASCII on
    /// a serial port), optionally shaped after a map
    Simulate {
        /// Map whose reference values and identity the device takes
        map: Option<String>,
        /// TCP or UDP port to listen on
        #[arg(long, default_value_t = 5020)]
        port: u16,
        /// Serve RTU frames over TCP (like a serial gateway) instead of
        /// Modbus TCP
        #[arg(long, conflicts_with_all = ["rtu", "ascii", "udp", "rtu_over_udp"])]
        rtu_over_tcp: bool,
        /// Serve Modbus TCP frames over UDP instead of TCP
        #[arg(long, conflicts_with_all = ["rtu", "ascii", "rtu_over_udp"])]
        udp: bool,
        /// Serve RTU frames over UDP (like a serial gateway) instead of TCP
        #[arg(long, conflicts_with_all = ["rtu", "ascii"])]
        rtu_over_udp: bool,
        /// Serve Modbus RTU on this serial port instead of TCP
        #[arg(long, conflicts_with = "ascii")]
        rtu: Option<String>,
        /// Serve Modbus ASCII on this serial port instead of TCP
        #[arg(long)]
        ascii: Option<String>,
        /// Serial baud rate (default 9600, or the map's)
        #[arg(long)]
        baud: Option<u32>,
        /// Serial parity: N, E or O (default N, or the map's)
        #[arg(long)]
        parity: Option<String>,
        /// Serial data bits (default 8, or the map's)
        #[arg(long)]
        data_bits: Option<u8>,
        /// Serial stop bits (default 1, or the map's)
        #[arg(long)]
        stop_bits: Option<u8>,
        /// Answer only addresses the map defines (others get exception 2)
        #[arg(long)]
        strict: bool,
        /// Answer only this unit ID (default: all)
        #[arg(long, short)]
        unit: Option<u8>,
        /// Listen on every network interface, not only this computer
        #[arg(long)]
        public: bool,
        /// JSON file with values that change on their own: a list of
        /// generators (increment, sine, random, copy), see the README
        #[arg(long, value_name = "FILE")]
        generators: Option<String>,
    },
    /// Check a map file (.mbmap.json, or a deprecated version 1 .mbmap) against the spec
    Validate { map: String },
    /// Convert a version 1 .mbmap file to version 2 (.mbmap.json)
    Convert {
        map: String,
        /// Where to write the result (default: next to the file, as <name>.mbmap.json)
        #[arg(long)]
        out: Option<String>,
    },
    /// Run as an MCP server over stdio
    Mcp,
}

#[derive(Args, Clone, Default, serde::Deserialize)]
struct Conn {
    /// Modbus TCP device, host[:port]
    #[arg(long)]
    tcp: Option<String>,
    /// Modbus RTU over TCP (serial gateway), host[:port]
    #[arg(long)]
    rtu_over_tcp: Option<String>,
    /// Modbus TCP frames over UDP, host[:port]
    #[arg(long)]
    udp: Option<String>,
    /// Modbus RTU over UDP (serial gateway), host[:port]
    #[arg(long)]
    rtu_over_udp: Option<String>,
    /// Serial port for Modbus RTU, e.g. COM3 or /dev/ttyUSB0
    #[arg(long)]
    rtu: Option<String>,
    /// Serial port for Modbus ASCII
    #[arg(long)]
    ascii: Option<String>,
    /// Baud rate (default 9600, or the map's)
    #[arg(long)]
    baud: Option<u32>,
    /// Parity: N, E or O (default N, or the map's)
    #[arg(long)]
    parity: Option<String>,
    /// Data bits (default 8, or the map's)
    #[arg(long)]
    data_bits: Option<u8>,
    /// Stop bits (default 1, or the map's)
    #[arg(long)]
    stop_bits: Option<u8>,
    /// Unit / slave ID (default 1, or the map's)
    #[arg(long, short)]
    unit: Option<u8>,
    /// Response timeout in milliseconds
    #[arg(long, default_value_t = 1000)]
    #[serde(default = "default_timeout")]
    timeout: u64,
    /// Pause between requests in milliseconds
    #[arg(long, default_value_t = 50)]
    #[serde(default = "default_delay")]
    delay: u64,
    /// Largest number of registers per read request (default 125, or the
    /// map's); longer reads are split
    #[arg(long)]
    max_regs: Option<u16>,
    /// Local IP address to send from over TCP or UDP, to pick the network
    /// interface (default: the system's routing)
    #[arg(long)]
    #[serde(default)]
    source: Option<String>,
}

fn default_timeout() -> u64 {
    1000
}

fn default_delay() -> u64 {
    50
}

fn host_port(text: &str) -> (String, u16) {
    match text.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
        _ => (text.to_string(), 502),
    }
}

impl Conn {
    /// Connection settings; values not given come from the map, then defaults.
    fn config(&self, map: Option<&DeviceMap>) -> Result<ConnectionConfig, String> {
        let meta = map.map(|m| &m.meta);
        let transport = if let Some(t) = &self.tcp {
            let (host, port) = host_port(t);
            Transport::Tcp { host, port }
        } else if let Some(t) = &self.rtu_over_tcp {
            let (host, port) = host_port(t);
            Transport::RtuOverTcp { host, port }
        } else if let Some(t) = &self.udp {
            let (host, port) = host_port(t);
            Transport::Udp { host, port }
        } else if let Some(t) = &self.rtu_over_udp {
            let (host, port) = host_port(t);
            Transport::RtuOverUdp { host, port }
        } else if let Some(port) = self.rtu.as_ref().or(self.ascii.as_ref()) {
            let parity = match self.parity.as_deref().map(str::to_uppercase).as_deref() {
                None => meta.and_then(|m| m.parity).unwrap_or(Parity::N),
                Some("N") => Parity::N,
                Some("E") => Parity::E,
                Some("O") => Parity::O,
                Some(p) => return Err(format!("parity must be N, E or O, not {p}")),
            };
            let port = port.clone();
            let baudrate = self.baud.or(meta.and_then(|m| m.baudrate)).unwrap_or(9600);
            let data_bits = self
                .data_bits
                .or(meta.and_then(|m| m.data_bits))
                .unwrap_or(8);
            let stop_bits = self
                .stop_bits
                .or(meta.and_then(|m| m.stop_bits))
                .unwrap_or(1);
            if self.rtu.is_some() {
                Transport::Rtu {
                    port,
                    baudrate,
                    parity,
                    data_bits,
                    stop_bits,
                }
            } else {
                Transport::Ascii {
                    port,
                    baudrate,
                    parity,
                    data_bits,
                    stop_bits,
                }
            }
        } else {
            return Err(
                "choose a connection: --tcp HOST[:PORT], --udp HOST[:PORT], --rtu PORT, --ascii PORT, --rtu-over-tcp HOST[:PORT] or --rtu-over-udp HOST[:PORT]"
                    .into(),
            );
        };
        Ok(ConnectionConfig {
            transport,
            unit_id: self.unit.or(meta.and_then(|m| m.unit_id)).unwrap_or(1),
            timeout_ms: self.timeout,
            local_address: self.source.clone(),
        })
    }

    fn describe(cfg: &ConnectionConfig) -> String {
        let t = match &cfg.transport {
            Transport::Tcp { host, port } => format!("TCP {host}:{port}"),
            Transport::RtuOverTcp { host, port } => format!("RTU over TCP {host}:{port}"),
            Transport::Udp { host, port } => format!("UDP {host}:{port}"),
            Transport::RtuOverUdp { host, port } => format!("RTU over UDP {host}:{port}"),
            Transport::Rtu {
                port,
                baudrate,
                parity,
                data_bits,
                stop_bits,
            } => format!("RTU {port} {baudrate} {data_bits}{parity:?}{stop_bits}"),
            Transport::Ascii {
                port,
                baudrate,
                parity,
                data_bits,
                stop_bits,
            } => format!("ASCII {port} {baudrate} {data_bits}{parity:?}{stop_bits}"),
        };
        format!("{t}, unit {}", cfg.unit_id)
    }
}

fn parse_int(text: &str) -> Result<u16, String> {
    let t = text.trim();
    let n = if let Some(h) = t.strip_prefix("0x").or(t.strip_prefix("0X")) {
        u32::from_str_radix(h, 16).ok()
    } else if let Some(h) = t.strip_suffix('h').or(t.strip_suffix('H')) {
        u32::from_str_radix(h, 16).ok()
    } else {
        t.parse::<u32>().ok()
    };
    n.filter(|&n| n <= 65535)
        .map(|n| n as u16)
        .ok_or_else(|| format!("not a number from 0 to 65535: {text}"))
}

fn parse_area(text: &str) -> Result<Area, String> {
    match text.to_lowercase().as_str() {
        "coil" | "coils" | "1" => Ok(Area::Coil),
        "discrete" | "2" => Ok(Area::Discrete),
        "holding" | "3" => Ok(Area::Holding),
        "input" | "4" => Ok(Area::Input),
        _ => Err(format!(
            "area must be coil, discrete, input or holding, not {text}"
        )),
    }
}

fn load_map(path: &str) -> Result<DeviceMap, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let parsed = mbmap::parse(&bytes);
    let errors = parsed
        .errors()
        .map(|d| format!("{}: {} [{}]", d.location(), d.message, d.code))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    parsed.map.ok_or(errors)
}

// ---- operations shared by the CLI and the MCP server

#[derive(Serialize)]
struct WordRow {
    address: u32,
    hex: String,
    unsigned: u16,
    signed: i16,
}

async fn read_registers(
    conn: &Conn,
    area: Area,
    address: u16,
    count: u16,
) -> Result<(String, RawData), String> {
    let cfg = conn.config(None)?;
    let mut session = Session::connect(&cfg, None)
        .await
        .map_err(|e| format!("could not connect ({}): {e}", Conn::describe(&cfg)))?;
    let data = session
        .read_chunked(area, address, count, conn.max_regs.unwrap_or(125))
        .await
        .map_err(|e| e.to_string());
    session.disconnect().await;
    Ok((Conn::describe(&cfg), data?))
}

fn raw_rows(address: u16, data: &RawData) -> Vec<WordRow> {
    match data {
        RawData::Registers(words) => words
            .iter()
            .enumerate()
            .map(|(i, &w)| WordRow {
                address: address as u32 + i as u32,
                hex: format!("{w:04X}"),
                unsigned: w,
                signed: w as i16,
            })
            .collect(),
        RawData::Bits(bits) => bits
            .iter()
            .enumerate()
            .map(|(i, &b)| WordRow {
                address: address as u32 + i as u32,
                hex: if b { "1".into() } else { "0".into() },
                unsigned: b as u16,
                signed: b as i16,
            })
            .collect(),
    }
}

fn raw_text(where_: &str, area: Area, address: u16, data: &RawData) -> String {
    let mut out = format!("{where_}: {area:?} {address}..\n");
    for r in raw_rows(address, data) {
        out.push_str(&format!(
            "{:>6}  {:>4}  {:>6}  {:>7}\n",
            r.address, r.hex, r.unsigned, r.signed
        ));
    }
    out
}

#[derive(Serialize)]
struct MapRow {
    name: String,
    group: String,
    address: u16,
    #[serde(rename = "type")]
    ty: String,
    value: Option<String>,
    unit: String,
    raw: Option<String>,
    error: Option<String>,
    /// "match" or "mismatch" for registers with a reference value.
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<&'static str>,
}

/// Reads the registers of a map once. Returns the map, the connection it
/// used and what each register gave (by position in the map).
async fn poll_map(
    conn: &Conn,
    path: &str,
    filter: Option<&str>,
) -> Result<(DeviceMap, ConnectionConfig, Vec<poller::PointResult>), String> {
    let map = load_map(path)?;
    let q = filter.map(str::to_lowercase);
    let wanted: Vec<usize> = (0..map.registers.len())
        .filter(|&i| {
            let r = &map.registers[i];
            q.as_ref().is_none_or(|q| {
                r.name.to_lowercase().contains(q) || r.group.to_lowercase().contains(q)
            })
        })
        .collect();
    if wanted.is_empty() {
        return Err("no registers match the filter".into());
    }
    let cfg = conn.config(Some(&map))?;
    let request = PollRequest {
        registers: wanted.iter().map(|&i| map.registers[i].clone()).collect(),
        byte_order: map.meta.byte_order(),
        byte_order_override: None,
        interval_ms: 0,
        delay_ms: conn.delay,
        limits: PlanLimits {
            max_registers: conn.max_regs.map_or(map.meta.max_read_registers(), |n| {
                n.min(map.meta.max_read_registers())
            }),
            max_bits: map.meta.max_read_bits(),
            max_gap: 0,
        },
        indexes: wanted.clone(),
        sentinels: map.meta.sentinels.clone(),
        scale_registers: map.scale_registers(&wanted),
        reconnect: false,
    };
    let mut session = Session::connect(&cfg, None)
        .await
        .map_err(|e| format!("could not connect ({}): {e}", Conn::describe(&cfg)))?;
    let (mut results, _) = poller::poll_once(&mut session, &request).await;
    session.disconnect().await;
    results.sort_by_key(|r| r.index);
    Ok((map, cfg, results))
}

async fn read_map(
    conn: &Conn,
    path: &str,
    filter: Option<&str>,
) -> Result<(String, Vec<MapRow>), String> {
    let (map, cfg, results) = poll_map(conn, path, filter).await?;
    let rows = results
        .into_iter()
        .map(|r| {
            let reg = &map.registers[r.index];
            MapRow {
                name: reg.name.clone(),
                group: reg.group.clone(),
                address: reg.address,
                ty: format!("{:?}", reg.ty).to_lowercase(),
                value: match &r.sentinel {
                    Some(label) => Some(format!("n/a ({label})")),
                    None => r.value.as_ref().map(|v| format_value(reg, v)),
                },
                unit: reg.unit.clone(),
                raw: r.raw.as_ref().map(format_raw),
                error: r.error,
                reference: r.reference,
            }
        })
        .collect();
    Ok((Conn::describe(&cfg), rows))
}

/// Reads every register of a map, checks the results, and writes the HTML
/// report when asked. Returns the text to print and whether any check failed.
async fn check_device(
    conn: &Conn,
    path: &str,
    report_path: Option<&str>,
    json: bool,
) -> Result<(String, bool), String> {
    let (map, cfg, results) = poll_map(conn, path, None).await?;
    let report = check::evaluate(&map, &results);
    if let Some(out) = report_path {
        let identification = match read_id(conn, 1).await {
            Ok((_, id)) => id
                .objects
                .iter()
                .map(|o| (o.name.to_string(), o.value.clone()))
                .collect(),
            Err(_) => Vec::new(),
        };
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let ctx = check::ReportContext {
            map: format!("{} {}", map.meta.vendor, map.meta.model),
            map_file: path.to_string(),
            map_revision: map.meta.revision.map(|r| {
                map.meta
                    .revised
                    .as_ref()
                    .map_or(r.to_string(), |d| format!("{r}, {d}"))
            }),
            connection: Conn::describe(&cfg),
            unit_id: cfg.unit_id,
            identification,
            when: check::utc_text(secs),
        };
        std::fs::write(out, check::html(&report, &ctx)).map_err(|e| format!("{out}: {e}"))?;
    }
    let text = if json {
        serde_json::to_string_pretty(&report).unwrap()
    } else {
        check_text(&Conn::describe(&cfg), path, &report)
    };
    Ok((text, report.failed()))
}

fn check_text(where_: &str, path: &str, report: &check::CheckReport) -> String {
    use check::Status;
    let mut out = format!("{where_}  {path}\n");
    for f in &report.findings {
        let mark = match f.status {
            Status::Pass => "pass",
            Status::Fail => "FAIL",
            Status::NotRead => "skip",
        };
        out.push_str(&format!("  {mark}  {}: {}\n", f.what, f.detail));
    }
    for r in report.rows.iter().filter(|r| r.status == Status::Fail) {
        out.push_str(&format!(
            "  FAIL  {} (address {}): {}\n",
            r.name,
            r.address,
            r.reason.as_deref().unwrap_or("")
        ));
    }
    out.push_str(&format!(
        "{} passed, {} failed, {} not read\n",
        report.pass, report.fail, report.not_read
    ));
    out
}

fn map_text(where_: &str, path: &str, rows: &[MapRow]) -> String {
    let name_w = rows.iter().map(|r| r.name.len()).max().unwrap_or(4).max(4);
    let mut out = format!(
        "{} via {where_}\n",
        Path::new(path)
            .file_name()
            .map_or(path.into(), |f| f.to_string_lossy())
    );
    for r in rows {
        let value = match (&r.value, &r.error) {
            (_, Some(e)) => format!("ERROR {e}"),
            (Some(v), None) if v.starts_with("n/a") => v.clone(),
            (Some(v), None) => format!("{v} {}", r.unit).trim_end().to_string(),
            (None, None) => "-".into(),
        };
        let mark = match r.reference {
            Some("match") => "  [reference ok]",
            Some(_) => "  [reference MISMATCH]",
            None => "",
        };
        out.push_str(&format!(
            "{:>5}  {:<name_w$}  {:<24}  {}{mark}\n",
            r.address,
            r.name,
            value,
            r.raw.as_deref().unwrap_or("")
        ));
    }
    let matched = rows.iter().filter(|r| r.reference == Some("match")).count();
    let differ = rows
        .iter()
        .filter(|r| r.reference == Some("mismatch"))
        .count();
    if matched + differ > 0 {
        out.push_str(&format!(
            "Reference checks: {matched} match, {differ} differ\n"
        ));
    }
    out
}

fn validate_text(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let parsed = mbmap::parse(&bytes);
    let mut out = String::new();
    for d in &parsed.diagnostics {
        let sev = if d.severity == Severity::Error {
            "error"
        } else {
            "warning"
        };
        out.push_str(&format!(
            "{sev}: {}: {} [{}]\n",
            d.location(),
            d.message,
            d.code
        ));
    }
    if parsed.version == Some(1) {
        out.push_str(&format!(
            "note: {} (modbench-cli convert {path})\n",
            mbmap::V1_DEPRECATED
        ));
    }
    match &parsed.map {
        Some(m) => out.push_str(&format!(
            "OK: {} {}, {} registers\n",
            m.meta.vendor,
            m.meta.model,
            m.registers.len()
        )),
        None => out.push_str("The file has errors.\n"),
    }
    Ok(out)
}

fn ports_text() -> Result<String, String> {
    let ports = modbus::list_serial_ports().map_err(|e| e.to_string())?;
    Ok(if ports.is_empty() {
        "No serial ports found.\n".into()
    } else {
        ports.join("\n") + "\n"
    })
}

// ---- MCP server (JSON-RPC 2.0 over stdio, one message per line)

fn conn_schema() -> Json {
    json!({
        "tcp": {"type": "string", "description": "Modbus TCP device, host[:port]"},
        "rtu_over_tcp": {"type": "string", "description": "RTU over TCP gateway, host[:port]"},
        "udp": {"type": "string", "description": "Modbus UDP device, host[:port]"},
        "rtu_over_udp": {"type": "string", "description": "RTU over UDP gateway, host[:port]"},
        "rtu": {"type": "string", "description": "Serial port for Modbus RTU, e.g. COM3"},
        "ascii": {"type": "string", "description": "Serial port for Modbus ASCII"},
        "baud": {"type": "integer"},
        "parity": {"type": "string", "enum": ["N", "E", "O"]},
        "data_bits": {"type": "integer"},
        "stop_bits": {"type": "integer"},
        "unit": {"type": "integer", "description": "Unit / slave ID"},
        "timeout": {"type": "integer", "description": "Response timeout, ms (default 1000)"},
        "delay": {"type": "integer", "description": "Pause between requests, ms (default 50)"},
        "max_regs": {"type": "integer", "description": "Largest number of registers per read request; longer reads are split"}
    })
}

fn tools() -> Json {
    let mut read_props = conn_schema();
    read_props["area"] =
        json!({"type": "string", "enum": ["coil", "discrete", "input", "holding"]});
    read_props["address"] = json!({"type": "integer", "minimum": 0, "maximum": 65535});
    read_props["count"] = json!({"type": "integer", "minimum": 1, "maximum": 125});
    let mut id_props = conn_schema();
    id_props["level"] = json!({"type": "integer", "enum": [1, 2, 3], "description": "1 basic (default), 2 regular, 3 extended"});
    let mut map_props = conn_schema();
    map_props["map_path"] = json!({"type": "string", "description": "Path to a .mbmap.json (or version 1 .mbmap) file"});
    map_props["filter"] = json!({"type": "string", "description": "Only registers whose name or group contains this text"});
    json!([
        {
            "name": "list_serial_ports",
            "description": "List the serial ports on this machine.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "read_registers",
            "description": "Read Modbus registers or bits once (read-only). Returns address, hex, unsigned and signed values.",
            "inputSchema": {"type": "object", "properties": read_props, "required": ["area", "address"]}
        },
        {
            "name": "read_map",
            "description": "Read every register of a .mbmap device map once (read-only) and return decoded, scaled values. Connection settings not given come from the map.",
            "inputSchema": {"type": "object", "properties": map_props, "required": ["map_path"]}
        },
        {
            "name": "read_device_id",
            "description": "Ask a device to identify itself (FC43 Read Device Identification): vendor, product code, revision and more. Read-only; many devices do not support it.",
            "inputSchema": {"type": "object", "properties": id_props}
        },
        {
            "name": "validate_map",
            "description": "Check a device map file (.mbmap.json, or a deprecated version 1 .mbmap) against the format spec.",
            "inputSchema": {"type": "object", "properties": {"map_path": {"type": "string"}}, "required": ["map_path"]}
        }
    ])
}

/// Tools that act on the Modbench window the operator is looking at, through
/// the GUI control channel (off unless the operator turned it on in Settings;
/// see `modbench_lib::gui_control`). They never write to a device.
fn gui_tools() -> Json {
    let reg_list = json!({"type": "array", "items": {"type": ["string", "integer"]},
        "description": "Register names, or indexes as get_state lists them"});
    json!([
        {
            "name": "gui_get_state",
            "description": "The running Modbench window's state as JSON: connection, open maps, the active map's registers and latest values, read settings, the screen shown, and whether the operator paused assistant control. Read it before changing anything.",
            "inputSchema": {"type": "object", "properties": {}}
        },
        {
            "name": "gui_set_connection",
            "description": "Change the connection settings in the window (not while a test runs). Only the fields given change.",
            "inputSchema": {"type": "object", "properties": {
                "transport": {"type": "string", "enum": ["tcp", "rtuovertcp", "udp", "rtuoverudp", "rtu"]},
                "host": {"type": "string"},
                "port": {"type": "integer", "description": "TCP/UDP port"},
                "serial_port": {"type": "string"},
                "framing": {"type": "string", "enum": ["rtu", "ascii"], "description": "On a serial port"},
                "baud": {"type": "integer"},
                "parity": {"type": "string", "enum": ["N", "E", "O"]},
                "data_bits": {"type": "integer"},
                "stop_bits": {"type": "integer"},
                "unit": {"type": "integer", "description": "Unit / slave ID, 0-255"},
                "timeout_ms": {"type": "integer"}
            }}
        },
        {
            "name": "gui_list_library",
            "description": "The device maps in the operator's library (path, vendor, model).",
            "inputSchema": {"type": "object", "properties": {"filter": {"type": "string", "description": "Only maps whose vendor or model contains this text"}}}
        },
        {
            "name": "gui_open_map",
            "description": "Open a device map in the window and make it the active one; the connection settings come from the map (or what was last used with it).",
            "inputSchema": {"type": "object", "properties": {"path": {"type": "string", "description": "A path from gui_list_library"}}, "required": ["path"]}
        },
        {
            "name": "gui_set_map_view",
            "description": "Change how the active map is read and shown: the address notation, the byte order, and which registers are read.",
            "inputSchema": {"type": "object", "properties": {
                "address_format": {"type": "string", "enum": ["base0", "base1", "hex", "modicon", "iec"]},
                "byte_order": {"type": "string", "enum": ["map", "ABCD", "DCBA", "BADC", "CDAB"], "description": "\"map\" uses the map's own"},
                "read_registers": reg_list,
                "skip_registers": reg_list,
                "read_all": {"type": "boolean", "description": "Turn reading of every register on (true) or off (false)"}
            }}
        },
        {
            "name": "gui_set_manual_read",
            "description": "Set up a manual read (no map) in the window: function code 1-4, address and quantity. Reads only.",
            "inputSchema": {"type": "object", "properties": {
                "fc": {"type": "integer", "enum": [1, 2, 3, 4]},
                "address": {"type": "integer", "minimum": 0, "maximum": 65535},
                "quantity": {"type": "integer", "minimum": 1, "maximum": 2000}
            }}
        },
        {
            "name": "gui_run",
            "description": "Start or stop reading in the window with its current settings (the active map, or the manual read). Loop and its interval can be set.",
            "inputSchema": {"type": "object", "properties": {
                "action": {"type": "string", "enum": ["start", "stop"]},
                "loop": {"type": "boolean"},
                "interval_ms": {"type": "integer", "minimum": 0, "description": "Time from the start of one cycle to the next; 0 as fast as the delay allows"}
            }, "required": ["action"]}
        },
        {
            "name": "gui_chart",
            "description": "Choose the registers of the active map shown on the Chart tab.",
            "inputSchema": {"type": "object", "properties": {
                "action": {"type": "string", "enum": ["add", "remove", "clear"]},
                "registers": reg_list
            }, "required": ["action"]}
        },
        {
            "name": "gui_navigate",
            "description": "Switch the screen shown in the window.",
            "inputSchema": {"type": "object", "properties": {
                "page": {"type": "string", "enum": ["main", "tools", "library", "settings"]},
                "tab": {"type": "string", "enum": ["registers", "map", "chart", "write", "raw", "log"]},
                "tool": {"type": "string", "enum": ["simulator", "identify", "addresses", "network", "request", "decoder", "frames", "stats", "alarms", "player"]}
            }}
        }
    ])
}

async fn call_tool(name: &str, args: &Json) -> Result<String, String> {
    let conn: Conn = serde_json::from_value(args.clone()).unwrap_or_default();
    let str_arg = |k: &str| args.get(k).and_then(Json::as_str).map(str::to_string);
    match name {
        "list_serial_ports" => ports_text(),
        "read_registers" => {
            let area = parse_area(&str_arg("area").ok_or("area is required")?)?;
            let address = args
                .get("address")
                .and_then(Json::as_u64)
                .filter(|&a| a <= 65535)
                .ok_or("address must be 0-65535")? as u16;
            let count = args
                .get("count")
                .and_then(Json::as_u64)
                .unwrap_or(1)
                .clamp(1, 2000) as u16;
            let (where_, data) = read_registers(&conn, area, address, count).await?;
            Ok(raw_text(&where_, area, address, &data))
        }
        "read_map" => {
            let path = str_arg("map_path").ok_or("map_path is required")?;
            let (where_, rows) = read_map(&conn, &path, str_arg("filter").as_deref()).await?;
            Ok(map_text(&where_, &path, &rows))
        }
        "read_device_id" => {
            let level = args.get("level").and_then(Json::as_u64).unwrap_or(1) as u8;
            let (where_, id) = read_id(&conn, level).await?;
            Ok(id_text(&where_, &id))
        }
        "validate_map" => validate_text(&str_arg("map_path").ok_or("map_path is required")?),
        // The running window's; without one the read-only tools above still work.
        _ if name.starts_with("gui_") => {
            let result = modbench_lib::gui_control::call(&name["gui_".len()..], args).await?;
            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

async fn handle(msg: &Json) -> Option<Json> {
    let id = msg.get("id")?.clone(); // notifications get no answer
    let method = msg.get("method").and_then(Json::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Json::Null);
    let result = match method {
        "initialize" => json!({
            "protocolVersion": params.get("protocolVersion").cloned().unwrap_or(json!("2025-06-18")),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "modbench", "version": env!("CARGO_PKG_VERSION")}
        }),
        "ping" => json!({}),
        "tools/list" => {
            let mut all = tools();
            all.as_array_mut()
                .unwrap()
                .extend(gui_tools().as_array().unwrap().iter().cloned());
            json!({"tools": all})
        }
        "tools/call" => {
            let name = params.get("name").and_then(Json::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(name, &args).await {
                Ok(text) => json!({"content": [{"type": "text", "text": text}]}),
                Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
            }
        }
        _ => {
            return Some(json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": -32601, "message": format!("method not found: {method}")}
            }));
        }
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

async fn serve_mcp() {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Json>(&line) {
            Ok(msg) => handle(&msg).await,
            Err(e) => Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": {"code": -32700, "message": format!("parse error: {e}")}
            })),
        };
        if let Some(reply) = reply {
            let _ = stdout.write_all(format!("{reply}\n").as_bytes()).await;
            let _ = stdout.flush().await;
        }
    }
}

async fn read_id(conn: &Conn, level: u8) -> Result<(String, modbus::DeviceIdentity), String> {
    let cfg = conn.config(None)?;
    let mut session = Session::connect(&cfg, None)
        .await
        .map_err(|e| format!("could not connect ({}): {e}", Conn::describe(&cfg)))?;
    let id = session
        .read_device_id(level)
        .await
        .map_err(|e| e.to_string());
    session.disconnect().await;
    Ok((Conn::describe(&cfg), id?))
}

fn id_text(where_: &str, id: &modbus::DeviceIdentity) -> String {
    let mut out = format!("{where_} (conformity 0x{:02X})\n", id.conformity);
    for o in &id.objects {
        out.push_str(&format!("  0x{:02X}  {:<22}  {}\n", o.id, o.name, o.value));
    }
    out
}

async fn simulate(
    map: Option<String>,
    listen: Transport,
    strict: bool,
    unit: Option<u8>,
    generators: Option<String>,
) -> Result<(), String> {
    use modbench_lib::generators::{self, Generator};
    use modbench_lib::simulator::{self, Memory, SimLogger};
    let memory = match &map {
        Some(path) => Memory::from_map(&load_map(path)?, strict),
        None => Memory::default(),
    };
    let generators = match &generators {
        Some(path) => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
            let list: Vec<Generator> =
                serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
            let problems: Vec<_> = list
                .iter()
                .enumerate()
                .filter_map(|(i, g)| {
                    generators::problem(&memory, g)
                        .map(|p| format!("{path}: generator {}: {p}", i + 1))
                })
                .collect();
            if !problems.is_empty() {
                return Err(problems.join(
                    "
",
                ));
            }
            Some(list)
        }
        None => None,
    };
    let count = generators.as_ref().map_or(0, Vec::len);
    let logger: SimLogger = std::sync::Arc::new(|e| {
        let what = match (e.address, e.count) {
            (Some(a), Some(n)) => format!(" {a} x{n}"),
            _ => String::new(),
        };
        let result = e.error.unwrap_or_else(|| "ok".into());
        println!(
            "{}  unit {:>3}  FC{:02}{what}  {result}",
            e.peer, e.unit, e.function
        );
    });
    let device = simulator::Device::new(unit, memory);
    *device.generators.lock().unwrap() = generators.unwrap_or_default();
    let devices = std::sync::Arc::new(std::sync::Mutex::new(vec![device]));
    let running = simulator::start(listen.clone(), devices, Some(logger))
        .await
        .map_err(|e| format!("{}: {e}", where_listening(&listen)))?;
    let changing = match count {
        0 => String::new(),
        1 => ", 1 generator".into(),
        n => format!(", {n} generators"),
    };
    println!(
        "Simulating {} on {}{changing} (Ctrl+C to stop)",
        map.as_deref().unwrap_or("an empty device"),
        where_listening(&running.listen)
    );
    // Runs until the process is stopped (Ctrl+C ends it).
    std::future::pending::<()>().await;
    running.stop();
    Ok(())
}

/// Where a simulator listens, for messages.
fn where_listening(listen: &Transport) -> String {
    match listen {
        Transport::Tcp { host, port } => format!("{host}:{port} (Modbus TCP)"),
        Transport::RtuOverTcp { host, port } => format!("{host}:{port} (RTU over TCP)"),
        Transport::Udp { host, port } => format!("{host}:{port} (Modbus UDP)"),
        Transport::RtuOverUdp { host, port } => format!("{host}:{port} (RTU over UDP)"),
        Transport::Rtu { port, .. } => format!("{port} (RTU)"),
        Transport::Ascii { port, .. } => format!("{port} (ASCII)"),
    }
}

async fn scan(cfg: &ConnectionConfig, from: u8, to: u8) -> Result<Vec<modbus::ScanHit>, String> {
    if from > to {
        return Err("--from is above --to".into());
    }
    let mut session = Session::connect(cfg, None)
        .await
        .map_err(|e| format!("could not connect ({}): {e}", Conn::describe(cfg)))?;
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let hits = session
        .scan_units(
            from,
            to,
            Duration::from_millis(cfg.timeout_ms),
            &cancel,
            |_| {},
        )
        .await;
    session.disconnect().await;
    Ok(hits)
}

// ---- CLI entry point

async fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Ports => print!("{}", ports_text()?),
        Command::Validate { map } => {
            let text = validate_text(&map)?;
            print!("{text}");
            if text.ends_with("The file has errors.\n") {
                return Err("invalid map".into());
            }
        }
        Command::Convert { map, out } => {
            let parsed = load_map(&map)?;
            let target = out.unwrap_or_else(|| mbmap::v2_file_name(&map));
            std::fs::write(&target, mbmap::to_string(&parsed))
                .map_err(|e| format!("{target}: {e}"))?;
            println!("{map} -> {target}");
        }
        Command::Read {
            conn,
            area,
            address,
            count,
            json,
        } => {
            let area = parse_area(&area)?;
            let address = parse_int(&address)?;
            let count = parse_int(&count)?.max(1);
            let (where_, data) = read_registers(&conn, area, address, count).await?;
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&raw_rows(address, &data)).unwrap()
                );
            } else {
                print!("{}", raw_text(&where_, area, address, &data));
            }
        }
        Command::Map {
            conn,
            map,
            filter,
            json,
        } => {
            let (where_, rows) = read_map(&conn, &map, filter.as_deref()).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&rows).unwrap());
            } else {
                print!("{}", map_text(&where_, &map, &rows));
            }
        }
        Command::Check {
            conn,
            map,
            report,
            json,
        } => {
            let (text, failed) = check_device(&conn, &map, report.as_deref(), json).await?;
            print!("{text}");
            if json {
                println!();
            }
            if failed {
                return Err("check failed".into());
            }
        }
        Command::Scan {
            conn,
            from,
            to,
            json,
        } => {
            let cfg = conn.config(None)?;
            let hits = scan(&cfg, from, to).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&hits).unwrap());
            } else if hits.is_empty() {
                println!(
                    "{}: no device answered on unit IDs {from}-{to}",
                    Conn::describe(&cfg)
                );
            } else {
                println!("{}", Conn::describe(&cfg));
                for h in &hits {
                    println!("  unit {:>3}  {}", h.unit, h.response);
                }
            }
        }
        Command::ScanSerial {
            rtu,
            bauds,
            parities,
            stop_bits,
            from,
            to,
            area,
            address,
            timeout,
            first,
            json,
        } => {
            let parities = parities
                .to_uppercase()
                .chars()
                .map(|c| match c {
                    'N' => Ok(Parity::N),
                    'E' => Ok(Parity::E),
                    'O' => Ok(Parity::O),
                    c => Err(format!("parity must be N, E or O, not {c}")),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let scan = modbus::SerialScan {
                port: rtu.clone(),
                baudrates: bauds,
                parities,
                stop_bits,
                data_bits: 8,
                from,
                to,
                area: parse_area(&area)?,
                address: parse_int(&address)?,
                timeout_ms: timeout,
                stop_at_first: first,
            };
            let cancel = std::sync::atomic::AtomicBool::new(false);
            let hits = modbus::scan_serial(&scan, &cancel, |p| {
                if !json {
                    eprint!(
                        "\r{} 8{:?}{} unit {:>3}  ({}/{})   ",
                        p.baudrate,
                        p.parity,
                        p.stop_bits,
                        p.unit,
                        p.done + 1,
                        p.total
                    );
                }
            })
            .await
            .map_err(|e| format!("{rtu}: {e}"))?;
            if !json {
                eprintln!();
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&hits).unwrap());
            } else if hits.is_empty() {
                println!("{rtu}: no setting got an answer");
            } else {
                for h in &hits {
                    println!(
                        "{rtu} {} {}{:?}{}  unit {:>3}  {}",
                        h.baudrate, h.data_bits, h.parity, h.stop_bits, h.unit, h.response
                    );
                }
            }
        }
        Command::Id { conn, level, json } => {
            let (where_, id) = read_id(&conn, level).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&id).unwrap());
            } else {
                print!("{}", id_text(&where_, &id));
            }
        }
        Command::Simulate {
            map,
            port,
            rtu_over_tcp,
            udp,
            rtu_over_udp,
            rtu,
            ascii,
            baud,
            parity,
            data_bits,
            stop_bits,
            strict,
            unit,
            public,
            generators,
        } => {
            let host = if public { "0.0.0.0" } else { "127.0.0.1" }.to_string();
            // The serial settings are read like a client's (see `Conn`).
            let serial = Conn {
                rtu,
                ascii,
                baud,
                parity,
                data_bits,
                stop_bits,
                ..Conn::default()
            };
            let listen = if serial.rtu.is_some() || serial.ascii.is_some() {
                let map = map.as_deref().map(load_map).transpose()?;
                serial.config(map.as_ref())?.transport
            } else if rtu_over_tcp {
                Transport::RtuOverTcp { host, port }
            } else if udp {
                Transport::Udp { host, port }
            } else if rtu_over_udp {
                Transport::RtuOverUdp { host, port }
            } else {
                Transport::Tcp { host, port }
            };
            simulate(map, listen, strict, unit, generators).await?
        }
        Command::Mcp => serve_mcp().await,
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli).await {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
    // Give a serial port a moment to close cleanly before exiting.
    tokio::time::sleep(Duration::from_millis(10)).await;
}
