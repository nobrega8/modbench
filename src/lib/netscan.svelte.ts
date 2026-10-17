// The network scan tool: its settings and results, kept while other
// pages are shown.

import { api, errorText } from "./api";
import type { NetHit, NetProgress, NetScan } from "./types";

/** Orders hits by address, numerically. */
function byAddress(a: NetHit, b: NetHit): number {
  const n = (ip: string) =>
    ip.split(".").reduce((sum, part) => sum * 256 + Number(part), 0);
  return n(a.ip) - n(b.ip);
}

class NetScanner {
  targets = $state("192.168.1.0/24");
  port = $state(502);
  unit = $state(1);
  timeoutMs = $state(500);
  identify = $state(true);

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
