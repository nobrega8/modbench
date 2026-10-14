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
  | { kind: "tcp"; host: string; port: number }
  | { kind: "rtuovertcp"; host: string; port: number }
  | {
      kind: "rtu";
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
}

export interface TypedAddress {
  area: Area;
  address: number;
  type: DataType;
  byte_order: ByteOrder;
  length: number | null;
  bit: number | null;
}

export interface LibraryEntry {
  path: string;
  vendor: string;
  model: string;
  description: string | null;
  vendor_website: string | null;
  /** x_status of the map: "unverified" until checked against the manual. */
  status: string | null;
  registers: number;
  warnings: number;
  error: string | null;
}

/** A map published in the online library (the mbmap-library repository). */
export interface OnlineEntry {
  path: string;
  vendor: string;
  model: string;
  description: string | null;
  vendor_website: string | null;
  status?: string | null;
  registers: number;
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
