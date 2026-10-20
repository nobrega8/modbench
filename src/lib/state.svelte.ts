// App state for the three-step workflow: connection, request, test.

import { t } from "./i18n.svelte.ts";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "./api";
import { FUNCTION_NAME, otherReadArea } from "./format";
import type { Mark, MarksView } from "./highlights";
import {
  appliesTo,
  identify,
  rangeText,
  versionText,
  type Identity,
} from "./identity";
import { settings } from "./settings.svelte";
import {
  NO_BREAKDOWN,
  NO_TIMES,
  addEntry,
  addTime,
  type Breakdown,
  type ResponseTimes,
} from "./stats";
import { loadAlarms, saveAlarms, type Alarm } from "./alarms";
import { isCustom, loadCustom, saveCustom } from "./custom-registers";
import { recordHeader, recordRow } from "./csv";
import { qualityNotes } from "./quality";
import { computeResults, isComputed } from "./computed";
import { snapMap, snapRead, type Snapshot } from "./snapshots";
import {
  addRead,
  sameSeries,
  type GridFormat,
  type ManualRead,
  type ManualSeries,
} from "./manual-chart";
import {
  MAX_SERIES,
  addCycle,
  freeSlot,
  type Sample,
  type Series,
} from "./trend";
import {
  isFresh,
  markPoll,
  markRead,
  prune,
  type ChangeTimes,
} from "./changes";
import {
  endpointOf,
  idsOf,
  indexesOf,
  loadMapDefault,
  loadProfile,
  marksIds,
  marksIndexes,
  profileKey,
  readAsIds,
  readAsIndexes,
  saveMapDefault,
  saveProfile,
} from "./profiles";
import type {
  AddressProgress,
  AddressRange,
  AddressScan,
  Area,
  ByteOrder,
  CheckReport,
  ConnectionConfig,
  DeviceIdentity,
  FunctionCheck,
  ReadLimit,
  DeviceMap,
  Diagnostic,
  LibraryEntry,
  MapSearch,
  LinkEvent,
  Parity,
  PointResult,
  PollCycle,
  RawData,
  ReportContext,
  Register,
  ScanHit,
  SerialHit,
  SerialProgress,
  SerialScan,
  TrafficEntry,
} from "./types";

export type TransportKind = "tcp" | "rtuovertcp" | "udp" | "rtuoverudp" | "rtu";
export type Page = "main" | "tools" | "library" | "settings";
export type ToolId =
  | "simulator"
  | "identify"
  | "addresses"
  | "network"
  | "request"
  | "decoder"
  | "frames"
  | "stats"
  | "alarms"
  | "player";
/** Framing on a serial port. */
export type Framing = "rtu" | "ascii";
export type Tab = "registers" | "map" | "chart" | "write" | "raw" | "log";

function hasRange(r: {
  firmware_min?: string | null;
  firmware_max?: string | null;
}) {
  return !!(r.firmware_min || r.firmware_max);
}

export interface OpenMap {
  path: string;
  map: DeviceMap;
  warnings: Diagnostic[];
  /** Format version of the file; 1 is deprecated. */
  version: number | null;
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
export const CONNECTION_FIELDS = [
  "transport",
  "host",
  "tcpPort",
  "localAddress",
  "serialPort",
  "framing",
  "baudrate",
  "parity",
  "dataBits",
  "stopBits",
  "delayMs",
  "loopMs",
  "timeoutMs",
  "maxRegisters",
  "unitId",
] as const;
const RAW_LIMIT = 200;

const LAST_MAP_KEY = "modbench.lastMap";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Pauses between attempts to open a lost link (same as the backend's). */
const BACKOFF_MS = [500, 1000, 2000, 5000, 10000];

class Bench {
  /** The demo runs: nothing it changes is remembered (see demo.svelte.ts). */
  demo = $state(false);

  // Step 1 — connection
  transport = $state<TransportKind>("tcp");
  host = $state("127.0.0.1");
  tcpPort = $state(502);
  /** Local address to send from over TCP/UDP, to pick the network
   * interface; empty for the system's routing. */
  localAddress = $state("");
  serialPort = $state("");
  framing = $state<Framing>("rtu");
  baudrate = $state(9600);
  parity = $state<Parity>("E");
  dataBits = $state(8);
  stopBits = $state(1);
  delayMs = $state(100);
  /** With Loop on: time from the start of one cycle to the start of the
   * next. 0 repeats as soon as Delay allows. */
  loopMs = $state(0);
  timeoutMs = $state(1000);
  /** Largest register block per read request; longer reads are split. */
  maxRegisters = $state(125);

  // Step 2 — request
  unitId = $state(1);
  source = $state<"manual" | "map">("manual");
  fc = $state(3);
  address = $state(0);
  quantity = $state(10);
  loop = $state(true);
  writeValues = $state<number[]>(Array(10).fill(0));
  /** Raw: a map is only ever replaced, never edited in place, and a deep
   * proxy over thousands of registers makes every read slow. */
  maps = $state.raw<OpenMap[]>([]);
  activeMap = $state<number | null>(null);
  mapError = $state<{ path: string; diagnostics: Diagnostic[] } | null>(null);
  orderOverride = $state<ByteOrder | null>(null);
  /** Registers of the active map that are not read (by index). */
  skipped = $state.raw<Set<number>>(new Set());
  /** Registers read with the other function code than their area's (by index). */
  readAs = $state.raw<Map<number, Area>>(new Map());
  /** Highlighted registers of the active map (by index): their colour. */
  marks = $state.raw<Map<number, Mark>>(new Map());
  /** Whether the Device map lists all registers, the highlighted first, or only them. */
  marksView = $state<MarksView>("all");
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
  /** Serial settings scan: where it is, and what answered. */
  serialProgress = $state.raw<SerialProgress | null>(null);
  serialHits = $state.raw<SerialHit[] | null>(null);
  /** The settings of the last scans, and where each stopped (null when it
   * ran to the end), for exporting their results. */
  lastUnitScan = $state.raw<{
    from: number;
    to: number;
    timeout_ms: number;
    connection: string;
    stoppedAt: number | null;
  } | null>(null);
  lastSerialScan = $state.raw<{ scan: SerialScan; stopped: boolean } | null>(
    null,
  );
  lastAddressScan = $state.raw<{
    scan: AddressScan;
    connection: string;
    unit: number;
    stoppedAt: number | null;
  } | null>(null);

  // Step 3 — test
  running = $state(false);
  connected = $state(false);
  status = $state<{ ok: boolean; text: string } | null>(null);
  /** Set while a lost link is being opened again. */
  link = $state<{
    attempt: number;
    retry_ms: number;
    cause: string | null;
    error: string | null;
  } | null>(null);
  page = $state<Page>("main");
  tool = $state<ToolId>("simulator");
  tab = $state<Tab>("registers");
  counters = $state({ valid: 0, errors: 0, timeouts: 0 });
  /** How fast the device answers (normal and exception responses). */
  times = $state.raw<ResponseTimes>(NO_TIMES);
  /** Every request since the last reset, by cause and by function code. */
  breakdown = $state.raw<Breakdown>(NO_BREAKDOWN);
  /** Alarm limits of the active map's registers. */
  alarms = $state.raw<Alarm[]>([]);
  /** Address scan: where it is, and what answered. */
  addressProgress = $state.raw<AddressProgress | null>(null);
  addressHits = $state.raw<AddressRange[] | null>(null);
  addressArea = $state<Area>("holding");
  /** Values kept to compare with: the base snapshot, and what it is
   * compared with (another snapshot, or null for the live values). */
  snapshot = $state.raw<Snapshot | null>(null);
  compareWith = $state.raw<Snapshot | null>(null);
  /** Shows only the values that differ from the snapshot. */
  onlyDifferences = $state(false);

  /** Keeps the values shown now: the map's, or the manual read's. */
  takeSnapshot(label = "") {
    const time = new Date().toLocaleTimeString();
    const name = label || t("state.snapshot", { time });
    const open = this.map;
    if (this.source === "map" && open) {
      this.snapshot = snapMap(
        open.map.meta,
        open.map.registers,
        this.mapResults,
        name,
      );
    } else if (this.read && this.read.data.kind === "registers") {
      this.snapshot = snapRead(
        this.read.area,
        this.read.address,
        this.read.data.values,
        name,
      );
    } else if (this.read) {
      this.snapshot = snapRead(
        this.read.area,
        this.read.address,
        this.read.data.values.map((b) => (b ? 1 : 0)),
        name,
      );
    }
    this.compareWith = null;
  }

  clearSnapshot() {
    this.snapshot = null;
    this.compareWith = null;
    this.onlyDifferences = false;
  }

  read = $state.raw<{ address: number; area: Area; data: RawData } | null>(
    null,
  );
  previous = $state.raw<RawData | null>(null);
  /** When each grid offset (`g:`) or map register (`m:`) last changed. */
  changes = $state.raw<ChangeTimes>(new Map());
  /** Clock for the highlight; ticks only while something is highlighted. */
  now = $state(Date.now());
  private ticker: ReturnType<typeof setInterval> | null = null;
  mapResults = $state.raw<Map<number, PointResult>>(new Map());
  /** The user chose to write although the device does not match the map. */
  mismatchAccepted = $state(false);
  /** Firmware version the registers being read were chosen for. */
  private readFor: string | null = null;
  lastCycle = $state.raw<PollCycle | null>(null);
  log = $state.raw<TrafficEntry[]>([]);
  /** Map registers on the Chart tab, with their color slots. */
  chartSeries = $state.raw<Series[]>([]);
  /** Values over time of the charted registers, by map index. */
  history = $state.raw<Map<number, Sample[]>>(new Map());
  /** How the Read registers grid shows values, which is also how an
   * address is charted from it. */
  gridFormat = $state<GridFormat>("unsigned");
  gridOrder = $state<ByteOrder>("ABCD");
  /** Addresses of manual reads on the Chart tab. */
  manualSeries = $state.raw<ManualSeries[]>([]);
  /** Values over time of those addresses, by color slot. */
  manualHistory = $state.raw<Map<number, Sample[]>>(new Map());
  /** CSV file that gets one row per map cycle, and its columns. */
  recording = $state.raw<{
    path: string;
    mapPath: string;
    indexes: number[];
    rows: number;
  } | null>(null);
  recordError = $state<string | null>(null);
  /** Appends run one after another, so rows stay in order. */
  private recordQueue: Promise<void> = Promise.resolve();
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
    if (!settings.rememberConnection || this.demo) return;
    const data: Record<string, unknown> = {};
    for (const field of CONNECTION_FIELDS) data[field] = this[field];
    try {
      localStorage.setItem(CONNECTION_KEY, JSON.stringify(data));
    } catch {
      // Storage unavailable: nothing to remember.
    }
  }

  /** The connected device checked against the open map (from its identity
   * registers), or null without a map. */
  get identity(): Identity | null {
    return this.map ? identify(this.map.map, this.mapResults) : null;
  }

  /** Before the first write to a device that does not match the map, asks
   * whether to go ahead; true to write. */
  async acceptMismatch(): Promise<boolean> {
    const id = this.identity;
    const meta = this.map?.map.meta;
    if (!id || !meta || this.mismatchAccepted) return true;
    const problems: string[] = [];
    if (id.model.status === "mismatch")
      problems.push(
        `The map is for ${(meta.models?.length ? meta.models : [meta.model]).join(", ")}; the device reports ${id.model.found}.`,
      );
    if (id.firmware.status === "mismatch" && id.firmware.version)
      problems.push(
        `The map is for firmware ${rangeText(meta.firmware_min, meta.firmware_max)}; the device reports ${versionText(id.firmware.version)}.`,
      );
    if (!problems.length) return true;
    const ok = await ask(
      t("state.mismatch.body", { problems: problems.join("\n") }),
      {
        title: t("state.mismatch.title"),
        kind: "warning",
        okLabel: t("state.writeanyway"),
        cancelLabel: t("state.cancel"),
      },
    );
    this.mismatchAccepted = ok;
    return ok;
  }

  /** Asks before a write when Settings require it; true to go ahead. */
  async confirmWrite(what: string): Promise<boolean> {
    if (!settings.confirmWrites) return true;
    return ask(t("state.write.body", { what }), {
      title: t("state.write.title"),
      kind: "warning",
      okLabel: t("state.write"),
      cancelLabel: t("state.cancel"),
    });
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
            kind: this.framing,
            port: this.serialPort,
            baudrate: this.baudrate,
            parity: this.parity,
            data_bits: this.dataBits,
            stop_bits: this.stopBits,
          }
        : { kind: this.transport, host: this.host.trim(), port: this.tcpPort };
    const config: ConnectionConfig = {
      transport,
      unit_id: this.unitId,
      timeout_ms: this.timeoutMs,
    };
    if (this.transport !== "rtu" && this.localAddress.trim())
      config.local_address = this.localAddress.trim();
    return config;
  }

  // ---- maps

  async refreshLibrary() {
    this.libraryFolder = await api.libraryFolder();
    this.library = await api.libraryList();
  }

  /** Opens a map that is not a file (the demo's), under a made-up path. */
  openMapObject(map: DeviceMap, path: string) {
    const entry: OpenMap = { path, map, warnings: [], version: 2 };
    const existing = this.maps.findIndex((m) => m.path === path);
    this.maps =
      existing >= 0
        ? this.maps.map((m, i) => (i === existing ? entry : m))
        : [...this.maps, entry];
    this.selectMap(existing >= 0 ? existing : this.maps.length - 1);
  }

  /** Closes an open map by its path. */
  closeMapPath(path: string) {
    const index = this.maps.findIndex((m) => m.path === path);
    if (index < 0) return;
    this.maps = this.maps.filter((_, i) => i !== index);
    if (this.activeMap === index) {
      this.activeMap = null;
      this.source = "manual";
    } else if (this.activeMap !== null && this.activeMap > index)
      this.activeMap--;
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
      map: withCustom(parsed.map, path),
      warnings: parsed.diagnostics.filter((d) => d.severity === "warning"),
      version: parsed.version,
    };
    const existing = this.maps.findIndex((m) => m.path === path);
    this.maps =
      existing >= 0
        ? this.maps.map((m, i) => (i === existing ? entry : m))
        : [...this.maps, entry];
    this.selectMap(existing >= 0 ? existing : this.maps.length - 1);
    if (!this.running) this.applyDefaults(entry.map);
  }

  /** Converts an open version 1 map to version 2 and opens the new file in
   * its place. The original file is kept. */
  async convertMap(index: number) {
    const old = this.maps[index];
    if (!old) return;
    const path = await api.convertMap(old.path);
    const parsed = await api.openMap(path);
    if (!parsed.map) return;
    // The custom registers follow the map to its new file.
    saveCustom(path, old.map.registers);
    const next: OpenMap = {
      path,
      map: withCustom(parsed.map, path),
      warnings: parsed.diagnostics.filter((d) => d.severity === "warning"),
      version: parsed.version,
    };
    this.maps = this.maps.map((m, i) => (i === index ? next : m));
    if (this.activeMap === index) this.selectMap(index);
  }

  selectMap(index: number) {
    this.page = "main";
    this.activeMap = index;
    this.source = "map";
    const path = this.maps[index]?.path;
    if (path && !this.demo) {
      this.lastMap = path;
      try {
        localStorage.setItem(LAST_MAP_KEY, path);
      } catch {
        // Storage unavailable.
      }
    }
    this.restoreDevice();
    this.alarms = path ? loadAlarms(path) : [];
    this.tab = "map";
    this.changes = new Map();
    this.chartSeries = [];
    this.history = new Map();
    this.mapResults = new Map();
    this.mismatchAccepted = false;
    this.readFor = null;
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
    this.mismatchAccepted = false;
    this.readFor = null;
    this.lastCycle = null;
    if (!settings.rememberDevices) {
      this.skipped = new Set();
      this.readAs = new Map();
      this.marks = new Map();
      this.marksView = "all";
      this.orderOverride = null;
      return;
    }
    const registers = open.map.registers;
    const key = this.deviceKey!;
    const profile = loadProfile(key);
    if (profile) {
      this.skipped = indexesOf(profile.skipped, registers);
      this.readAs = readAsIndexes(profile.readAs, registers);
      this.marks = marksIndexes(profile.marks, registers);
      this.marksView = profile.marksView;
      this.orderOverride = profile.order;
      this.loop = profile.loop;
      return;
    }
    const seed = loadMapDefault(open.path, registers);
    this.skipped = seed ? indexesOf(seed, registers) : new Set();
    this.readAs = new Map();
    this.marks = new Map();
    this.marksView = "all";
    this.orderOverride = null;
  }

  /** Remembers the current choices for this device. */
  private persistDevice() {
    const open = this.map;
    const key = this.deviceKey;
    if (!open || !key || !settings.rememberDevices || this.demo) return;
    const ids = idsOf(this.skipped, open.map.registers);
    saveProfile(key, {
      v: 1,
      skipped: ids,
      order: this.orderOverride,
      loop: this.loop,
      readAs: readAsIds(this.readAs, open.map.registers),
      marks: marksIds(this.marks, open.map.registers),
      marksView: this.marksView,
    });
    saveMapDefault(open.path, ids);
  }

  /** Replaces the alarm limits of the active map, and remembers them. */
  setAlarms(alarms: Alarm[]) {
    this.alarms = alarms;
    if (this.map) saveAlarms(this.map.path, alarms);
  }

  setOrderOverride(order: ByteOrder | null) {
    this.orderOverride = order;
    this.persistDevice();
    this.refreshPolling();
  }

  /**
   * Reads one register with the other function code (FC03 instead of FC04 or
   * the other way round, FC01 instead of FC02), or back with its own area's
   * when `area` is null. Writes keep using the register's own area.
   */
  setReadArea(index: number, area: Area | null) {
    const reg = this.map?.map.registers[index];
    if (!reg) return;
    const next = new Map(this.readAs);
    if (area === null || area === reg.area) next.delete(index);
    else if (area === otherReadArea(reg.area)) next.set(index, area);
    else return;
    this.readAs = next;
    this.persistDevice();
    this.refreshPolling();
  }

  /** Highlights a register in a colour, or removes its highlight. */
  setMark(index: number, mark: Mark | null) {
    const next = new Map(this.marks);
    if (mark === null) next.delete(index);
    else next.set(index, mark);
    this.marks = next;
    // With nothing left to show, "only highlighted" would show an empty list.
    if (next.size === 0) this.marksView = "all";
    this.persistDevice();
  }

  setMarksView(view: MarksView) {
    this.marksView = this.marks.size === 0 ? "all" : view;
    this.persistDevice();
  }

  setLoop(on: boolean) {
    this.loop = on;
    this.persistDevice();
  }

  /** Changes the time between loop cycles; a running map poll picks it up. */
  setLoopMs(ms: number) {
    this.loopMs = Math.max(0, Math.round(ms) || 0);
    this.saveConnection();
    this.refreshPolling();
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
      this.status = {
        ok: false,
        text: t("state.connectfail", { error: errorText(e) }),
      };
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
      if (settings.readOnly) {
        this.status = {
          ok: false,
          text: t("state.readonly"),
        };
        return;
      }
      const ok = await this.confirmWrite(
        t("state.send", {
          name: FUNCTION_NAME[this.fc],
          address: this.address,
        }),
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
    this.link = null;
    if (this.source === "map") await api.stopPolling();
    await this.close();
  }

  private async runManual() {
    this.tab = this.isRead
      ? this.tab === "chart"
        ? "chart"
        : "registers"
      : this.tab === "registers"
        ? "write"
        : this.tab;
    do {
      const started = Date.now();
      await this.sendOnce();
      if (!this.loop || !this.running) break;
      if (this.status && !this.status.ok && (await this.lost())) {
        await this.reopen();
        continue;
      }
      const rest = this.loopMs - (Date.now() - started);
      await sleep(Math.max(rest, this.delayMs, 10));
    } while (this.running);
    if (this.running) await this.stop();
  }

  private async sendOnce() {
    try {
      if (this.isRead) {
        const area = READ_AREA[this.fc];
        const data = await api.readRaw(
          area,
          this.address,
          this.quantity,
          area === "coil" || area === "discrete" ? null : this.maxRegisters,
        );
        const prev =
          this.read?.address === this.address ? this.read.data : null;
        this.previous = prev;
        this.markChanges(
          prev
            ? markRead(this.changes, prev, data, Date.now())
            : this.withoutGrid(),
        );
        this.read = { address: this.address, area, data };
        this.chartRead(this.read);
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
      // A failed read leaves a gap in the charted lines.
      if (this.isRead) this.chartRead(null);
    }
  }

  /** Adds a manual read (null: one that failed) to the Chart tab. */
  private chartRead(read: ManualRead | null) {
    if (!this.manualSeries.length) return;
    const next = addRead(
      this.manualSeries,
      this.manualHistory,
      read,
      Date.now(),
    );
    this.manualSeries = next.series;
    this.manualHistory = next.history;
  }

  /** Whether the link is lost and should be opened again. */
  private async lost(): Promise<boolean> {
    if (!settings.autoReconnect) return false;
    try {
      return await api.linkLost();
    } catch {
      return false;
    }
  }

  /** Opens a lost link again, waiting longer between attempts. */
  private async reopen() {
    const cause = this.status?.ok === false ? this.status.text : null;
    let error: string | null = null;
    for (let attempt = 1; this.running; attempt++) {
      const retry_ms = BACKOFF_MS[Math.min(attempt, BACKOFF_MS.length) - 1];
      this.link = { attempt, retry_ms, cause, error };
      await sleep(retry_ms);
      if (!this.running) break;
      try {
        await api.reconnect();
        this.link = null;
        this.status = null;
        return;
      } catch (e) {
        error = errorText(e);
      }
    }
    this.link = null;
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

  private pollRequest(ignoreSkipped = false) {
    const open = this.map!;
    const m = open.map.meta;
    const all = $state.snapshot(open.map.registers);
    // Registers the device's firmware does not have are not read.
    const version = this.identity?.firmware.version ?? null;
    this.readFor = version ? versionText(version) : null;
    const indexes = all
      .map((_, i) => i)
      .filter(
        (i) =>
          (ignoreSkipped || !this.skipped.has(i)) &&
          !isComputed(all[i]) &&
          appliesTo(all[i], version),
      );
    const registers = indexes.map((i) => {
      const area = this.readAs.get(i);
      return area ? { ...all[i], area } : all[i];
    });
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
      // Like a manual loop: Delay is the pause between every two requests,
      // and a cycle starts at most every loopMs.
      interval_ms: Math.max(0, Math.round(this.loopMs) || 0),
      delay_ms: this.delayMs,
      reconnect: settings.autoReconnect,
      limits: {
        max_registers: Math.min(m.max_read_registers ?? 125, this.maxRegisters),
        max_bits: m.max_read_bits ?? 2000,
        max_gap: 0,
      },
    };
  }

  private async runMap() {
    if (!this.map) {
      this.status = { ok: false, text: t("state.choosemap") };
      await this.stop();
      return;
    }
    if (this.skipped.size >= this.map.map.registers.length) {
      this.status = {
        ok: false,
        text: t("state.alloff"),
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
    this.cancelled = false;
    this.lastUnitScan = null;
    const connection = this.connectionText;
    try {
      if (!(await this.open())) {
        this.scanError = this.status?.text ?? t("state.connectshort");
        return;
      }
      this.status = null;
      this.scanHits = await api.scanUnits(from, to, timeoutMs);
      this.lastUnitScan = {
        from,
        to,
        timeout_ms: timeoutMs,
        connection,
        stoppedAt: this.cancelled ? this.scanUnit : null,
      };
    } catch (e) {
      this.scanError = errorText(e);
    } finally {
      await this.close();
      this.scanning = false;
      this.scanUnit = null;
    }
  }

  /**
   * Tries combinations of baud rate, parity and stop bits on the serial port
   * of step 1, asking each unit ID for one register. Reads only.
   */
  async scanSerial(
    scan: Omit<SerialScan, "port" | "data_bits">,
  ): Promise<void> {
    if (this.running || this.scanning) return;
    this.scanning = true;
    this.serialHits = null;
    this.scanError = null;
    this.serialProgress = null;
    this.cancelled = false;
    this.lastSerialScan = null;
    try {
      await this.close();
      const full: SerialScan = {
        ...scan,
        port: this.serialPort,
        data_bits: this.dataBits,
      };
      this.serialHits = await api.scanSerial(full);
      this.lastSerialScan = { scan: full, stopped: this.cancelled };
    } catch (e) {
      this.scanError = errorText(e);
    } finally {
      this.scanning = false;
      this.serialProgress = null;
    }
  }

  onSerialProgress(p: SerialProgress) {
    this.serialProgress = p;
  }

  /** Uses a serial setting found by a scan. */
  useSerial(hit: SerialHit) {
    this.baudrate = hit.baudrate;
    this.parity = hit.parity;
    this.dataBits = hit.data_bits;
    this.stopBits = hit.stop_bits;
    this.unitId = hit.unit;
  }

  /** Set when the user stops a scan, to tell a stopped scan from one that
   * ran to the end. */
  private cancelled = false;

  cancelScan() {
    this.cancelled = true;
    api.cancelScan().catch(() => {});
  }

  /** The connection of step 1, in a few words. */
  get connectionText(): string {
    return this.transport === "rtu"
      ? `${this.serialPort} · ${this.framing === "ascii" ? "ASCII " : ""}${this.baudrate} ${this.dataBits}${this.parity}${this.stopBits}`
      : `${this.host}:${this.tcpPort}${this.localAddress.trim() ? ` from ${this.localAddress.trim()}` : ""}`;
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
    if (!(await this.acceptMismatch()))
      return items.map((i) => ({
        index: i.index,
        error: t("state.notwritten"),
      }));
    const temporary = !this.connected;
    if (temporary && !(await this.open())) {
      const error = this.status?.text ?? t("state.connectshort");
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

  /**
   * Runs `work` over the open connection, or over one opened just for it.
   * Fails with the connection error when it cannot connect.
   */
  private async withLink<T>(work: () => Promise<T>): Promise<T> {
    const temporary = !this.connected;
    if (temporary) {
      this.saveConnection();
      if (!(await this.open()))
        throw new Error(this.status?.text ?? t("state.connectshort"));
      this.status = null;
    }
    try {
      return await work();
    } finally {
      if (temporary) await this.close();
    }
  }

  /**
   * Sends a request built by hand over the connection of step 1, and
   * returns the frames that went out and came back.
   */
  async sendPdu(
    fc: number,
    data: number[],
    broadcast: boolean,
  ): Promise<Exchange | null> {
    if (this.scanning) throw new Error(t("state.scanrunning"));
    return this.withLink(async () => {
      const before = this.exchanges.length;
      const last = this.exchanges.at(-1);
      await api.sendPdu(fc, data, broadcast);
      // The frames arrive as events; give the last ones a moment.
      await sleep(30);
      const now = this.exchanges;
      const fresh = now.length > before || now.at(-1) !== last;
      return fresh ? (now.at(-1) ?? null) : null;
    });
  }

  /** Sends bytes as they are, without framing, and returns the answer. */
  async sendRaw(bytes: number[], waitMs: number): Promise<number[]> {
    if (this.running || this.scanning)
      throw new Error("Stop the test on the Main page first.");
    await this.close();
    return api.sendRaw(this.config(), bytes, waitMs);
  }

  /**
   * Checks the library's maps (and the maps open now) against the device of
   * step 1 by their identity registers, best fit first.
   */
  async findMaps(): Promise<MapSearch> {
    if (this.scanning) throw new Error(t("state.scanrunning"));
    const paths = [
      ...this.library.filter((e) => !e.error).map((e) => e.path),
      ...this.maps.map((m) => m.path).filter((p) => !p.startsWith("modbench:")),
    ];
    return this.withLink(() => api.findMaps([...new Set(paths)]));
  }

  /**
   * Reads every register of the open map once (the ones switched off too)
   * and checks the results against the map. Nothing is written. Stops short
   * when the map is being read in a loop, which owns the connection.
   */
  async checkDevice(): Promise<{
    report: CheckReport;
    context: ReportContext;
  }> {
    const open = this.map;
    if (!open) throw new Error(t("state.choosemap"));
    if (this.running) throw new Error(t("check.stop_first"));
    return this.withLink(async () => {
      const report = await api.checkDevice(
        $state.snapshot(open.map),
        this.pollRequest(true),
      );
      const identification = await api
        .readDeviceId(1)
        .then((id): [string, string][] =>
          id.objects.map((o) => [o.name, o.value]),
        )
        .catch((): [string, string][] => []);
      const meta = open.map.meta;
      const connection =
        this.transport === "rtu"
          ? `${this.framing === "ascii" ? "ASCII" : "RTU"} ${this.serialPort} ${this.baudrate} ${this.dataBits}${this.parity}${this.stopBits}`
          : `${this.transport.toUpperCase()} ${this.host.trim()}:${this.tcpPort}`;
      const context: ReportContext = {
        map: `${meta.vendor} ${meta.model}`,
        map_file: open.path,
        map_revision:
          meta.revision == null
            ? null
            : `${meta.revision}${meta.revised ? `, ${meta.revised}` : ""}`,
        connection,
        unit_id: this.unitId,
        identification,
        when: new Date().toISOString().slice(0, 16).replace("T", " ") + " UTC",
      };
      return { report, context };
    });
  }

  /** Asks the device to identify itself (FC43): 1 basic, 2 regular, 3 extended. */
  async readDeviceId(level: number): Promise<DeviceIdentity> {
    if (this.scanning) throw new Error(t("state.scanrunning"));
    return this.withLink(() => api.readDeviceId(level));
  }

  /**
   * Tries each read function code once at `address`, over the connection of
   * step 1. With `writes`, also the write function codes, which write back
   * what was just read there: always asked first. Null when not confirmed.
   */
  async checkFunctions(
    address: number,
    writes: boolean,
  ): Promise<FunctionCheck[] | null> {
    if (this.scanning) throw new Error(t("state.scanrunning"));
    if (writes) {
      if (settings.readOnly) throw new Error(t("state.rofunctions"));
      const ok = await ask(t("state.tryw.body", { address }), {
        title: t("state.tryw.title"),
        kind: "warning",
        okLabel: t("state.write"),
        cancelLabel: t("state.cancel"),
      });
      if (!ok) return null;
    }
    return this.withLink(() => api.checkFunctions(address, writes));
  }

  /** The largest block of an area the device returns in one read. */
  async largestRead(area: Area, address: number): Promise<ReadLimit> {
    if (this.scanning) throw new Error(t("state.scanrunning"));
    return this.withLink(() => api.largestRead(area, address));
  }

  /** Finds which addresses of an area answer, over the connection of step 1. */
  async scanAddresses(scan: AddressScan) {
    if (this.running || this.scanning) return;
    this.scanning = true;
    this.addressHits = null;
    this.addressArea = scan.area;
    this.addressProgress = {
      address: scan.from,
      done: 0,
      total: scan.to - scan.from + 1,
      found: 0,
    };
    this.scanError = null;
    this.cancelled = false;
    this.lastAddressScan = null;
    const connection = this.connectionText;
    try {
      this.addressHits = await this.withLink(() => api.scanAddresses(scan));
      this.lastAddressScan = {
        scan,
        connection,
        unit: this.unitId,
        stoppedAt: this.cancelled
          ? (this.addressProgress?.address ?? scan.from)
          : null,
      };
    } catch (e) {
      this.scanError = errorText(e);
    } finally {
      this.scanning = false;
      this.addressProgress = null;
    }
  }

  onAddressProgress(p: AddressProgress) {
    this.addressProgress = p;
  }

  /** Sets up step 2 to read a range of addresses, and goes there. */
  readRange(area: Area, start: number, count: number) {
    const fc = { coil: 1, discrete: 2, holding: 3, input: 4 }[area];
    this.source = "manual";
    this.fc = fc;
    this.address = start;
    this.setQuantity(
      Math.min(count, area === "coil" || area === "discrete" ? 2000 : 125),
    );
    this.tab = "registers";
    this.page = "main";
  }

  /**
   * Switches one bit of a holding register (read, change the bit, write
   * back), opening the connection just for it when needed.
   */
  async writeBit(address: number, bit: number, on: boolean) {
    if (settings.readOnly)
      throw new Error("Read-only mode is on: writes are blocked.");
    const ok = await this.confirmWrite(
      `Set bit ${bit} of holding register ${address} to ${on ? 1 : 0}?`,
    );
    if (!ok) return;
    await this.withLink(() =>
      api.writeValue(
        {
          area: "holding",
          address,
          type: "bool",
          byte_order: "ABCD",
          length: null,
          bit,
        },
        on,
      ),
    );
    // Show the change at once when nothing reads it again.
    const read = this.read;
    if (!this.running && read?.data.kind === "registers") {
      const i = address - read.address;
      const values = [...read.data.values];
      if (i >= 0 && i < values.length) {
        values[i] = on ? values[i] | (1 << bit) : values[i] & ~(1 << bit);
        this.read = { ...read, data: { kind: "registers", values } };
      }
    }
  }

  /** Writes to a map register, opening the connection just for it if needed. */
  async writeMapValue(index: number, value: import("./types").Value) {
    const open = this.map;
    if (!open) return;
    if (!(await this.acceptMismatch()))
      throw new Error("the device does not match the map");
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

  onPoll(read: PollCycle) {
    // Computed values are worked out from what was just read, and join the
    // cycle like read registers (charts, recordings, alarms)
    const cycle = this.withComputed(read);
    const regs = this.map?.map.registers;
    if (regs) {
      const notes = qualityNotes(regs, this.mapResults, cycle.results);
      if (notes.length) this.addNotes(notes);
    }
    this.markChanges(
      markPoll(this.changes, this.mapResults, cycle.results, Date.now()),
    );
    const next = new Map(this.mapResults);
    for (const r of cycle.results) next.set(r.index, r);
    this.mapResults = next;
    // Once the firmware is known, read only the registers it has.
    const version = this.identity?.firmware.version;
    const seen = version ? versionText(version) : null;
    if (seen !== this.readFor && this.map?.map.registers.some(hasRange))
      this.refreshPolling();
    this.lastCycle = cycle;
    if (this.chartSeries.length)
      this.history = addCycle(
        this.history,
        this.chartSeries,
        cycle.results,
        Date.now(),
      );
    this.recordCycle(cycle);
  }

  /**
   * Starts recording every map cycle to a CSV file: the registers read now
   * become the columns, in map order. The file is replaced.
   */
  async startRecording(path: string) {
    const open = this.map;
    if (!open) return;
    const all = open.map.registers;
    const indexes = all
      .map((_, i) => i)
      .filter((i) => !this.skipped.has(i) && all[i].access !== "W");
    this.recordError = null;
    try {
      await api.saveText(
        path,
        recordHeader(
          indexes.map((i) => all[i]),
          settings.csvSeparator,
        ),
      );
      this.recording = { path, mapPath: open.path, indexes, rows: 0 };
    } catch (e) {
      this.recordError = errorText(e);
    }
  }

  stopRecording() {
    this.recording = null;
  }

  private recordCycle(cycle: PollCycle) {
    const rec = this.recording;
    const open = this.map;
    if (!rec || !open || open.path !== rec.mapPath) return;
    const text = recordRow(
      Date.now(),
      open.map.registers,
      rec.indexes,
      new Map(cycle.results.map((r) => [r.index, r])),
      settings.csvSeparator,
    );
    this.recording = { ...rec, rows: rec.rows + 1 };
    this.recordQueue = this.recordQueue.then(() =>
      api.appendText(rec.path, text).catch((e) => {
        this.recordError = errorText(e);
        this.recording = null;
      }),
    );
  }

  onLink(event: LinkEvent) {
    if (event.state === "restored") {
      this.link = null;
      if (this.status && !this.status.ok) this.status = null;
    } else if (this.running) {
      const { attempt, retry_ms, cause, error } = event;
      this.link = { attempt, retry_ms, cause, error };
    }
  }

  onPollStopped(reason: string) {
    this.link = null;
    this.status = { ok: false, text: t("state.stopped", { reason }) };
    this.running = false;
    this.connected = false;
  }

  /** A cycle with the results of the map's computed values added. */
  private withComputed(cycle: PollCycle): PollCycle {
    const regs = this.map?.map.registers;
    if (!regs?.some(isComputed)) return cycle;
    const now = new Map(this.mapResults);
    for (const r of cycle.results) now.set(r.index, r);
    const extra = computeResults(regs, now, this.skipped);
    return { ...cycle, results: [...cycle.results, ...extra] };
  }

  /** Adds the app's own lines to the log (not requests: no statistics). */
  private addNotes(notes: string[]) {
    const time_ms = Date.now();
    const lines: TrafficEntry[] = notes.map((note) => ({
      time_ms,
      function: 0,
      address: 0,
      count: 0,
      duration_ms: 0,
      error: null,
      note,
    }));
    this.log = [...this.log, ...lines].slice(-settings.logLimit);
  }

  onTraffic(entry: TrafficEntry) {
    if (entry.note) {
      // Not a Modbus request (raw bytes): logged, not counted
      this.pendingLog.push(entry);
      this.scheduleFlush();
      return;
    }
    const e = entry.error;
    if (e === null) this.counters.valid++;
    else if (e.startsWith("timed out")) this.counters.timeouts++;
    else this.counters.errors++;
    this.times = addTime(this.times, entry);
    this.breakdown = addEntry(this.breakdown, entry);
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

  /** Whether a grid offset or map register changed within the hold time. */
  fresh(key: string): boolean {
    const hold = settings.highlightSeconds * 1000;
    return hold > 0 && isFresh(this.changes, key, this.now, hold);
  }

  private withoutGrid(): ChangeTimes {
    const kept = [...this.changes].filter(([k]) => !k.startsWith("g:"));
    return kept.length === this.changes.size ? this.changes : new Map(kept);
  }

  /** Stores new change times and keeps the clock running while needed. */
  private markChanges(times: ChangeTimes) {
    if (times === this.changes) return;
    this.changes = times;
    this.now = Date.now();
    if (this.ticker || !times.size || settings.highlightSeconds <= 0) return;
    this.ticker = setInterval(() => {
      this.now = Date.now();
      this.changes = prune(
        this.changes,
        this.now,
        settings.highlightSeconds * 1000,
      );
      if (!this.changes.size && this.ticker) {
        clearInterval(this.ticker);
        this.ticker = null;
      }
    }, 200);
  }

  /** Adds a register the open map is missing; it is read from the next
   * cycle and kept for this map file on this computer. */
  addCustomRegister(reg: Register) {
    const open = this.map;
    if (!open) return;
    const registers = [...open.map.registers, reg];
    this.setRegisters(open, registers);
    saveCustom(open.path, registers);
    this.refreshPolling();
  }

  /** Replaces the registers of an open map (the maps are never edited in
   * place). */
  private setRegisters(open: OpenMap, registers: Register[]) {
    const next: OpenMap = { ...open, map: { ...open.map, registers } };
    this.maps = this.maps.map((m) => (m === open ? next : m));
  }

  /** Removes a register added with addCustomRegister. Everything kept by
   * register index moves down past it. */
  removeCustomRegister(index: number) {
    const open = this.map;
    if (!open || !isCustom(open.map.registers[index])) return;
    const registers = open.map.registers.filter((_, i) => i !== index);
    this.setRegisters(open, registers);
    saveCustom(open.path, registers);
    const shift = (i: number) => (i > index ? i - 1 : i);
    const rekey = <T>(m: Map<number, T>) =>
      new Map(
        [...m].filter(([i]) => i !== index).map(([i, v]) => [shift(i), v]),
      );
    this.skipped = new Set(
      [...this.skipped].filter((i) => i !== index).map(shift),
    );
    this.readAs = rekey(this.readAs);
    this.marks = rekey(this.marks);
    this.mapResults = rekey(this.mapResults);
    this.history = rekey(this.history);
    this.chartSeries = this.chartSeries
      .filter((s) => s.index !== index)
      .map((s) => ({ ...s, index: shift(s.index) }));
    this.changes = new Map();
    this.persistDevice();
    this.refreshPolling();
  }

  /** Puts a map register on the chart, and switches its reading on. */
  addToChart(index: number) {
    if (this.chartSeries.some((s) => s.index === index)) return;
    if (this.chartSeries.length >= MAX_SERIES) return;
    const slot = freeSlot(this.chartSeries);
    if (slot === null) return;
    this.chartSeries = [...this.chartSeries, { index, slot }];
    if (this.skipped.has(index)) this.toggleRead(index);
  }

  removeFromChart(index: number) {
    this.chartSeries = this.chartSeries.filter((s) => s.index !== index);
    const next = new Map(this.history);
    next.delete(index);
    this.history = next;
  }

  clearChart() {
    this.history = new Map();
  }

  /** The manual series reading this address the same way, if charted. */
  manualSeriesOf(s: Omit<ManualSeries, "slot">): ManualSeries | null {
    return this.manualSeries.find((x) => sameSeries(x, s)) ?? null;
  }

  /** Puts an address of the manual read on the Chart tab. */
  addManualToChart(s: Omit<ManualSeries, "slot">) {
    if (this.manualSeriesOf(s)) return;
    const slot = freeSlot(this.manualSeries);
    if (slot === null) return;
    this.manualSeries = [...this.manualSeries, { ...s, slot }];
  }

  removeManualFromChart(slot: number) {
    this.manualSeries = this.manualSeries.filter((s) => s.slot !== slot);
    const next = new Map(this.manualHistory);
    next.delete(slot);
    this.manualHistory = next;
  }

  clearManualChart() {
    this.manualHistory = new Map();
  }

  resetTimes() {
    this.times = NO_TIMES;
  }

  resetBreakdown() {
    this.breakdown = NO_BREAKDOWN;
  }

  clearLog() {
    this.log = [];
  }

  clearRaw() {
    this.exchanges = [];
  }
}

export const bench = new Bench();

/** A map with the registers added to it on this computer appended. */
function withCustom(map: DeviceMap, path: string): DeviceMap {
  const custom = loadCustom(path).filter(
    (c) => !map.registers.some((r) => r.name === c.name),
  );
  return custom.length
    ? { ...map, registers: [...map.registers, ...custom] }
    : map;
}
