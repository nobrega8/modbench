// The network scan tool: its settings and results, kept while other
// pages are shown.

import { api, errorText } from "./api";
import { bench } from "./state.svelte";
import type { NetHit, NetProgress, NetScan } from "./types";

/** Orders hits by address, numerically. */
function byAddress(a: NetHit, b: NetHit): number {
  const n = (ip: string) =>
    ip.split(".").reduce((sum, part) => sum * 256 + Number(part), 0);
  return n(a.ip) - n(b.ip);
}

export type ScanMode = "fast" | "thorough";

/** Attempts after the first in thorough mode; fast mode makes none. */
export const THOROUGH_RETRIES = 1;

class NetScanner {
  targets = $state("192.168.1.0/24");
  port = $state(502);
  unit = $state(1);
  /** Fast: a short wait and no retries, for a first pass. Thorough: a
   * longer wait and a retry, for slow devices or networks. */
  mode = $state<ScanMode>("fast");
  /** Wait per address, in each mode. */
  waits = $state<Record<ScanMode, number>>({ fast: 150, thorough: 1000 });
  identify = $state(true);

  get timeoutMs(): number {
    return this.waits[this.mode];
  }

  set timeoutMs(ms: number) {
    this.waits = { ...this.waits, [this.mode]: ms };
  }

  get retries(): number {
    return this.mode === "fast" ? 0 : THOROUGH_RETRIES;
  }

  running = $state(false);
  progress = $state.raw<NetProgress | null>(null);
  /** What answered so far, by address; null before the first scan. */
  hits = $state.raw<NetHit[] | null>(null);
  error = $state<string | null>(null);
  /** The settings of the last scan, for its results and their export. */
  last = $state.raw<NetScan | null>(null);

  async start() {
    if (this.running) return;
    const scan: NetScan = {
      targets: this.targets,
      port: this.port,
      unit: this.unit,
      timeout_ms: this.timeoutMs,
      identify: this.identify,
      retries: this.retries,
      // The interface chosen for the connection of step 1
      local_address: bench.localAddress.trim() || null,
    };
    this.running = true;
    this.error = null;
    this.hits = [];
    this.progress = null;
    this.last = scan;
    try {
      this.hits = await api.scanNetwork(scan);
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.running = false;
    }
  }

  stop() {
    api.cancelScan().catch(() => {});
  }

  onHit(hit: NetHit) {
    if (!this.running) return;
    this.hits = [...(this.hits ?? []), hit].sort(byAddress);
  }

  onProgress(p: NetProgress) {
    if (this.running) this.progress = p;
  }
}

export const netScanner = new NetScanner();
