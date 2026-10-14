// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import type {
  Area,
  ByteOrder,
  ConnectionConfig,
  DeviceMap,
  ImportResult,
  LibraryEntry,
  OnlineEntry,
  Parsed,
  PollCycle,
  PollRequest,
  RawData,
  Register,
  ScanHit,
  SerialHit,
  SerialScan,
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
    link: "repo" | "issues" | "releases" | "library" | "spec" | "support",
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
  readText: (path: string) => invoke<string>("read_text", { path }),
  saveText: (path: string, text: string) =>
    invoke<void>("save_text", { path, text }),
  appendText: (path: string, text: string) =>
    invoke<void>("append_text", { path, text }),
};

/** Error text from a rejected command (commands reject with a string). */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
