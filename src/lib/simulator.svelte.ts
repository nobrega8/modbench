// The simulated devices on the Tools page. The backend runs them and holds
// their state; this mirrors that state in each window (the main one, and the
// simulator's own window when it is popped out) and keeps the recent
// activity while the page is not shown.

import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "./api";
import type {
  DeviceMap,
  Generator,
  Parity,
  Register,
  SimDevice,
  SimEvent,
  Transport,
} from "./types";

/** This page is the simulator's own window (see `sim_open_window`). */
export const IN_SIM_WINDOW =
  typeof location !== "undefined" &&
  new URLSearchParams(location.search).get("window") === "simulator";

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Requests kept per list, reads and writes apiece. */
const EVENT_LIMIT = 300;
/** Function codes that change values (also when they fail). */
const WRITES = new Set([5, 6, 15, 16, 22, 23]);

/** How the simulator is reached: the same kinds as step 1 of Main. */
export type SimKind = Transport["kind"];

/** What a device is shaped after: a map's file name, or its vendor and
 * model; null when its memory is empty. */
export function deviceName(d: SimDevice): string | null {
  if (!d.map) return null;
  return d.source
    ? fileName(d.source)
    : `${d.map.meta.vendor} ${d.map.meta.model}`;
}

/** Where a running simulator listens, as short text. */
export function whereText(t: Transport): string {
  switch (t.kind) {
    case "tcp":
      return `${t.host === "0.0.0.0" ? "every interface" : t.host}:${t.port}`;
    case "rtuovertcp":
      return `${t.host === "0.0.0.0" ? "every interface" : t.host}:${t.port} · RTU over TCP`;
    default:
      return `${t.port} · ${t.kind.toUpperCase()} ${t.baudrate} ${t.data_bits}${t.parity}${t.stop_bits}`;
  }
}

class Simulator {
  /** Where it listens while running; null when stopped. */
  running = $state.raw<Transport | null>(null);
  /** How it listens when started. */
  kind = $state<SimKind>("tcp");
  port = $state(5020);
  /** Listen on every network interface, not only this computer. */
  isPublic = $state(false);
  serialPort = $state("");
  baudrate = $state(9600);
  parity = $state<Parity>("N");
  dataBits = $state(8);
  stopBits = $state(1);

  /** Every simulated device, as the backend has them. */
  devices = $state.raw<SimDevice[]>([]);
  /** The device shown and edited (an index into `devices`). */
  selected = $state(0);

  // The selected device, as the page edits it.
  /** The unit ID it answers; null answers every one no other has. */
  unit = $state<number | null>(null);
  /** Only the addresses of the loaded map exist. */
  strict = $state(true);
  /** What the memory was shaped after: a map's name, or null when empty. */
  loaded = $state<string | null>(null);
  /** The map it was shaped after, and the file it came from. */
  map = $state.raw<DeviceMap | null>(null);
  source = $state<string | null>(null);
  /** Values that change on their own while it runs; kept across stop and
   * start. */
  generators = $state.raw<Generator[]>([]);
  /** Why each generator cannot run (same order), or null. */
  problems = $state.raw<(string | null)[]>([]);
  /** Registers of the loaded map a generator can drive (not text). */
  registers = $state.raw<Register[]>([]);

  error = $state<string | null>(null);
  /** Recent requests, oldest first: reads, and writes kept apart so a
   * stream of reads does not push the writes out. */
  reads = $state.raw<SimEvent[]>([]);
  writes = $state.raw<SimEvent[]>([]);
  /** Bumped when a client writes, so the memory view reads again. */
  changes = $state(0);
  /** The simulator is shown in a window of its own. */
  windowOpen = $state(false);

  private pending: SimEvent[] = [];
  private flushQueued = false;

  /** Follows the backend: the simulator's state when another window (or
   * this one) changes it, its requests, and its own window opening and
   * closing. Returns a function that stops listening. */
  connect(): () => void {
    const stops = [
      listen<SimEvent>("sim", (e) => this.onEvent(e.payload)),
      listen("sim-state", () => this.sync()),
      listen<boolean>("sim-window", (e) => (this.windowOpen = e.payload)),
    ];
    this.sync();
    return () => stops.forEach((p) => p.then((stop) => stop()));
  }

  /** Takes the simulator's state from the backend. */
  async sync() {
    try {
      const info = await api.simInfo();
      this.running = info.listen;
      if (info.listen) this.listenFrom(info.listen);
      this.devices = info.devices;
      this.show(Math.min(this.selected, info.devices.length - 1));
      this.windowOpen = info.window_open;
    } catch {
      // Not running under Tauri (tests): keep what is here.
    }
  }

  /** Takes the listen settings of a running simulator. */
  private listenFrom(t: Transport) {
    this.kind = t.kind;
    if (t.kind === "tcp" || t.kind === "rtuovertcp") {
      this.port = t.port;
      this.isPublic = t.host === "0.0.0.0";
    } else {
      this.serialPort = t.port;
      this.baudrate = t.baudrate;
      this.parity = t.parity;
      this.dataBits = t.data_bits;
      this.stopBits = t.stop_bits;
    }
  }

  /** Shows a device: its map, generators and memory. */
  show(index: number) {
    const d = this.devices[index];
    if (!d) return;
    this.selected = index;
    this.unit = d.unit;
    this.map = d.map;
    this.source = d.source;
    this.loaded = deviceName(d);
    if (d.map) this.strict = d.strict;
    this.registers = d.map?.registers.filter((r) => r.type !== "string") ?? [];
    this.generators = d.generators;
    this.problems = d.problems;
    this.changes++;
  }

  /** Where to listen, from the settings. */
  listenSettings(): Transport {
    if (this.kind === "tcp" || this.kind === "rtuovertcp")
      return {
        kind: this.kind,
        host: this.isPublic ? "0.0.0.0" : "127.0.0.1",
        port: this.port,
      };
    return {
      kind: this.kind,
      port: this.serialPort,
      baudrate: this.baudrate,
      parity: this.parity,
      data_bits: this.dataBits,
      stop_bits: this.stopBits,
    };
  }

  async start() {
    this.error = null;
    if ((this.kind === "rtu" || this.kind === "ascii") && !this.serialPort) {
      this.error = "Choose a serial port first.";
      return;
    }
    try {
      this.running = await api.simStart(this.listenSettings());
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async stop() {
    await api.simStop();
    this.running = null;
  }

  /** Adds a device on the lowest unit ID no device has, and shows it. */
  async addDevice() {
    this.error = null;
    const used = new Set(this.devices.map((d) => d.unit));
    let unit = 1;
    while (used.has(unit) && unit < 247) unit++;
    try {
      const index = await api.simAddDevice(unit);
      await this.sync();
      this.show(index);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Removes the device shown; the last one stays. */
  async removeDevice() {
    if (this.devices.length < 2) return;
    this.error = null;
    try {
      await api.simRemoveDevice(this.selected);
      this.selected = Math.max(0, this.selected - 1);
      await this.sync();
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Changes the unit ID of the device shown. */
  async setUnit(unit: number | null) {
    this.error = null;
    try {
      await api.simSetUnit(this.selected, unit);
      this.unit = unit;
    } catch (e) {
      this.error = errorText(e);
    }
    await this.sync();
  }

  /** Shapes the memory after a map file: one open on the Main page, or one
   * from the library. Null empties the device. */
  async pick(path: string | null, open?: DeviceMap) {
    if (path === null) return this.clear();
    this.error = null;
    try {
      const map = open ?? (await api.openMap(path)).map;
      if (!map) {
        this.error =
          "This map has errors; open it on the Main page to see them.";
        return;
      }
      await this.load(map, fileName(path), path);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Loads the same map again, e.g. after "Only the map's addresses"
   * changed. Values go back to the map's references. */
  async reload() {
    if (this.map && this.source)
      await this.load(this.map, this.loaded ?? "", this.source);
  }

  /** Shapes the memory after a map (its reference values, its addresses). */
  async load(map: DeviceMap, name: string, path: string | null = null) {
    this.error = null;
    try {
      await api.simLoad(this.selected, map, this.strict, path);
      this.loaded = name;
      this.map = map;
      this.source = path;
      this.registers = map.registers.filter((r) => r.type !== "string");
      this.changes++;
      await this.setGenerators(this.generators);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Every address back to zero, all of them defined. */
  async clear() {
    await api.simLoad(this.selected, null, false, null);
    this.loaded = null;
    this.map = null;
    this.source = null;
    this.registers = [];
    this.changes++;
    await this.setGenerators(this.generators);
  }

  /** Replaces the generators; each is checked against the memory. */
  async setGenerators(list: Generator[]) {
    this.generators = list;
    try {
      this.problems = await api.simSetGenerators(this.selected, list);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  onEvent(event: SimEvent) {
    this.pending.push(event);
    if (this.flushQueued) return;
    this.flushQueued = true;
    requestAnimationFrame(() => {
      const wrote = this.pending.some((e) => e.wrote);
      const writes = this.pending.filter((e) => WRITES.has(e.function));
      const reads = this.pending.filter((e) => !WRITES.has(e.function));
      if (reads.length)
        this.reads = [...this.reads, ...reads].slice(-EVENT_LIMIT);
      if (writes.length)
        this.writes = [...this.writes, ...writes].slice(-EVENT_LIMIT);
      this.pending = [];
      this.flushQueued = false;
      if (wrote) this.changes++;
    });
  }

  clearEvents() {
    this.reads = [];
    this.writes = [];
  }
}

export const sim = new Simulator();
