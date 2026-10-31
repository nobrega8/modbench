//! A device map written as the configuration of other software: Home
//! Assistant's Modbus integration, ESPHome's `modbus_controller`, Telegraf's
//! `inputs.modbus`, or a CSV tag list for PLC and SCADA tools.
//!
//! Registers a format cannot express (a byte order it lacks, bits inside a
//! register, SunSpec scale factors…) are left out and listed in a comment at
//! the top, never dropped silently. Write-only registers are left out too:
//! every format here reads.

use serde::{Deserialize, Serialize};

use crate::codec::{ByteOrder, DataType};
use crate::mbmap::{DeviceMap, Register};
use crate::modbus::{ConnectionConfig, Parity, Transport};
use crate::planner::Area;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    /// Home Assistant `modbus:` (configuration.yaml)
    HomeAssistant,
    /// ESPHome `modbus_controller` sensors
    Esphome,
    /// Telegraf `inputs.modbus`, request style
    Telegraf,
    /// One row per register, for PLC and SCADA tag imports
    Csv,
}

impl Format {
    /// The file name the export is usually saved as.
    pub fn file_name(self, map: &DeviceMap) -> String {
        let slug = slug(&format!("{} {}", map.meta.vendor, map.meta.model), '-');
        match self {
            Format::HomeAssistant => format!("{slug}-home-assistant.yaml"),
            Format::Esphome => format!("{slug}-esphome.yaml"),
            Format::Telegraf => format!("{slug}-telegraf.conf"),
            Format::Csv => format!("{slug}-tags.csv"),
        }
    }
}

/// Where the device is and how often to read it. Without a connection, the
/// map's own defaults are used and the host or serial port is a placeholder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Options {
    #[serde(default)]
    pub connection: Option<ConnectionConfig>,
    /// Unit ID, over the connection's and the map's.
    #[serde(default)]
    pub unit_id: Option<u8>,
    /// Seconds between reads (default 10).
    #[serde(default)]
    pub interval_s: Option<u32>,
}

const HOST: &str = "<host>";
const SERIAL: &str = "<serial-port>";

pub fn export(map: &DeviceMap, format: Format, options: &Options) -> String {
    let target = Target::new(map, options);
    match format {
        Format::HomeAssistant => home_assistant(map, &target),
        Format::Esphome => esphome(map, &target),
        Format::Telegraf => telegraf(map, &target),
        Format::Csv => csv(map),
    }
}

/// The connection an export is written for.
struct Target {
    transport: Transport,
    unit_id: u8,
    interval_s: u32,
}

impl Target {
    fn new(map: &DeviceMap, options: &Options) -> Target {
        let meta = &map.meta;
        let transport = match &options.connection {
            Some(c) => c.transport.clone(),
            None if meta.protocol == Some(crate::mbmap::Protocol::Rtu) => Transport::Rtu {
                port: SERIAL.into(),
                baudrate: meta.baudrate(),
                parity: meta.parity(),
                data_bits: meta.data_bits(),
                stop_bits: meta.stop_bits(),
            },
            None => Transport::Tcp {
                host: HOST.into(),
                port: meta.port(),
            },
        };
        Target {
            transport,
            unit_id: options.unit_id.unwrap_or(
                options
                    .connection
                    .as_ref()
                    .map_or(meta.unit_id(), |c| c.unit_id),
            ),
            interval_s: options.interval_s.unwrap_or(10).max(1),
        }
    }

    /// The serial settings, from the connection or the map's defaults.
    fn serial(&self, map: &DeviceMap) -> (String, u32, Parity, u8, u8) {
        match &self.transport {
            Transport::Rtu {
                port,
                baudrate,
                parity,
                data_bits,
                stop_bits,
            }
            | Transport::Ascii {
                port,
                baudrate,
                parity,
                data_bits,
                stop_bits,
            } => (port.clone(), *baudrate, *parity, *data_bits, *stop_bits),
            _ => {
                let m = &map.meta;
                (
                    SERIAL.into(),
                    m.baudrate(),
                    m.parity(),
                    m.data_bits(),
                    m.stop_bits(),
                )
            }
        }
    }
}

/// Registers a format leaves out, with the reason, for the comment on top.
#[derive(Default)]
struct Skipped(Vec<String>);

impl Skipped {
    fn add(&mut self, r: &Register, why: &str) {
        self.0.push(format!("{} ({}): {why}", r.name, r.address));
    }

    /// Reasons every format shares: nothing to read, or a value that needs
    /// another register to make sense.
    fn common(&mut self, r: &Register) -> bool {
        if !r.access.readable() {
            self.add(r, "write-only");
        } else if r.scale_factor.is_some() {
            self.add(r, "scale factor from another register");
        } else {
            return false;
        }
        true
    }

    fn comment(&self, prefix: &str) -> String {
        if self.0.is_empty() {
            return String::new();
        }
        let mut out = format!("{prefix} Not exported:\n");
        for s in &self.0 {
            out.push_str(&format!("{prefix}   {s}\n"));
        }
        out
    }
}

/// Lowercase words joined by `sep`.
fn slug(text: &str, sep: char) -> String {
    let mut out = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with(sep) {
            out.push(sep);
        }
    }
    out.trim_end_matches(sep).to_string()
}

/// A double-quoted string, valid in YAML and TOML alike.
fn quote(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".into())
}

/// A number without a trailing `.0`.
fn num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// Decimals worth showing for a scaled value.
fn decimals(r: &Register) -> usize {
    let from_scale = |x: f64| {
        let s = num(x);
        s.split_once('.').map_or(0, |(_, d)| d.len())
    };
    let d = from_scale(r.scale).max(from_scale(r.offset));
    match r.ty {
        DataType::Float32 | DataType::Float64 => d.max(2),
        _ => d,
    }
}

fn is_wide(ty: DataType) -> bool {
    !matches!(ty, DataType::Int16 | DataType::Uint16 | DataType::Bool)
}

/// A bool or bit field inside a word register.
fn in_word(r: &Register) -> bool {
    !r.area.is_bit() && (r.bit.is_some() || r.bits.is_some())
}

fn header(prefix: &str, what: &str, map: &DeviceMap, skipped: &Skipped) -> String {
    let out = format!(
        "{prefix} {what} for {} {}, made by Modbench from its device map.\n",
        map.meta.vendor, map.meta.model
    );
    out + &skipped.comment(prefix)
}

// ---- Home Assistant

fn home_assistant(map: &DeviceMap, target: &Target) -> String {
    let mut skipped = Skipped::default();
    let mut sensors = String::new();
    let mut binary = String::new();
    let device = slug(&format!("{} {}", map.meta.vendor, map.meta.model), '_');
    for r in &map.registers {
        if skipped.common(r) {
            continue;
        }
        if in_word(r) {
            skipped.add(r, "bits inside a register");
            continue;
        }
        let id = format!("{device}_{}_{}", slug(&r.name, '_'), r.address);
        let common = format!(
            "      - name: {}\n        unique_id: {id}\n        slave: {}\n        address: {}\n        scan_interval: {}\n",
            quote(&r.name),
            target.unit_id,
            r.address,
            target.interval_s
        );
        if r.area.is_bit() {
            let input = if r.area == Area::Coil {
                "coil"
            } else {
                "discrete_input"
            };
            binary.push_str(&format!("{common}        input_type: {input}\n"));
            continue;
        }
        let input = if r.area == Area::Input {
            "input"
        } else {
            "holding"
        };
        let ty = match r.ty {
            DataType::Int16 => "int16",
            DataType::Uint16 => "uint16",
            DataType::Int32 => "int32",
            DataType::Uint32 => "uint32",
            DataType::Float32 => "float32",
            DataType::Int64 => "int64",
            DataType::Uint64 => "uint64",
            DataType::Float64 => "float64",
            DataType::String => "string",
            DataType::Bool => unreachable!("bools are bit areas or bits in a word"),
        };
        let order = r.byte_order(&map.meta);
        let swap = match (order, is_wide(r.ty)) {
            (ByteOrder::ABCD, _) | (ByteOrder::CDAB, false) => "none",
            (ByteOrder::BADC, _) | (ByteOrder::DCBA, false) => "byte",
            (ByteOrder::CDAB, true) => "word",
            (ByteOrder::DCBA, true) => "word_byte",
        };
        let mut s = format!("{common}        input_type: {input}\n        data_type: {ty}\n");
        if r.ty == DataType::String {
            s.push_str(&format!("        count: {}\n", r.count()));
        } else {
            if swap != "none" {
                s.push_str(&format!("        swap: {swap}\n"));
            }
            if r.scale != 1.0 {
                s.push_str(&format!("        scale: {}\n", num(r.scale)));
            }
            if r.offset != 0.0 {
                s.push_str(&format!("        offset: {}\n", num(r.offset)));
            }
            s.push_str(&format!("        precision: {}\n", decimals(r)));
            if !r.unit.is_empty() {
                s.push_str(&format!(
                    "        unit_of_measurement: {}\n",
                    quote(&r.unit)
                ));
            }
        }
        if !r.enum_labels.is_empty() {
            s.push_str("        # State labels (not applied): ");
            let labels: Vec<_> = r
                .enum_labels
                .iter()
                .map(|(v, l)| format!("{v}={l}"))
                .collect();
            s.push_str(&labels.join(", "));
            s.push('\n');
        }
        sensors.push_str(&s);
    }

    let mut hub = format!(
        "  - name: {}\n",
        quote(&format!("{} {}", map.meta.vendor, map.meta.model))
    );
    match &target.transport {
        Transport::Tcp { host, port }
        | Transport::RtuOverTcp { host, port }
        | Transport::Udp { host, port }
        | Transport::RtuOverUdp { host, port } => {
            let ty = match target.transport {
                Transport::Tcp { .. } => "tcp",
                Transport::RtuOverTcp { .. } => "rtuovertcp",
                _ => "udp",
            };
            if matches!(target.transport, Transport::RtuOverUdp { .. }) {
                hub.push_str(
                    "    # Home Assistant has no RTU over UDP: Modbus UDP is used instead\n",
                );
            }
            hub.push_str(&format!(
                "    type: {ty}\n    host: {}\n    port: {port}\n",
                quote(host)
            ));
        }
        Transport::Rtu { .. } | Transport::Ascii { .. } => {
            let (port, baud, parity, bits, stop) = target.serial(map);
            let method = if matches!(target.transport, Transport::Ascii { .. }) {
                "ascii"
            } else {
                "rtu"
            };
            hub.push_str(&format!(
                "    type: serial\n    method: {method}\n    port: {}\n    baudrate: {baud}\n    bytesize: {bits}\n    parity: {}\n    stopbits: {stop}\n",
                quote(&port),
                parity_letter(parity)
            ));
        }
    }
    if !sensors.is_empty() {
        hub.push_str("    sensors:\n");
        hub.push_str(&sensors);
    }
    if !binary.is_empty() {
        hub.push_str("    binary_sensors:\n");
        hub.push_str(&binary);
    }
    let mut out = header(
        "#",
        "Home Assistant Modbus integration (configuration.yaml)",
        map,
        &skipped,
    );
    if hub.contains(HOST) || hub.contains(SERIAL) {
        out.push_str(&format!(
            "# Replace {HOST} or {SERIAL} with the device's address or port.\n"
        ));
    }
    out + "modbus:\n" + &hub
}

fn parity_letter(p: Parity) -> &'static str {
    match p {
        Parity::N => "N",
        Parity::E => "E",
        Parity::O => "O",
    }
}

// ---- ESPHome

fn esphome(map: &DeviceMap, target: &Target) -> String {
    let mut skipped = Skipped::default();
    let mut sensors = String::new();
    let mut binary = String::new();
    let mut text = String::new();
    for r in &map.registers {
        if skipped.common(r) {
            continue;
        }
        let register_type = match r.area {
            Area::Coil => "coil",
            Area::Discrete => "discrete_input",
            Area::Input => "read",
            Area::Holding => "holding",
        };
        let common = format!(
            "  - platform: modbus_controller\n    modbus_controller_id: device\n    name: {}\n    register_type: {register_type}\n    address: {}\n",
            quote(&r.name),
            r.address
        );
        if r.area.is_bit() {
            binary.push_str(&common);
            continue;
        }
        if r.ty == DataType::Bool {
            let bit = r.bit.unwrap_or(0);
            binary.push_str(&format!("{common}    bitmask: {:#06x}\n", 1u32 << bit));
            continue;
        }
        if r.ty == DataType::String {
            let n = r.count();
            text.push_str(&format!(
                "{common}    register_count: {n}\n    response_size: {}\n    raw_encode: NONE\n",
                n * 2
            ));
            continue;
        }
        let order = r.byte_order(&map.meta);
        let swapped = matches!(order, ByteOrder::CDAB);
        if matches!(order, ByteOrder::BADC | ByteOrder::DCBA) {
            skipped.add(r, "byte order with swapped bytes");
            continue;
        }
        let value_type = match (r.ty, swapped && is_wide(r.ty)) {
            (DataType::Uint16, _) => "U_WORD",
            (DataType::Int16, _) => "S_WORD",
            (DataType::Uint32, false) => "U_DWORD",
            (DataType::Uint32, true) => "U_DWORD_R",
            (DataType::Int32, false) => "S_DWORD",
            (DataType::Int32, true) => "S_DWORD_R",
            (DataType::Float32, false) => "FP32",
            (DataType::Float32, true) => "FP32_R",
            (DataType::Uint64, false) => "U_QWORD",
            (DataType::Uint64, true) => "U_QWORD_R",
            (DataType::Int64, false) => "S_QWORD",
            (DataType::Int64, true) => "S_QWORD_R",
            _ => {
                skipped.add(r, "64-bit float");
                continue;
            }
        };
        let mut s = format!("{common}    value_type: {value_type}\n");
        if let Some(bits) = r.bits {
            let mask = ((1u32 << bits) - 1) << r.bit.unwrap_or(0);
            s.push_str(&format!("    bitmask: {mask:#06x}\n"));
        }
        if !r.unit.is_empty() {
            s.push_str(&format!("    unit_of_measurement: {}\n", quote(&r.unit)));
        }
        s.push_str(&format!("    accuracy_decimals: {}\n", decimals(r)));
        if r.scale != 1.0 || r.offset != 0.0 {
            s.push_str("    filters:\n");
            if r.scale != 1.0 {
                s.push_str(&format!("      - multiply: {}\n", num(r.scale)));
            }
            if r.offset != 0.0 {
                s.push_str(&format!("      - offset: {}\n", num(r.offset)));
            }
        }
        sensors.push_str(&s);
    }

    let (_, baud, parity, bits, stop) = target.serial(map);
    let parity = match parity {
        Parity::N => "NONE",
        Parity::E => "EVEN",
        Parity::O => "ODD",
    };
    let mut out = header("#", "ESPHome modbus_controller", map, &skipped);
    if !matches!(target.transport, Transport::Rtu { .. }) {
        out.push_str(
            "# ESPHome talks Modbus RTU on a UART: reach a TCP device through a gateway.\n",
        );
    }
    out.push_str(&format!(
        "# Set the UART pins, and flow_control_pin if the RS-485 transceiver needs one.\n\
uart:\n  id: modbus_uart\n  tx_pin: GPIO17\n  rx_pin: GPIO16\n  baud_rate: {baud}\n  data_bits: {bits}\n  parity: {parity}\n  stop_bits: {stop}\n\n\
modbus:\n  id: modbus_bus\n  uart_id: modbus_uart\n\n\
modbus_controller:\n  - id: device\n    address: {:#04x}\n    modbus_id: modbus_bus\n    update_interval: {}s\n",
        target.unit_id, target.interval_s
    ));
    for (name, list) in [
        ("sensor", sensors),
        ("binary_sensor", binary),
        ("text_sensor", text),
    ] {
        if !list.is_empty() {
            out.push_str(&format!("\n{name}:\n{list}"));
        }
    }
    out
}

// ---- Telegraf

fn telegraf(map: &DeviceMap, target: &Target) -> String {
    let mut skipped = Skipped::default();
    // One request per area and byte order, in the map's order.
    let mut requests: Vec<((Area, ByteOrder), Vec<String>)> = Vec::new();
    let mut names = std::collections::HashSet::new();
    for r in &map.registers {
        if skipped.common(r) {
            continue;
        }
        if in_word(r) {
            skipped.add(r, "bits inside a register");
            continue;
        }
        if r.offset != 0.0 {
            skipped.add(r, "offset");
            continue;
        }
        // Field names must be unique in the input.
        let mut name = slug(&r.name, '_');
        if name.is_empty() || !names.insert(name.clone()) {
            name = format!("{name}_{}", r.address);
            names.insert(name.clone());
        }
        let field = if r.area.is_bit() {
            format!("{{ address = {}, name = {} }}", r.address, quote(&name))
        } else {
            let ty = match r.ty {
                DataType::Int16 => "INT16",
                DataType::Uint16 => "UINT16",
                DataType::Int32 => "INT32",
                DataType::Uint32 => "UINT32",
                DataType::Float32 => "FLOAT32",
                DataType::Int64 => "INT64",
                DataType::Uint64 => "UINT64",
                DataType::Float64 => "FLOAT64",
                DataType::String => "STRING",
                DataType::Bool => unreachable!("bools are bit areas or bits in a word"),
            };
            let mut f = format!(
                "{{ address = {}, name = {}, type = \"{ty}\"",
                r.address,
                quote(&name)
            );
            if r.ty == DataType::String {
                f.push_str(&format!(", length = {}", r.count()));
            } else if r.scale != 1.0 {
                f.push_str(&format!(", scale = {}, output = \"FLOAT64\"", num(r.scale)));
            }
            f + " }"
        };
        let key = (
            r.area,
            if r.area.is_bit() {
                ByteOrder::ABCD
            } else {
                r.byte_order(&map.meta)
            },
        );
        match requests.iter_mut().find(|(k, _)| *k == key) {
            Some((_, fields)) => fields.push(field),
            None => requests.push((key, vec![field])),
        }
    }

    let mut out = header("#", "Telegraf inputs.modbus (request style)", map, &skipped);
    out.push_str(&format!(
        "[[inputs.modbus]]\n  name = {}\n  interval = \"{}s\"\n  timeout = \"1s\"\n",
        quote(&format!("{} {}", map.meta.vendor, map.meta.model)),
        target.interval_s
    ));
    match &target.transport {
        Transport::Tcp { host, port }
        | Transport::RtuOverTcp { host, port }
        | Transport::Udp { host, port }
        | Transport::RtuOverUdp { host, port } => {
            let mode = match target.transport {
                Transport::RtuOverTcp { .. } | Transport::RtuOverUdp { .. } => "RTUoverTCP",
                _ => "TCP",
            };
            if matches!(
                target.transport,
                Transport::Udp { .. } | Transport::RtuOverUdp { .. }
            ) {
                out.push_str("  # Telegraf has no Modbus over UDP: TCP is used instead\n");
            }
            if host == HOST {
                out.push_str(&format!("  # Replace {HOST} with the device's address\n"));
            }
            out.push_str(&format!(
                "  controller = \"tcp://{host}:{port}\"\n  transmission_mode = \"{mode}\"\n"
            ));
        }
        Transport::Rtu { .. } | Transport::Ascii { .. } => {
            let (port, baud, parity, bits, stop) = target.serial(map);
            let mode = if matches!(target.transport, Transport::Ascii { .. }) {
                "ASCII"
            } else {
                "RTU"
            };
            if port == SERIAL {
                out.push_str(&format!(
                    "  # Replace {SERIAL} with the serial port, e.g. /dev/ttyUSB0\n"
                ));
            }
            out.push_str(&format!(
                "  controller = \"file://{port}\"\n  transmission_mode = \"{mode}\"\n  baud_rate = {baud}\n  data_bits = {bits}\n  parity = \"{}\"\n  stop_bits = {stop}\n",
                parity_letter(parity)
            ));
        }
    }
    out.push_str("  configuration_type = \"request\"\n");
    for ((area, order), fields) in requests {
        let register = match area {
            Area::Coil => "coil",
            Area::Discrete => "discrete",
            Area::Input => "input",
            Area::Holding => "holding",
        };
        out.push_str(&format!(
            "\n  [[inputs.modbus.request]]\n    slave_id = {}\n    register = \"{register}\"\n",
            target.unit_id
        ));
        if !area.is_bit() {
            out.push_str(&format!("    byte_order = \"{order:?}\"\n"));
        }
        out.push_str("    fields = [\n");
        for f in fields {
            out.push_str(&format!("      {f},\n"));
        }
        out.push_str("    ]\n");
    }
    out
}

// ---- CSV tag list

fn csv(map: &DeviceMap) -> String {
    let cell = |s: &str| {
        if s.contains([',', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let base = u32::from(map.meta.address_base());
    let mut out = String::from(
        "name,group,area,address,manual_address,type,length,byte_order,bit,bits,scale,offset,unit,access,labels,description\n",
    );
    for r in &map.registers {
        let area = match r.area {
            Area::Coil => "coil",
            Area::Discrete => "discrete",
            Area::Input => "input",
            Area::Holding => "holding",
        };
        let ty = serde_json::to_value(r.ty)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let labels: Vec<_> = r
            .enum_labels
            .iter()
            .map(|(v, l)| format!("{v}={l}"))
            .collect();
        let row = [
            cell(&r.name),
            cell(&r.group),
            area.into(),
            r.address.to_string(),
            (u32::from(r.address) + base).to_string(),
            ty,
            if r.ty == DataType::String {
                r.count().to_string()
            } else {
                String::new()
            },
            if r.area.is_bit() {
                String::new()
            } else {
                format!("{:?}", r.byte_order(&map.meta))
            },
            r.bit.map(|b| b.to_string()).unwrap_or_default(),
            r.bits.map(|b| b.to_string()).unwrap_or_default(),
            num(r.scale),
            num(r.offset),
            cell(&r.unit),
            format!("{:?}", r.access),
            cell(&labels.join("; ")),
            cell(&r.description),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mbmap;

    fn demo() -> DeviceMap {
        mbmap::parse(include_bytes!("demo.mbmap.json")).map.unwrap()
    }

    fn all(format: Format, options: &Options) -> String {
        export(&demo(), format, options)
    }

    #[test]
    fn home_assistant_uses_placeholders_without_a_connection() {
        let out = all(Format::HomeAssistant, &Options::default());
        assert!(out.contains("modbus:\n  - name: "));
        assert!(out.contains("    type: tcp\n    host: \"<host>\"\n    port: 502\n"));
        assert!(out.contains("data_type: float32"));
        assert!(out.contains("Replace <host>"));
    }

    #[test]
    fn home_assistant_takes_the_connection() {
        let options = Options {
            connection: Some(ConnectionConfig {
                transport: Transport::Rtu {
                    port: "/dev/ttyUSB0".into(),
                    baudrate: 19200,
                    parity: Parity::E,
                    data_bits: 8,
                    stop_bits: 1,
                },
                unit_id: 7,
                timeout_ms: 1000,
                local_address: None,
            }),
            unit_id: None,
            interval_s: Some(5),
        };
        let out = all(Format::HomeAssistant, &options);
        assert!(out.contains(
            "type: serial\n    method: rtu\n    port: \"/dev/ttyUSB0\"\n    baudrate: 19200\n"
        ));
        assert!(out.contains("slave: 7\n"));
        assert!(out.contains("scan_interval: 5\n"));
        assert!(!out.contains("<host>"));
    }

    #[test]
    fn swaps_follow_the_byte_order() {
        let mut map = demo();
        for r in &mut map.registers {
            r.byte_order = Some(ByteOrder::CDAB);
        }
        let ha = export(&map, Format::HomeAssistant, &Options::default());
        assert!(ha.contains("swap: word"));
        let esp = export(&map, Format::Esphome, &Options::default());
        assert!(esp.contains("value_type: FP32_R"));
        let tg = export(&map, Format::Telegraf, &Options::default());
        assert!(tg.contains("byte_order = \"CDAB\""));
    }

    #[test]
    fn lists_what_a_format_cannot_express() {
        let mut map = demo();
        map.registers[0].byte_order = Some(ByteOrder::BADC);
        map.registers[1].access = crate::mbmap::Access::W;
        let esp = export(&map, Format::Esphome, &Options::default());
        let name0 = &map.registers[0].name;
        let name1 = &map.registers[1].name;
        assert!(esp.contains(&format!("#   {name0} (0): byte order with swapped bytes")));
        assert!(esp.contains(&format!("#   {name1} (2): write-only")));
        assert!(!esp.contains(&format!("name: {}", quote(name0))));
    }

    #[test]
    fn telegraf_groups_requests_and_keeps_field_names_unique() {
        let mut map = demo();
        map.registers[1].name = map.registers[0].name.clone();
        let out = export(&map, Format::Telegraf, &Options::default());
        assert!(out.contains("configuration_type = \"request\""));
        assert_eq!(out.matches("[[inputs.modbus.request]]").count(), 2);
        let first = slug(&map.registers[0].name, '_');
        assert!(out.contains(&format!("name = \"{first}\"")));
        assert!(out.contains(&format!("name = \"{first}_2\"")));
    }

    #[test]
    fn csv_has_a_row_per_register() {
        let out = all(Format::Csv, &Options::default());
        assert_eq!(out.lines().count(), demo().registers.len() + 1);
        assert!(out.starts_with("name,group,area,address,"));
    }

    #[test]
    fn file_names() {
        let map = demo();
        assert!(Format::Telegraf.file_name(&map).ends_with("-telegraf.conf"));
        assert_eq!(slug("ABB  M4M-30 / x", '-'), "abb-m4m-30-x");
    }
}
