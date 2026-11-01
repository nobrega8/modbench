// The bus monitor tool: the serial line it listens to, what it heard, and
// whether it listens (see `monitor.rs`).

import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "./api";
import type { MonitorFilter } from "./bus-monitor";
import { t } from "./i18n.svelte.ts";
import type { MonitorEvent, Parity } from "./types";

/** The most requests the list keeps. */
const KEPT = 2000;

class BusMonitor {
  listening = $state(false);
  framing = $state<"rtu" | "ascii">("rtu");
  serialPort = $state("");
  baudrate = $state(9600);
  parity = $state<Parity>("N");
  dataBits = $state(8);
  stopBits = $state(1);
  error = $state<string | null>(null);
  /** Oldest first. */
  events = $state.raw<MonitorEvent[]>([]);
  filter = $state<MonitorFilter>({ unit: null, fn: null, problems: false });

  /** Follows what the line carries, and the port closing. Returns a
   * function that stops listening. */
  connect(): () => void {
    const stops = [
      listen<MonitorEvent>("monitor", (e) => {
        const next = [...this.events, e.payload];
        this.events = next.length > KEPT ? next.slice(-KEPT) : next;
      }),
      listen<string>("monitor-ended", (e) => {
        if (this.listening) this.error = e.payload;
        this.listening = false;
      }),
    ];
    return () => stops.forEach((p) => void p.then((s) => s()));
  }

  async start() {
    this.error = null;
    if (!this.serialPort) {
      this.error = t("sim.chooseport");
      return;
    }
    try {
      await api.monitorStart({
        kind: this.framing,
        port: this.serialPort,
        baudrate: this.baudrate,
        parity: this.parity,
        data_bits: this.dataBits,
        stop_bits: this.stopBits,
      });
      this.listening = true;
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async stop() {
    this.listening = false;
    await api.monitorStop();
  }

  clear() {
    this.events = [];
  }
}

export const busMonitor = new BusMonitor();
