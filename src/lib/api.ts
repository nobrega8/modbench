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
  TypedAddress,
  Value,
} from "./types";

export const api = {
  appVersion: () => invoke<string>("app_version"),
  listSerialPorts: () => invoke<string[]>("list_serial_ports"),
  connect: (config: ConnectionConfig) => invoke<void>("connect", { config }),
  disconnect: () => invoke<void>("disconnect"),
  readRaw: (area: Area, address: number, count: number) =>
    invoke<RawData>("read_raw", { area, address, count }),
  writeRegisters: (address: number, words: number[]) =>
    invoke<void>("write_registers", { address, words }),
  writeCoils: (address: number, bits: boolean[]) =>
    invoke<void>("write_coils", { address, bits }),
  writeValue: (target: TypedAddress, value: Value) =>
    invoke<void>("write_value", { target, value }),
  openMap: (path: string) => invoke<Parsed>("open_map", { path }),
  saveMap: (path: string, map: DeviceMap) =>
    invoke<void>("save_map", { path, map }),
  startPolling: (request: PollRequest) =>
    invoke<void>("start_polling", { request }),
  stopPolling: () => invoke<void>("stop_polling"),
  pollMapOnce: (request: PollRequest) =>
    invoke<PollCycle>("poll_map_once", { request }),
  writeMapValue: (register: Register, byteOrder: ByteOrder, value: Value) =>
    invoke<void>("write_map_value", { register, byteOrder, value }),
  libraryFolder: () => invoke<string>("library_folder"),
  libraryList: () => invoke<LibraryEntry[]>("library_list"),
  libraryImport: (paths: string[]) =>
    invoke<ImportResult[]>("library_import", { paths }),
  libraryDelete: (path: string) => invoke<void>("library_delete", { path }),
  libraryOpenFolder: () => invoke<void>("library_open_folder"),
  libraryOnlineIndex: () => invoke<OnlineEntry[]>("library_online_index"),
  libraryOnlineImport: (paths: string[]) =>
    invoke<ImportResult[]>("library_online_import", { paths }),
  vendorIcon: (website: string) =>
    invoke<string | null>("vendor_icon", { website }),
  startupFile: () => invoke<string | null>("startup_file"),
  saveText: (path: string, text: string) =>
    invoke<void>("save_text", { path, text }),
};

/** Error text from a rejected command (commands reject with a string). */
export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
}
