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
  offset: number;
  unit: string;
  access: Access;
  byte_order: ByteOrder | null;
  bit: number | null;
  /** Width of a uint16 bit field starting at `bit`. */
  bits: number | null;
  enum: [number, string][];
  enum_ref: string | null;
  description: string;
  extra: [string, string][];
}

export interface DeviceMap {
  meta: Meta;
  registers: Register[];
}

export interface Diagnostic {
  severity: "error" | "warning";
  line: number | null;
  code: string;
  message: string;
}

export interface Parsed {
  map: DeviceMap | null;
  diagnostics: Diagnostic[];
}

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
}

export interface PointResult {
  index: number;
  value: Value | null;
  raw: RawData | null;
  error: string | null;
  /** Set when the raw value is a reserved one, e.g. "Unimplemented". */
  sentinel: string | null;
}

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
  registers: number;
  warnings: number;
  error: string | null;
}

export interface ImportResult {
  source: string;
  stored: string | null;
  error: string | null;
}
