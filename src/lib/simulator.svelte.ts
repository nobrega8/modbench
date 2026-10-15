// The simulated device on the Tools page. The backend runs it and holds its
// state; this mirrors that state in each window (the main one, and the
// simulator's own window when it is popped out) and keeps the recent
// activity while the page is not shown.

import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "./api";
import type { DeviceMap, Generator, Register, SimEvent } from "./types";

/** This page is the simulator's own window (see `sim_open_window`). */
export const IN_SIM_WINDOW =
  typeof location !== "undefined" &&
  new URLSearchParams(location.search).get("window") === "simulator";

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Requests kept per list, reads and writes apiece. */
const EVENT_LIMIT = 300;
/** Function codes that change values (also when they fail). */
const WRITES = new Set([5, 6, 15, 16, 22, 23]);

class Simulator {
  /** Port it listens on while running; null when stopped. */
  running = $state<number | null>(null);
  port = $state(5020);
  /** Listen on every network interface, not only this computer. */
  isPublic = $state(false);
  /** Answer only this unit ID; null answers all of them. */
  unit = $state<number | null>(null);
  /** Only the addresses of the loaded map exist. */
  strict = $state(true);
  /** What the memory was shaped after: a map's name, or null when empty. */
  loaded = $state<string | null>(null);
  /** The map it was shaped after, and the file it came from. */
  map = $state.raw<DeviceMap | null>(null);
  source = $state<string | null>(null);
  error = $state<string | null>(null);
  /** Recent requests, oldest first: reads, and writes kept apart so a
   * stream of reads does not push the writes out. */
  reads = $state.raw<SimEvent[]>([]);
  writes = $state.raw<SimEvent[]>([]);
  /** Bumped when a client writes, so the memory view reads again. */
  changes = $state(0);
  /** Values that change on their own while it runs; kept across stop and
   * start. */
  generators = $state.raw<Generator[]>([]);
  /** Why each generator cannot run (same order), or null. */
  problems = $state.raw<(string | null)[]>([]);
  /** Registers of the loaded map a generator can drive (not text). */
  registers = $state.raw<Register[]>([]);
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
      this.running = info.port;
      if (info.port !== null) {
        this.port = info.port;
        this.isPublic = info.public;
        this.unit = info.unit;
      }
      this.map = info.map;
      this.source = info.source;
      this.loaded = info.map
        ? info.source
          ? fileName(info.source)
          : `${info.map.meta.vendor} ${info.map.meta.model}`
        : null;
      if (info.map) this.strict = info.strict;
      this.registers =
        info.map?.registers.filter((r) => r.type !== "string") ?? [];
      this.generators = info.generators;
      this.problems = info.problems;
      this.windowOpen = info.window_open;
      this.changes++;
    } catch {
      // Not running under Tauri (tests): keep what is here.
    }
  }

  async start() {
    this.error = null;
    try {
      this.running = await api.simStart(this.port, this.isPublic, this.unit);
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async stop() {
    await api.simStop();
    this.running = null;
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
      await api.simLoad(map, this.strict, path);
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
    await api.simLoad(null, false, null);
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
      this.problems = await api.simSetGenerators(list);
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
