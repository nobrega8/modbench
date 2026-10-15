// The simulated device on the Tools page. The backend runs it; this keeps
// its settings and recent activity while the page is not shown.

import { api, errorText } from "./api";
import type { DeviceMap, Generator, Register, SimEvent } from "./types";

const EVENT_LIMIT = 300;

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
  events = $state.raw<SimEvent[]>([]);
  /** Bumped when a client writes, so the memory view reads again. */
  changes = $state(0);
  /** Values that change on their own while it runs; kept across stop and
   * start. */
  generators = $state.raw<Generator[]>([]);
  /** Why each generator cannot run (same order), or null. */
  problems = $state.raw<(string | null)[]>([]);
  /** Registers of the loaded map a generator can drive (not text). */
  registers = $state.raw<Register[]>([]);

  private pending: SimEvent[] = [];
  private flushQueued = false;

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
      await this.load(map, path.split(/[\\/]/).pop() ?? path, path);
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
      await api.simLoad(map, this.strict);
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
    await api.simLoad(null, false);
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
      this.events = [...this.events, ...this.pending].slice(-EVENT_LIMIT);
      this.pending = [];
      this.flushQueued = false;
      if (wrote) this.changes++;
    });
  }

  clearEvents() {
    this.events = [];
  }
}

export const sim = new Simulator();
