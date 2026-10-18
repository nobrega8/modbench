// The demo: the app's own simulator plays a made-up meter on this
// computer, and the Main page reads it, with values that move. Nothing the
// demo changes is remembered, and leaving it puts the connection and the
// simulator back as they were.

import { api, errorText } from "./api";
import { settings } from "./settings.svelte";
import { sim } from "./simulator.svelte";
import { bench, CONNECTION_FIELDS } from "./state.svelte";
import type { DeviceMap, Generator } from "./types";
import { t } from "./i18n.svelte.ts";

/** The path the demo's map is opened under; no file has it. */
export const DEMO_PATH = "modbench:demo";
/** Ports tried for the simulator, in order. */
const PORTS = [5020, 5021, 5022, 5023, 5502];

/** Values that move, so the chart and the table have something to show. */
export const DEMO_GENERATORS: Generator[] = [
  {
    register: "Voltage L1",
    kind: "sine",
    offset: 230,
    amplitude: 4,
    period_s: 20,
    every_ms: 500,
  },
  {
    register: "Voltage L2",
    kind: "sine",
    offset: 231,
    amplitude: 3,
    period_s: 27,
    every_ms: 500,
  },
  {
    register: "Voltage L3",
    kind: "sine",
    offset: 229,
    amplitude: 5,
    period_s: 33,
    every_ms: 500,
  },
  { register: "Current L1", kind: "random", min: 10, max: 14, every_ms: 1000 },
  {
    register: "Active power",
    kind: "random",
    min: 7000,
    max: 9500,
    every_ms: 1000,
  },
  {
    register: "Energy",
    kind: "increment",
    step: 1,
    min: 125000,
    max: 999999,
    every_ms: 2000,
  },
  {
    register: "Temperature",
    kind: "sine",
    offset: 31,
    amplitude: 1.5,
    period_s: 60,
    every_ms: 1000,
  },
];

/** What the demo replaced, to put it back. */
interface Saved {
  connection: Record<string, unknown>;
  tab: typeof bench.tab;
  activeMap: number | null;
  source: typeof bench.source;
  simDevice: {
    map: DeviceMap | null;
    source: string | null;
    name: string | null;
    strict: boolean;
    generators: Generator[];
  } | null;
  simPort: number;
  simKind: typeof sim.kind;
  simPublic: boolean;
}

class Demo {
  /** Steps of the guide shown while the demo runs. */
  active = $state(false);
  starting = $state(false);
  error = $state<string | null>(null);
  private saved: Saved | null = null;

  /** Whether to offer the demo now: first start, nothing set up yet. */
  get offered(): boolean {
    if (!settings.offerDemo || this.active) return false;
    try {
      return localStorage.getItem("modbench.connection") === null;
    } catch {
      return false;
    }
  }

  /** Stops offering it at start. */
  dismiss(forever: boolean) {
    if (forever) {
      settings.offerDemo = false;
      settings.save();
    }
    this.dismissed = true;
  }

  /** Hides the offer for this session. */
  dismissed = $state(false);

  async start() {
    if (this.active || this.starting) return;
    this.error = null;
    if (bench.running) {
      this.error = t("demo.stopfirst");
      return;
    }
    if (sim.running) {
      this.error =
        "The simulator is running with your own setup. Stop it in Tools › Simulator to try the demo.";
      return;
    }
    this.starting = true;
    const d = sim.devices[sim.selected];
    const connection: Record<string, unknown> = {};
    for (const f of CONNECTION_FIELDS) connection[f] = bench[f];
    this.saved = {
      connection,
      tab: bench.tab,
      activeMap: bench.activeMap,
      source: bench.source,
      simDevice: d
        ? {
            map: d.map,
            source: d.source,
            name: sim.loaded,
            strict: sim.strict,
            generators: d.generators,
          }
        : null,
      simPort: sim.port,
      simKind: sim.kind,
      simPublic: sim.isPublic,
    };
    bench.demo = true;
    try {
      const parsed = await api.demoMap();
      if (!parsed.map) throw new Error("The demo map could not be read.");
      const map = parsed.map;
      sim.kind = "tcp";
      sim.isPublic = false;
      sim.strict = true;
      await sim.load(map, "Demo meter (simulated)");
      await sim.setGenerators(DEMO_GENERATORS);
      let port: number | null = null;
      for (const p of PORTS) {
        sim.port = p;
        await sim.start();
        if (sim.running) {
          port = p;
          break;
        }
      }
      if (port === null)
        throw new Error(sim.error ?? "No free port for the simulator.");
      bench.transport = "tcp";
      bench.host = "127.0.0.1";
      bench.tcpPort = port;
      bench.localAddress = "";
      bench.unitId = 1;
      bench.timeoutMs = 1000;
      bench.delayMs = 10;
      bench.loopMs = 500;
      bench.openMapObject(map, DEMO_PATH);
      bench.loop = true;
      bench.page = "main";
      this.active = true;
      await bench.start();
      const reg = map.registers.findIndex((r) => r.name === "Voltage L1");
      if (reg >= 0) bench.addToChart(reg);
      bench.tab = "map";
    } catch (e) {
      this.error = errorText(e);
      await this.exit();
    } finally {
      this.starting = false;
    }
  }

  /** Leaves the demo: stops reading, closes its map, stops the simulator
   * and puts back the connection and the simulator's device. */
  async exit() {
    const saved = this.saved;
    this.active = false;
    if (bench.running) await bench.stop();
    bench.closeMapPath(DEMO_PATH);
    if (sim.running) await sim.stop();
    if (saved) {
      for (const [k, v] of Object.entries(saved.connection))
        (bench as unknown as Record<string, unknown>)[k] = v;
      bench.source = saved.source;
      bench.tab = saved.tab;
      if (saved.activeMap !== null && bench.maps[saved.activeMap])
        bench.selectMap(saved.activeMap);
      sim.port = saved.simPort;
      sim.kind = saved.simKind;
      sim.isPublic = saved.simPublic;
      const d = saved.simDevice;
      if (d?.map) {
        sim.strict = d.strict;
        await sim.load(d.map, d.name ?? "", d.source);
      } else await sim.clear();
      await sim.setGenerators(d?.generators ?? []);
    }
    this.saved = null;
    bench.demo = false;
  }
}

export const demo = new Demo();
