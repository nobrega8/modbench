// The simulated device on the Tools page. The backend runs it; this keeps
// its settings and recent activity while the page is not shown.

import { api, errorText } from "./api";
import type { DeviceMap, SimEvent } from "./types";

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
  error = $state<string | null>(null);
  events = $state.raw<SimEvent[]>([]);
  /** Bumped when a client writes, so the memory view reads again. */
  changes = $state(0);

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

  /** Shapes the memory after a map (its reference values, its addresses). */
  async load(map: DeviceMap, name: string) {
    this.error = null;
    try {
      await api.simLoad(map, this.strict);
      this.loaded = name;
      this.changes++;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  /** Every address back to zero, all of them defined. */
  async clear() {
    await api.simLoad(null, false);
    this.loaded = null;
    this.changes++;
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
