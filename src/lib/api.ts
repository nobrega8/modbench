// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import type {
  AddressRange,
  AddressScan,
  Area,
  ByteOrder,
  ConnectionConfig,
  DeviceIdentity,
  FunctionCheck,
  ReadLimit,
  DeviceMap,
  Generator,
  ImportResult,
  LibraryEntry,
  LocalInterface,
  MapSearch,
  NetHit,
  NetScan,
  OnlineEntry,
  Parsed,
  PollCycle,
  PollRequest,
  RawData,
  Register,
  ScanHit,
  SerialHit,
  SerialScan,
  SimInfo,
  SimView,
  Transport,
  TypedAddress,
  Value,
} from "./types";

export const api = {
  appVersion: () => invoke<string>("app_version"),
  installedWithApt: () => invoke<boolean>("installed_with_apt"),
  listSerialPorts: () => invoke<string[]>("list_serial_ports"),
  connect: (config: ConnectionConfig) => invoke<void>("connect", { config }),
  disconnect: () => invoke<void>("disconnect"),
  setReadOnly: (on: boolean) => invoke<void>("set_read_only", { on }),
  setSingleWriteFc16: (on: boolean) =>
    invoke<void>("set_single_write_fc16", { on }),
  linkLost: () => invoke<boolean>("link_lost"),
  reconnect: () => invoke<void>("reconnect"),
  readRaw: (
    area: Area,
    address: number,
    count: number,
    maxPerRequest: number | null = null,
  ) => invoke<RawData>("read_raw", { area, address, count, maxPerRequest }),
  writeRegisters: (address: number, words: number[]) =>
    invoke<void>("write_registers", { address, words }),
  writeCoils: (address: number, bits: boolean[]) =>
    invoke<void>("write_coils", { address, bits }),
  writeValue: (target: TypedAddress, value: Value) =>
    invoke<void>("write_value", { target, value }),
  openMap: (path: string) => invoke<Parsed>("open_map", { path }),
  demoMap: () => invoke<Parsed>("demo_map"),
  findMaps: (paths: string[]) => invoke<MapSearch>("find_maps", { paths }),
  saveMap: (path: string, map: DeviceMap) =>
    invoke<void>("save_map", { path, map }),
  /** Writes a version 2 copy of a version 1 map; returns its path. */
  convertMap: (path: string) => invoke<string>("convert_map", { path }),
  startPolling: (request: PollRequest) =>
    invoke<void>("start_polling", { request }),
  stopPolling: () => invoke<void>("stop_polling"),
  pollMapOnce: (request: PollRequest) =>
    invoke<PollCycle>("poll_map_once", { request }),
  writeMapValue: (
    register: Register,
    byteOrder: ByteOrder,
    value: Value,
    scaleRegister: Register | null,
  ) =>
    invoke<void>("write_map_value", {
      register,
      byteOrder,
      value,
      scaleRegister,
    }),
  libraryFolder: () => invoke<string>("library_folder"),
  libraryList: () => invoke<LibraryEntry[]>("library_list"),
  libraryImport: (paths: string[]) =>
    invoke<ImportResult[]>("library_import", { paths }),
  libraryDelete: (path: string) => invoke<void>("library_delete", { path }),
  libraryOpenFolder: () => invoke<void>("library_open_folder"),
  openLink: (
    link:
      | "repo"
      | "issues"
      | "releases"
      | "library"
      | "spec"
      | "support"
      | "manual",
  ) => invoke<void>("open_link", { link }),
  libraryOnlineIndex: () => invoke<OnlineEntry[]>("library_online_index"),
  libraryOnlineImport: (paths: string[]) =>
    invoke<ImportResult[]>("library_online_import", { paths }),
  vendorIcon: (website: string) =>
    invoke<string | null>("vendor_icon", { website }),
  startupFile: () => invoke<string | null>("startup_file"),
  scanUnits: (from: number, to: number, timeoutMs: number) =>
    invoke<ScanHit[]>("scan_units", { from, to, timeoutMs }),
  scanSerial: (scan: SerialScan) =>
    invoke<SerialHit[]>("scan_serial", { scan }),
  cancelScan: () => invoke<void>("cancel_scan"),
  scanNetwork: (scan: NetScan) => invoke<NetHit[]>("scan_network", { scan }),
  localInterfaces: () => invoke<LocalInterface[]>("local_interfaces"),
  readText: (path: string) => invoke<string>("read_text", { path }),
  saveText: (path: string, text: string) =>
    invoke<void>("save_text", { path, text }),
  saveBytes: (path: string, bytes: Uint8Array) =>
    invoke<void>("save_bytes", { path, bytes: Array.from(bytes) }),
  appendText: (path: string, text: string) =>
    invoke<void>("append_text", { path, text }),
  readDeviceId: (level: number) =>
    invoke<DeviceIdentity>("read_device_id", { level }),
  checkFunctions: (address: number, writes: boolean) =>
    invoke<FunctionCheck[]>("check_functions", { address, writes }),
  largestRead: (area: Area, address: number) =>
    invoke<ReadLimit>("largest_read", { area, address }),
  scanAddresses: (scan: AddressScan) =>
    invoke<AddressRange[]>("scan_addresses", { scan }),
  /** Starts the simulator; returns where it listens (with the port it
   * picked when asked for port 0). */
  simStart: (listen: Transport) => invoke<Transport>("sim_start", { listen }),
  simStop: () => invoke<void>("sim_stop"),
  /** Adds a device with empty memory; returns its index. */
  simAddDevice: (unit: number | null) =>
    invoke<number>("sim_add_device", { unit }),
  simRemoveDevice: (device: number) =>
    invoke<void>("sim_remove_device", { device }),
  /** The unit ID a device answers; null answers every other one. */
  simSetUnit: (device: number, unit: number | null) =>
    invoke<void>("sim_set_unit", { device, unit }),
  /** Shapes a device after a map; `source` is the file it came from. */
  simLoad: (
    device: number,
    map: DeviceMap | null,
    strict: boolean,
    source: string | null,
  ) => invoke<void>("sim_load", { device, map, strict, source }),
  /** The simulator as the backend has it, for any window. */
  simInfo: () => invoke<SimInfo>("sim_info"),
  /** Shows the simulator in a window of its own, or brings it to the front. */
  simOpenWindow: () => invoke<void>("sim_open_window"),
  simCloseWindow: () => invoke<void>("sim_close_window"),
  simRead: (device: number, area: Area, address: number, count: number) =>
    invoke<SimView>("sim_read", { device, area, address, count }),
  simWrite: (device: number, area: Area, address: number, data: RawData) =>
    invoke<void>("sim_write", { device, area, address, data }),
  /** Every register of a device's map, in engineering units. */
  simValues: (device: number) =>
    invoke<(Value | null)[]>("sim_values", { device }),
  simSetValue: (device: number, register: string, value: number) =>
    invoke<void>("sim_set_value", { device, register, value }),
  /** Replaces a device's generators; returns why each one cannot run, or
   * null. */
  simSetGenerators: (device: number, list: Generator[]) =>
    invoke<(string | null)[]>("sim_set_generators", { device, list }),
};

/** Error text from a rejected command (commands reject with a string). */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
