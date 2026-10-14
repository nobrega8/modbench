// App state for the three-step workflow: connection, request, test.

import { ask } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "./api";
import { FUNCTION_NAME } from "./format";
import { settings } from "./settings.svelte";
import { NO_TIMES, addTime, type ResponseTimes } from "./stats";
import {
  endpointOf,
  idsOf,
  indexesOf,
  loadMapDefault,
  loadProfile,
  profileKey,
  saveMapDefault,
  saveProfile,
} from "./profiles";
import type {
  Area,
  ByteOrder,
  ConnectionConfig,
  DeviceMap,
  Diagnostic,
  LibraryEntry,
  Parity,
  PointResult,
  PollCycle,
  RawData,
  ScanHit,
  TrafficEntry,
} from "./types";

export type TransportKind = "tcp" | "rtuovertcp" | "rtu";
export type Page = "main" | "library" | "settings";
export type Tab = "registers" | "map" | "write" | "raw" | "log";

export interface OpenMap {
  path: string;
  map: DeviceMap;
  warnings: Diagnostic[];
}

export interface Exchange {
  time_ms: number;
  out: number[];
  in: number[];
}

const READ_AREA: Record<number, Area> = {
  1: "coil",
  2: "discrete",
  3: "holding",
  4: "input",
};
const CONNECTION_KEY = "modbench.connection";
const CONNECTION_FIELDS = [
  "transport",
  "host",
  "tcpPort",
  "serialPort",
  "baudrate",
  "parity",
  "dataBits",
  "stopBits",
  "delayMs",
  "timeoutMs",
  "unitId",
] as const;
const RAW_LIMIT = 200;

const LAST_MAP_KEY = "modbench.lastMap";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

class Bench {
  // Step 1 — connection
  transport = $state<TransportKind>("tcp");
  host = $state("127.0.0.1");
  tcpPort = $state(502);
  serialPort = $state("");
  baudrate = $state(9600);
  parity = $state<Parity>("E");
  dataBits = $state(8);
  stopBits = $state(1);
  delayMs = $state(100);
  timeoutMs = $state(1000);

  // Step 2 — request
  unitId = $state(1);
  source = $state<"manual" | "map">("manual");
  fc = $state(3);
  address = $state(0);
  quantity = $state(10);
  loop = $state(true);
  writeValues = $state<number[]>(Array(10).fill(0));
  maps = $state<OpenMap[]>([]);
  activeMap = $state<number | null>(null);
  mapError = $state<{ path: string; diagnostics: Diagnostic[] } | null>(null);
  orderOverride = $state<ByteOrder | null>(null);
  /** Registers of the active map that are not read (by index). */
  skipped = $state.raw<Set<number>>(new Set());
  library = $state.raw<LibraryEntry[]>([]);
  libraryFolder = $state("");
  /** Path of the map used last time, offered first in the picker. */
  lastMap = $state<string | null>(null);
  /** Set to ask the Library page to open the online library. */
  onlineRequested = $state(false);

  // Unit ID scan
  scanning = $state(false);
  /** The unit ID being asked right now. */
  scanUnit = $state<number | null>(null);
  scanHits = $state.raw<ScanHit[] | null>(null);
  scanError = $state<string | null>(null);

  // Step 3 — test
  running = $state(false);
  connected = $state(false);
  status = $state<{ ok: boolean; text: string } | null>(null);
  page = $state<Page>("main");
  tab = $state<Tab>("registers");
  counters = $state({ valid: 0, errors: 0, timeouts: 0 });
  /** How fast the device answers (normal and exception responses). */
  times = $state.raw<ResponseTimes>(NO_TIMES);
  read = $state.raw<{ address: number; data: RawData } | null>(null);
  previous = $state.raw<RawData | null>(null);
  mapResults = $state.raw<Map<number, PointResult>>(new Map());
  lastCycle = $state.raw<PollCycle | null>(null);
  log = $state.raw<TrafficEntry[]>([]);
  exchanges = $state.raw<Exchange[]>([]);

  private pendingLog: TrafficEntry[] = [];
  private flushQueued = false;

  constructor() {
    try {
      this.lastMap = localStorage.getItem(LAST_MAP_KEY);
    } catch {
      // Storage unavailable.
    }
    if (!settings.rememberConnection) return;
    try {
      const saved = JSON.parse(localStorage.getItem(CONNECTION_KEY) ?? "{}");
      for (const field of CONNECTION_FIELDS) {
        if (field in saved)
          (this as Record<string, unknown>)[field] = saved[field];
      }
    } catch {
      // Nothing saved yet, or storage unavailable.
    }
  }

  /** Remembers the connection settings used for the last test. */
  private saveConnection() {
    if (!settings.rememberConnection) return;
    const data: Record<string, unknown> = {};
    for (const field of CONNECTION_FIELDS) data[field] = this[field];
    try {
      localStorage.setItem(CONNECTION_KEY, JSON.stringify(data));
    } catch {
      // Storage unavailable: nothing to remember.
    }
  }

  /** Asks before a write when Settings require it; true to go ahead. */
  async confirmWrite(what: string): Promise<boolean> {
    if (!settings.confirmWrites) return true;
    return ask(
      `${what}

This writes to the device.`,
      {
        title: "Write to device",
        kind: "warning",
        okLabel: "Write",
        cancelLabel: "Cancel",
      },
    );
  }

  get map(): OpenMap | null {
    return this.activeMap === null ? null : (this.maps[this.activeMap] ?? null);
  }

  /**
   * Identity of the device the current choices belong to: the map, how it is
   * reached and the unit ID. Null until a map is open.
   */
  get deviceKey(): string | null {
    const open = this.map;
    if (!open) return null;
    return profileKey(open.path, endpointOf(this), this.unitId);
  }

  get isRead(): boolean {
    return this.fc in READ_AREA;
  }

  config(): ConnectionConfig {
    const transport =
      this.transport === "rtu"
        ? {
            kind: "rtu" as const,
            port: this.serialPort,
            baudrate: this.baudrate,
            parity: this.parity,
            data_bits: this.dataBits,
            stop_bits: this.stopBits,
          }
        : { kind: this.transport, host: this.host.trim(), port: this.tcpPort };
    return { transport, unit_id: this.unitId, timeout_ms: this.timeoutMs };
  }

  // ---- maps

  async refreshLibrary() {
    this.libraryFolder = await api.libraryFolder();
    this.library = await api.libraryList();
  }

  async openMap(path: string) {
    const parsed = await api.openMap(path);
    if (!parsed.map) {
      this.mapError = { path, diagnostics: parsed.diagnostics };
      return;
    }
    this.mapError = null;
    const entry: OpenMap = {
      path,
      map: parsed.map,
      warnings: parsed.diagnostics.filter((d) => d.severity === "warning"),
    };
    const existing = this.maps.findIndex((m) => m.path === path);
    if (existing >= 0) this.maps[existing] = entry;
    else this.maps.push(entry);
    this.selectMap(existing >= 0 ? existing : this.maps.length - 1);
    if (!this.running) this.applyDefaults(entry.map);
  }

  selectMap(index: number) {
    this.page = "main";
    this.activeMap = index;
    this.source = "map";
    const path = this.maps[index]?.path;
    if (path) {
      this.lastMap = path;
      try {
        localStorage.setItem(LAST_MAP_KEY, path);
      } catch {
        // Storage unavailable.
      }
    }
    this.restoreDevice();
    this.tab = "map";
    this.mapResults = new Map();
    this.lastCycle = null;
  }

  /**
   * Puts back what was chosen for the current device: registers switched off,
   * byte order override and Loop. Only changes what the screen shows; it never
   * sends anything to a device, and does nothing while a test runs.
   *
   * A device seen for the first time starts from the registers last used with
   * the same map (or everything on) and from the map's own byte order, so one
   * device's byte order override never carries over to another.
   */
  restoreDevice() {
    const open = this.map;
    if (!open || this.running) return;
    this.mapResults = new Map();
    this.lastCycle = null;
    if (!settings.rememberDevices) {
      this.skipped = new Set();
      this.orderOverride = null;
      return;
    }
    const registers = open.map.registers;
    const key = this.deviceKey!;
    const profile = loadProfile(key);
    if (profile) {
      this.skipped = indexesOf(profile.skipped, registers);
      this.orderOverride = profile.order;
      this.loop = profile.loop;
      return;
    }
    const seed = loadMapDefault(open.path, registers);
    this.skipped = seed ? indexesOf(seed, registers) : new Set();
    this.orderOverride = null;
  }

  /** Remembers the current choices for this device. */
  private persistDevice() {
    const open = this.map;
    const key = this.deviceKey;
    if (!open || !key || !settings.rememberDevices) return;
    const ids = idsOf(this.skipped, open.map.registers);
    saveProfile(key, {
      v: 1,
      skipped: ids,
      order: this.orderOverride,
      loop: this.loop,
    });
    saveMapDefault(open.path, ids);
  }

  setOrderOverride(order: ByteOrder | null) {
    this.orderOverride = order;
    this.persistDevice();
    this.refreshPolling();
  }

  setLoop(on: boolean) {
    this.loop = on;
    this.persistDevice();
  }

  /** Fills the connection settings with a map's defaults. */
  applyDefaults(map: DeviceMap) {
    const m = map.meta;
    if (m.protocol === "rtu") this.transport = "rtu";
    else if (m.protocol === "tcp" && this.transport === "rtu")
      this.transport = "tcp";
    if (m.unit_id !== null) this.unitId = m.unit_id;
    if (m.port !== null) this.tcpPort = m.port;
    if (m.baudrate !== null) this.baudrate = m.baudrate;
    if (m.parity !== null) this.parity = m.parity;
    if (m.data_bits !== null) this.dataBits = m.data_bits;
    if (m.stop_bits !== null) this.stopBits = m.stop_bits;
  }

  // ---- test run

  setQuantity(n: number) {
    this.quantity = n;
    const next = this.writeValues.slice(0, n);
    while (next.length < n) next.push(0);
    this.writeValues = next;
  }

  private async open(): Promise<boolean> {
    if (this.connected) return true;
    try {
      await api.connect(this.config());
      this.connected = true;
      return true;
    } catch (e) {
      this.status = { ok: false, text: `Could not connect: ${errorText(e)}` };
      return false;
    }
  }

  private async close() {
    if (!this.connected) return;
    this.connected = false;
    try {
      await api.disconnect();
    } catch {
      // already closed
    }
  }

  async start() {
    if (this.running || this.scanning) return;
    if (this.source === "manual" && !this.isRead) {
      const ok = await this.confirmWrite(
        `Send "${FUNCTION_NAME[this.fc]}" to address ${this.address}?`,
      );
      if (!ok) return;
    }
    this.status = null;
    this.running = true;
    this.saveConnection();
    if (!(await this.open())) {
      this.running = false;
      return;
    }
    if (this.source === "map") await this.runMap();
    else await this.runManual();
  }

  async stop() {
    this.running = false;
    if (this.source === "map") await api.stopPolling();
    await this.close();
  }

  private async runManual() {
    this.tab = this.isRead
      ? "registers"
      : this.tab === "registers"
        ? "write"
        : this.tab;
    do {
      await this.sendOnce();
      if (!this.loop || !this.running) break;
      await sleep(Math.max(this.delayMs, 10));
    } while (this.running);
    if (this.running) await this.stop();
  }

  private async sendOnce() {
    try {
      if (this.isRead) {
        const data = await api.readRaw(
          READ_AREA[this.fc],
          this.address,
          this.quantity,
        );
        this.previous =
          this.read?.address === this.address ? this.read.data : null;
        this.read = { address: this.address, data };
      } else if (this.fc === 5) {
        await api.writeCoils(this.address, [this.writeValues[0] !== 0]);
      } else if (this.fc === 15) {
        await api.writeCoils(
          this.address,
          this.writeValues.map((v) => v !== 0),
        );
      } else if (this.fc === 6) {
        await api.writeRegisters(this.address, [this.writeValues[0] & 0xffff]);
      } else {
        await api.writeRegisters(
          this.address,
          this.writeValues.map((v) => v & 0xffff),
        );
      }
      if (this.status && !this.status.ok) this.status = null;
    } catch (e) {
      this.status = { ok: false, text: errorText(e) };
    }
  }

  /** Turns reading of one map register on or off. */
  toggleRead(index: number) {
    const next = new Set(this.skipped);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    this.setSkipped(next);
  }

  setAllRead(on: boolean) {
    const count = this.map?.map.registers.length ?? 0;
    this.setSkipped(
      on ? new Set() : new Set(Array.from({ length: count }, (_, i) => i)),
    );
  }

  private setSkipped(next: Set<number>) {
    this.skipped = next;
    this.persistDevice();
    this.refreshPolling();
  }

  private pollRequest() {
    const open = this.map!;
    const m = open.map.meta;
    const all = $state.snapshot(open.map.registers);
    const indexes = all.map((_, i) => i).filter((i) => !this.skipped.has(i));
    const registers = indexes.map((i) => all[i]);
    // Scale factors of read registers are read too, even when switched off.
    const names = new Set(registers.map((r) => r.name));
    const scale_registers = [];
    for (const r of registers) {
      const sf = r.scale_factor;
      if (!sf || names.has(sf)) continue;
      const target = all.find((x) => x.name === sf);
      if (target) {
        scale_registers.push(target);
        names.add(sf);
      }
    }
    return {
      indexes,
      sentinels: $state.snapshot(m.sentinels),
      registers,
      scale_registers,
      byte_order: m.byte_order ?? "ABCD",
      byte_order_override: this.orderOverride,
      // Like a manual loop: Delay is the pause between every two requests.
      interval_ms: 0,
      delay_ms: this.delayMs,
      limits: {
        max_registers: m.max_read_registers ?? 125,
        max_bits: m.max_read_bits ?? 2000,
        max_gap: 0,
      },
    };
  }

  private async runMap() {
    if (!this.map) {
      this.status = { ok: false, text: "Choose a device map first." };
      await this.stop();
      return;
    }
    if (this.skipped.size >= this.map.map.registers.length) {
      this.status = {
        ok: false,
        text: "Every register is switched off. Turn on Read for at least one.",
      };
      await this.stop();
      return;
    }
    this.tab = "map";
    if (this.loop) {
      await api.startPolling(this.pollRequest());
      return;
    }
    try {
      this.onPoll(await api.pollMapOnce(this.pollRequest()));
    } catch (e) {
      this.status = { ok: false, text: errorText(e) };
    }
    await this.stop();
  }

  /** Restarts map polling so a changed setting takes effect. */
  async refreshPolling() {
    if (this.running && this.source === "map" && this.loop) {
      await api.startPolling(this.pollRequest());
    }
  }

  /**
   * Asks every unit ID from `from` to `to` whether a device answers, over the
   * connection settings above. Reads one register per unit ID.
   */
  async scan(from: number, to: number, timeoutMs: number) {
    if (this.running || this.scanning) return;
    this.scanning = true;
    this.scanHits = null;
    this.scanError = null;
    this.scanUnit = from;
    try {
      if (!(await this.open())) {
        this.scanError = this.status?.text ?? "Could not connect.";
        return;
      }
      this.status = null;
      this.scanHits = await api.scanUnits(from, to, timeoutMs);
    } catch (e) {
      this.scanError = errorText(e);
    } finally {
      await this.close();
      this.scanning = false;
      this.scanUnit = null;
    }
  }

  cancelScan() {
    api.cancelScan().catch(() => {});
  }

  onScanProgress(unit: number) {
    this.scanUnit = unit;
  }

  /**
   * Writes several map registers over one connection (opened just for this
   * when none is open). Returns what each write did, in order.
   */
  async writeMapValues(
    items: { index: number; value: import("./types").Value }[],
  ): Promise<{ index: number; error: string | null }[]> {
    const open = this.map;
    if (!open || !items.length) return [];
    const temporary = !this.connected;
    if (temporary && !(await this.open())) {
      const error = this.status?.text ?? "Could not connect.";
      return items.map((i) => ({ index: i.index, error }));
    }
    const done: { index: number; error: string | null }[] = [];
    try {
      for (const { index, value } of items) {
        const register = $state.snapshot(open.map.registers[index]);
        const sf = register.scale_factor
          ? open.map.registers.find((r) => r.name === register.scale_factor)
          : undefined;
        try {
          await api.writeMapValue(
            register,
            open.map.meta.byte_order ?? "ABCD",
            value,
            sf ? $state.snapshot(sf) : null,
          );
          done.push({ index, error: null });
        } catch (e) {
          done.push({ index, error: errorText(e) });
        }
      }
    } finally {
      if (temporary) await this.close();
    }
    return done;
  }

  /** Writes to a map register, opening the connection just for it if needed. */
  async writeMapValue(index: number, value: import("./types").Value) {
    const open = this.map;
    if (!open) return;
    const temporary = !this.connected;
    if (temporary && !(await this.open())) throw new Error(this.status?.text);
    try {
      const register = $state.snapshot(open.map.registers[index]);
      const sf = register.scale_factor
        ? open.map.registers.find((r) => r.name === register.scale_factor)
        : undefined;
      await api.writeMapValue(
        register,
        open.map.meta.byte_order ?? "ABCD",
        value,
        sf ? $state.snapshot(sf) : null,
      );
    } finally {
      if (temporary) await this.close();
    }
  }

  copyReadToWrite() {
    if (!this.read) return;
    const values =
      this.read.data.kind === "bits"
        ? this.read.data.values.map((b) => (b ? 1 : 0))
        : [...this.read.data.values];
    this.address = this.read.address;
    this.fc = this.read.data.kind === "bits" ? 15 : 16;
    this.quantity = values.length;
    this.writeValues = values;
    this.tab = "write";
  }

  // ---- events from the backend

  onPoll(cycle: PollCycle) {
    const next = new Map(this.mapResults);
    for (const r of cycle.results) next.set(r.index, r);
    this.mapResults = next;
    this.lastCycle = cycle;
  }

  onPollStopped(reason: string) {
    this.status = { ok: false, text: `Reading stopped: ${reason}` };
    this.running = false;
    this.connected = false;
  }

  onTraffic(entry: TrafficEntry) {
    const e = entry.error;
    if (e === null) this.counters.valid++;
    else if (e.startsWith("timed out")) this.counters.timeouts++;
    else this.counters.errors++;
    this.times = addTime(this.times, entry);
    this.pendingLog.push(entry);
    this.scheduleFlush();
  }

  onRaw(time_ms: number, direction: "out" | "in", bytes: number[]) {
    const list = this.exchanges;
    if (direction === "out") {
      this.exchanges = [...list, { time_ms, out: bytes, in: [] }].slice(
        -RAW_LIMIT,
      );
    } else if (list.length) {
      const last = list[list.length - 1];
      this.exchanges = [
        ...list.slice(0, -1),
        { ...last, in: [...last.in, ...bytes] },
      ];
    }
  }

  private scheduleFlush() {
    if (this.flushQueued) return;
    this.flushQueued = true;
    requestAnimationFrame(() => {
      this.log = [...this.log, ...this.pendingLog].slice(-settings.logLimit);
      this.pendingLog = [];
      this.flushQueued = false;
    });
  }

  resetTimes() {
    this.times = NO_TIMES;
  }

  clearLog() {
    this.log = [];
  }

  clearRaw() {
    this.exchanges = [];
  }
}

export const bench = new Bench();
