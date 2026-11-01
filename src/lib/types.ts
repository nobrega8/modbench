// Mirrors of the Rust types sent over Tauri IPC (src-tauri/src).

export type Area = "coil" | "discrete" | "input" | "holding";
export type DataType =
  | "bool"
  | "int16"
  | "uint16"
  | "int32"
  | "uint32"
  | "float32"
  | "int64"
  | "uint64"
  | "float64"
  | "string";
export type ByteOrder = "ABCD" | "CDAB" | "BADC" | "DCBA";
export type Parity = "N" | "E" | "O";
export type Access = "R" | "W" | "RW";
export type Value = boolean | number | string;

export type Transport =
  | {
      kind: "tcp" | "rtuovertcp" | "udp" | "rtuoverudp";
      host: string;
      port: number;
    }
  | {
      kind: "rtu" | "ascii";
      port: string;
      baudrate: number;
      parity: Parity;
      data_bits: number;
      stop_bits: number;
    };

export interface ConnectionConfig {
  transport: Transport;
  unit_id: number;
  timeout_ms: number;
  /** Local address to send from over TCP/UDP; absent for the system's
   * routing. */
  local_address?: string;
  /** Modbus/TCP Security (see `tls.rs`): PEM file paths. */
  tls?: TlsConfig;
}

export interface TlsConfig {
  /** CA the device's certificate must be signed by; null trusts any. */
  ca: string | null;
  cert: string | null;
  key: string | null;
}

/** A network interface of this computer and one of its addresses. */
export interface LocalInterface {
  name: string;
  address: string;
  loopback: boolean;
}

export type RawData =
  { kind: "bits"; values: boolean[] } | { kind: "registers"; values: number[] };

export interface Meta {
  vendor: string;
  model: string;
  vendor_website: string | null;
  description: string | null;
  firmware: string | null;
  source: string | null;
  protocol: "tcp" | "rtu" | "tcp+rtu" | null;
  unit_id: number | null;
  port: number | null;
  baudrate: number | null;
  parity: Parity | null;
  data_bits: number | null;
  stop_bits: number | null;
  byte_order: ByteOrder | null;
  address_base: number | null;
  max_read_registers: number | null;
  max_read_bits: number | null;
  enums: [string, [number, string][]][];
  sentinels: [DataType, [number, string][]][];
  extra: [string, string][];
  /** Free notes about the whole file. */
  notes?: string[];
  /** Every model name or code the map applies to; empty means `model`. */
  models?: string[];
  /** Revision of the map's content, and its date. */
  revision?: number | null;
  revised?: string | null;
  /** Firmware range the map applies to, e.g. "1" to "2" (1.x and 2.x). */
  firmware_min?: string | null;
  firmware_max?: string | null;
  sources?: Source[];
}

/** A document a map was built or checked from. */
export interface Source {
  title: string;
  version: string | null;
  date: string | null;
  url: string | null;
  note: string | null;
}

export interface Register {
  name: string;
  group: string;
  area: Area;
  /** Address on the wire. */
  address: number;
  type: DataType;
  length: number | null;
  scale: number;
  /** Register holding a power-of-ten exponent for this value (SunSpec). */
  scale_factor: string | null;
  offset: number;
  unit: string;
  access: Access;
  /** Limits for writes, in engineering units. */
  write_min: number | null;
  write_max: number | null;
  /** Raw value a healthy device holds here, as written in the map. */
  reference: string | null;
  byte_order: ByteOrder | null;
  bit: number | null;
  /** Width of a uint16 bit field starting at `bit`. */
  bits: number | null;
  enum: [number, string][];
  enum_ref: string | null;
  description: string;
  extra: [string, string][];
  /** Free note about this register. */
  comment?: string;
  /** Firmware range that has this register. */
  firmware_min?: string | null;
  firmware_max?: string | null;
  /** What this register tells about the device. */
  role?: "model" | "firmware" | "serial" | null;
}

export interface DeviceMap {
  meta: Meta;
  registers: Register[];
}

export interface Diagnostic {
  severity: "error" | "warning";
  line: number | null;
  /** JSON Pointer to the value, in version 2 files. */
  pointer: string | null;
  code: string;
  message: string;
}

export interface Parsed {
  map: DeviceMap | null;
  diagnostics: Diagnostic[];
  /** Format version of the file; 1 is deprecated. */
  version: number | null;
}

/** Where a diagnostic points: "/registers/3/address", "line 5" or "file". */
export function diagnosticWhere(d: Diagnostic): string {
  if (d.pointer) return d.pointer;
  return d.line === null ? "file" : `line ${d.line}`;
}

/** File extensions offered when picking a map (`.mbmap.json` ends in json). */
export const MAP_EXTENSIONS = ["mbmap", "json"];

export interface PlanLimits {
  max_registers: number;
  max_bits: number;
  max_gap: number;
}

export interface PollRequest {
  registers: Register[];
  byte_order: ByteOrder;
  byte_order_override: ByteOrder | null;
  interval_ms: number;
  delay_ms: number;
  limits: PlanLimits;
  /** Map index of each register, echoed back in results. */
  indexes: number[];
  sentinels: [DataType, [number, string][]][];
  /** Scale factor registers the request needs but does not report. */
  scale_registers: Register[];
  /** Open the link again when it is lost. */
  reconnect: boolean;
}

export type ReferenceOutcome = "match" | "mismatch";

export interface PointResult {
  index: number;
  /** Result of the register's reference check, when it has one. */
  reference: ReferenceOutcome | null;
  value: Value | null;
  raw: RawData | null;
  error: string | null;
  /** Set when the raw value is a reserved one, e.g. "Unimplemented". */
  sentinel: string | null;
}

/** Change in the link while reading, sent by the backend. */
export type LinkEvent =
  | {
      state: "reconnecting";
      attempt: number;
      retry_ms: number;
      /** The error that showed the link was lost. */
      cause: string | null;
      /** Why the last attempt to open it again failed. */
      error: string | null;
    }
  | { state: "restored" };

export interface PollCycle {
  cycle: number;
  duration_ms: number;
  requests: number;
  results: PointResult[];
}

export interface TrafficEntry {
  time_ms: number;
  function: number;
  address: number;
  count: number;
  duration_ms: number;
  error: string | null;
  /** Set on the app's own lines (e.g. a register that starts failing),
   * which are not requests: function, address and count are 0. */
  note?: string;
}

export interface TypedAddress {
  area: Area;
  address: number;
  type: DataType;
  byte_order: ByteOrder;
  length: number | null;
  bit: number | null;
}

/** One identification object of a device (FC43). */
export interface DeviceIdItem {
  id: number;
  name: string;
  value: string;
}

export interface DeviceIdentity {
  /** Conformity level byte: which objects and access the device supports. */
  conformity: number;
  objects: DeviceIdItem[];
}

/** How a device answered one function code. */
export type Support =
  "supported" | "not_supported" | "refused" | "no_answer" | "skipped";

/** The answer to one function code, from checkFunctions. */
export interface FunctionCheck {
  function: number;
  name: string;
  support: Support;
  /** The exception or error, when there was one. */
  detail: string | null;
}

/** The largest read a device accepted. */
export interface ReadLimit {
  largest: number;
  /** Why one more failed; null when the protocol's maximum was read. */
  limit: string | null;
}

/** What an address scan looks at. */
export interface AddressScan {
  area: Area;
  from: number;
  to: number;
  /** Addresses read at once before splitting. */
  block: number;
  timeout_ms: number;
}

/** A run of addresses that answered during an address scan. */
export interface AddressRange {
  start: number;
  count: number;
}

export interface AddressProgress {
  address: number;
  done: number;
  total: number;
  found: number;
}

/** One request a client sent to the simulator. */
export interface SimEvent {
  time_ms: number;
  peer: string;
  unit: number;
  function: number;
  address: number | null;
  count: number | null;
  error: string | null;
  wrote: boolean;
}

/** One request the gateway passed on (see `gateway.rs`). */
export interface GatewayEvent {
  time_ms: number;
  peer: string;
  /** The unit ID the client asked for, and the one the request went to. */
  unit: number;
  target: number;
  function: number;
  address: number | null;
  count: number | null;
  /** The exception sent back, if any. */
  exception: number | null;
  /** Why, when the gateway made the exception up. */
  error: string | null;
  ms: number;
}

/** One request heard by the bus monitor, with its answer (see `monitor.rs`). */
export interface MonitorEvent {
  time_ms: number;
  outcome:
    "ok" | "exception" | "no_answer" | "broadcast" | "bad_check" | "unknown";
  unit: number | null;
  function: number | null;
  address: number | null;
  count: number | null;
  registers: number[] | null;
  bits: boolean[] | null;
  exception: number | null;
  ms: number | null;
  /** Bytes as hex. */
  request: string;
  response: string | null;
}

export interface GatewayOptions {
  listen: Transport;
  upstream: ConnectionConfig;
  unit: number | null;
  read_only: boolean;
}

/** Values held by the simulator, and which of those addresses exist. */
export interface SimView {
  data: RawData;
  defined: boolean[];
}

/** One simulated device (see `sim_info`). */
export interface SimDevice {
  /** The unit ID it answers; null answers every one no other device has. */
  unit: number | null;
  map: DeviceMap | null;
  source: string | null;
  strict: boolean;
  generators: Generator[];
  problems: (string | null)[];
}

/** The simulator as the backend has it (see `sim_info`). */
export interface SimInfo {
  /** Where it listens while running; null when stopped. */
  listen: Transport | null;
  devices: SimDevice[];
  window_open: boolean;
}

/** Where a simulator generator writes (or a copy reads): a register of the
 * loaded map, in engineering units, or one raw address. */
export type GeneratorTarget =
  { register: string } | { area: Area; address: number };

export type GeneratorKind =
  | { kind: "increment"; step: number; min: number; max: number }
  | { kind: "sine"; period_s: number; amplitude: number; offset: number }
  | { kind: "random"; min: number; max: number }
  | { kind: "copy"; from: GeneratorTarget };

/** A value the simulator changes on its own every `every_ms`. */
export type Generator = GeneratorTarget & GeneratorKind & { every_ms: number };

export interface LibraryEntry {
  path: string;
  vendor: string;
  model: string;
  /** Every model name or code the map applies to (`models`). */
  models: string[];
  description: string | null;
  vendor_website: string | null;
  /** x_status of the map: "verified", "official" or "unofficial" (older maps say "unverified" for "official"). */
  status: string | null;
  registers: number;
  warnings: number;
  error: string | null;
  /** Where the map came from; null for maps stored by older releases. */
  origin: "online" | "imported" | null;
  /** The file changed since it was stored. */
  edited: boolean;
}

/** A map published in the online library (the mbmap-library repository). */
export interface OnlineEntry {
  path: string;
  vendor: string;
  model: string;
  /** Every model name or code the map applies to (`models`). */
  models: string[];
  description: string | null;
  vendor_website: string | null;
  /** x_status of the map: "verified", "official" or "unofficial" (older maps say "unverified" for "official"). */
  status?: string | null;
  registers: number;
}

/** What saving a map from the builder into the library did. */
export interface SaveOutcome {
  /** Where the map is now; empty when nothing was written. */
  path: string;
  /** The library has a map for this vendor and model, and nothing was written. */
  exists: boolean;
}

export interface ImportResult {
  source: string;
  stored: string | null;
  error: string | null;
}

/** What to try when looking for a serial device's settings. */
export interface SerialScan {
  port: string;
  baudrates: number[];
  parities: Parity[];
  stop_bits: number[];
  data_bits: number;
  from: number;
  to: number;
  area: Area;
  address: number;
  timeout_ms: number;
  stop_at_first: boolean;
}

/** A serial setting and unit ID that got an answer. */
export interface SerialHit {
  baudrate: number;
  parity: Parity;
  data_bits: number;
  stop_bits: number;
  unit: number;
  response: string;
}

export interface SerialProgress {
  baudrate: number;
  parity: Parity;
  stop_bits: number;
  unit: number;
  done: number;
  total: number;
}

/** A unit ID that answered during a scan. */
export interface ScanHit {
  unit: number;
  response: string;
}

/** A Modbus TCP scan of a range of addresses (netscan.rs). */
export interface NetScan {
  /** 192.168.1.0/24, 192.168.1.10-50, or addresses separated by commas. */
  targets: string;
  port: number;
  unit: number;
  timeout_ms: number;
  identify: boolean;
  /** Attempts after the first when the connection or the answer times out. */
  retries: number;
  /** Local address to send from; null for the system's routing. */
  local_address: string | null;
}

export interface NetHit {
  ip: string;
  port: number;
  unit: number;
  /** modbus: it answered; tcp_only: the port opened but nothing answered. */
  status: "modbus" | "tcp_only";
  response_ms: number;
  response: string;
  identity: string | null;
}

export interface NetProgress {
  done: number;
  total: number;
  found: number;
}

/** A map that fits the connected device (mapfind.rs). */
export interface MapMatch {
  path: string;
  vendor: string;
  model: string;
  score: number;
  matched: string[];
  warnings: string[];
}

export interface MapSearch {
  matches: MapMatch[];
  /** Maps with identity registers that were checked. */
  candidates: number;
  /** Reads it took: one per distinct identity register. */
  reads: number;
}

/** A map to open in the map builder, and the register to show in it. */
export interface BuilderTarget {
  path: string;
  register: { area: Area; address: number; name: string } | null;
}

export type CheckStatus = "pass" | "fail" | "not_read";

export interface CheckRow {
  name: string;
  group: string;
  address: number;
  type: string;
  unit: string;
  value: string | null;
  raw: string | null;
  status: CheckStatus;
  reason: string | null;
}

/** A check that is not about one register: the model or the firmware. */
export interface CheckFinding {
  what: string;
  status: CheckStatus;
  detail: string;
}

/** The commissioning check of a device against its map. */
export interface CheckReport {
  rows: CheckRow[];
  findings: CheckFinding[];
  pass: number;
  fail: number;
  not_read: number;
}

/** What the HTML report says about the run besides the results. */
export interface ReportContext {
  map: string;
  map_file: string;
  map_revision: string | null;
  connection: string;
  unit_id: number;
  identification: [string, string][];
  when: string;
}

/** Software a map can be exported for (see `export.rs`). */
export type ExportFormat =
  "homeassistant" | "esphome" | "telegraf" | "nodered" | "csv";

export interface ExportOptions {
  /** The device's connection; without it the address is a placeholder. */
  connection?: ConnectionConfig;
  interval_s?: number;
}

/** A test sequence (see `sequence.rs`), saved as `.mbtest.json`. */
export interface Sequence {
  name: string;
  steps: SequenceStep[];
}

export type SequenceStep =
  | { do: "write"; register: string; value: number }
  | { do: "wait"; ms: number }
  | {
      do: "expect";
      register: string;
      equals?: number;
      tolerance?: number;
      min?: number;
      max?: number;
      within_ms: number;
    }
  | { do: "read"; register: string };

export interface StepResult {
  index: number;
  status: "pass" | "fail" | "error" | "done";
  value: string | null;
  message: string | null;
  time_ms: number;
  ms: number;
}
