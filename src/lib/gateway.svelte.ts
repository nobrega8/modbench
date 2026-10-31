// The gateway tool: where clients connect, the requests it passed on to the
// device of step 1, and whether it runs (see `gateway.rs`).

import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "./api";
import type { SimKind } from "./simulator.svelte.ts";
import { bench } from "./state.svelte.ts";
import { t } from "./i18n.svelte.ts";
import type { GatewayEvent, Parity, Transport } from "./types";

/** The most requests the log keeps. */
const KEPT = 500;

class Gateway {
  /** Where it listens while running; null when stopped. */
  running = $state.raw<Transport | null>(null);
  kind = $state<SimKind>("tcp");
  /** Above 1023, which every system lets a program use. */
  port = $state(5502);
  isPublic = $state(false);
  serialPort = $state("");
  baudrate = $state(9600);
  parity = $state<Parity>("N");
  dataBits = $state(8);
  stopBits = $state(1);
  /** Unit ID every request goes to; null keeps the client's. */
  unit = $state<number | null>(null);
  readOnly = $state(false);
  error = $state<string | null>(null);
  /** Newest first. */
  events = $state.raw<GatewayEvent[]>([]);
  total = $state(0);
  failed = $state(0);

  /** Follows the requests it passes on. Returns a function that stops
   * listening. */
  connect(): () => void {
    const stop = listen<GatewayEvent>("gateway", (e) =>
      this.onEvent(e.payload),
    );
    return () => void stop.then((s) => s());
  }

  private onEvent(e: GatewayEvent) {
    this.events = [e, ...this.events].slice(0, KEPT);
    this.total++;
    if (e.exception !== null) this.failed++;
  }

  clear() {
    this.events = [];
    this.total = 0;
    this.failed = 0;
  }

  /** Where to listen, from the settings. */
  listenSettings(): Transport {
    if (this.kind !== "rtu" && this.kind !== "ascii")
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
      this.error = t("sim.chooseport");
      return;
    }
    try {
      this.running = await api.gatewayStart({
        listen: this.listenSettings(),
        upstream: bench.config(),
        unit: this.unit,
        read_only: this.readOnly,
      });
    } catch (e) {
      this.error = errorText(e);
    }
  }

  async stop() {
    await api.gatewayStop();
    this.running = null;
  }
}

export const gateway = new Gateway();
