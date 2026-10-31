// An assistant driving the running window through `modbench-cli mcp` (the
// control channel in src-tauri/src/gui_control.rs). The requests change the
// same state the operator sees; this module is where the guard rails are:
// the operator can pause it, a running test is never reconfigured under their
// hands, nothing it does writes to a device, and what it changed is listed.

import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "./api";
import { ADDRESS_FORMATS, type AddressFormat } from "./format";
import { settings } from "./settings.svelte";
import { bench, type Page, type Tab, type ToolId } from "./state.svelte";
import type { Area, ByteOrder, Parity } from "./types";

const MAX_ACTIVITY = 50;
/** The registers get_state lists at most (the rest are counted). */
const MAX_REGISTERS = 300;

export interface Activity {
  time: number;
  text: string;
  /** The operator's pause, or the rules, turned the request down. */
  refused: boolean;
}

const TRANSPORTS = ["tcp", "rtuovertcp", "udp", "rtuoverudp", "rtu"] as const;
const PAGES: Page[] = ["main", "tools", "library", "settings"];
const TABS: Tab[] = ["registers", "map", "chart", "write", "raw", "log"];
const TOOLS: ToolId[] = [
  "simulator",
  "gateway",
  "identify",
  "addresses",
  "network",
  "request",
  "decoder",
  "frames",
  "stats",
  "alarms",
  "player",
];
const ORDERS: ByteOrder[] = ["ABCD", "DCBA", "BADC", "CDAB"];
const READ_AREAS: Record<number, Area> = {
  1: "coil",
  2: "discrete",
  3: "holding",
  4: "input",
};

/** Raised for a request the rules turn down; its text goes to the assistant. */
class Refused extends Error {}

type Args = Record<string, unknown>;

function oneOf<T extends string>(
  value: unknown,
  list: readonly T[],
  what: string,
): T {
  if (typeof value === "string" && (list as readonly string[]).includes(value))
    return value as T;
  throw new Error(`${what} must be one of ${list.join(", ")}`);
}

function int(value: unknown, what: string, min: number, max: number): number {
  if (
    typeof value !== "number" ||
    !Number.isInteger(value) ||
    value < min ||
    value > max
  )
    throw new Error(`${what} must be an integer from ${min} to ${max}`);
  return value;
}

class AssistantControl {
  /** Assistants connected to the channel. */
  clients = $state(0);
  /** The operator paused it: every request is turned down. */
  paused = $state(false);
  activity = $state.raw<Activity[]>([]);
  /** Where the channel listens, while it is on. */
  port = $state<number | null>(null);
  error = $state<string | null>(null);

  private log(text: string, refused = false) {
    this.activity = [
      { time: Date.now(), text, refused },
      ...this.activity,
    ].slice(0, MAX_ACTIVITY);
  }

  /** Turns the channel on or off to match the setting. */
  async apply() {
    try {
      const s = await api.guiControlEnable(settings.assistantControl);
      this.port = s.port;
      this.clients = s.clients;
      this.error = null;
    } catch (e) {
      this.port = null;
      this.error = errorText(e);
    }
    if (!settings.assistantControl) this.paused = false;
  }

  async disconnect() {
    try {
      const s = await api.guiControlDisconnect();
      this.port = s.port;
      this.clients = s.clients;
      this.log("Operator disconnected the assistants");
    } catch (e) {
      this.error = errorText(e);
    }
  }

  setPaused(on: boolean) {
    this.paused = on;
    this.log(
      on
        ? "Operator paused assistant control"
        : "Operator resumed assistant control",
    );
  }

  /** Listens for requests from the channel and answers them. */
  listen() {
    const stops = [
      listen<{ id: number; tool: string; args: Args }>(
        "gui-control-request",
        (e) => this.handle(e.payload.id, e.payload.tool, e.payload.args ?? {}),
      ),
      listen<number>("gui-control-clients", (e) => {
        if (e.payload > this.clients) this.log("An assistant connected");
        else if (e.payload < this.clients)
          this.log("An assistant disconnected");
        this.clients = e.payload;
      }),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  }

  private async handle(id: number, tool: string, args: Args) {
    try {
      const value = await this.run(tool, args);
      await api.guiControlReply(id, true, value);
    } catch (e) {
      const text = e instanceof Error ? e.message : errorText(e);
      if (e instanceof Refused) this.log(`Refused ${tool}: ${text}`, true);
      await api.guiControlReply(id, false, text);
    }
  }

  /** Looks up registers of the active map by name or index. */
  private registers(list: unknown): number[] {
    const regs = bench.map?.map.registers;
    if (!regs) throw new Error("no map is open");
    if (!Array.isArray(list) || list.length === 0)
      throw new Error("registers must be a non-empty list of names or indexes");
    return list.map((r) => {
      const index =
        typeof r === "number"
          ? r
          : regs.findIndex(
              (x) => x.name.toLowerCase() === String(r).toLowerCase(),
            );
      if (!Number.isInteger(index) || index < 0 || index >= regs.length)
        throw new Error(`no register ${JSON.stringify(r)} in the open map`);
      return index;
    });
  }

  /** What the rules say about changing the window right now. */
  private guard(tool: string, changesTest: boolean) {
    if (this.paused)
      throw new Refused(
        "the operator paused assistant control; ask them to resume it",
      );
    if (changesTest && bench.running)
      throw new Refused(
        "a test is running; the operator or gui_run with action stop must stop it first",
      );
    void tool;
  }

  private async run(tool: string, args: Args): Promise<unknown> {
    switch (tool) {
      case "get_state":
        return this.state();

      case "list_library": {
        const q = String(args.filter ?? "").toLowerCase();
        return bench.library
          .filter((e) => !e.error)
          .filter((e) => `${e.vendor} ${e.model}`.toLowerCase().includes(q))
          .map((e) => ({
            path: e.path,
            vendor: e.vendor,
            model: e.model,
            registers: e.registers,
            status: e.status,
          }));
      }

      case "set_connection": {
        this.guard(tool, true);
        const changed: string[] = [];
        const set = <K extends keyof typeof bench>(
          key: K,
          value: (typeof bench)[K],
        ) => {
          bench[key] = value;
          changed.push(`${String(key)} ${String(value)}`);
        };
        if ("transport" in args)
          set("transport", oneOf(args.transport, TRANSPORTS, "transport"));
        if ("host" in args) set("host", String(args.host));
        if ("port" in args) set("tcpPort", int(args.port, "port", 1, 65535));
        if ("serial_port" in args) set("serialPort", String(args.serial_port));
        if ("framing" in args)
          set(
            "framing",
            oneOf(args.framing, ["rtu", "ascii"] as const, "framing"),
          );
        if ("baud" in args)
          set("baudrate", int(args.baud, "baud", 50, 4000000));
        if ("parity" in args)
          set(
            "parity",
            oneOf(args.parity, ["N", "E", "O"] as const, "parity") as Parity,
          );
        if ("data_bits" in args)
          set("dataBits", int(args.data_bits, "data_bits", 5, 8));
        if ("stop_bits" in args)
          set("stopBits", int(args.stop_bits, "stop_bits", 1, 2));
        if ("unit" in args) set("unitId", int(args.unit, "unit", 0, 255));
        if ("timeout_ms" in args)
          set("timeoutMs", int(args.timeout_ms, "timeout_ms", 50, 600000));
        this.log(
          changed.length
            ? `Set connection: ${changed.join(", ")}`
            : "Set connection (nothing changed)",
        );
        return this.state();
      }

      case "open_map": {
        this.guard(tool, true);
        const path = String(args.path ?? "");
        if (!bench.library.some((e) => e.path === path && !e.error))
          throw new Error(
            "path is not a map of the library; list them with gui_list_library",
          );
        await bench.openMap(path);
        if (bench.mapError)
          throw new Error(
            `the map has errors: ${bench.mapError.diagnostics.map((d) => d.message).join("; ")}`,
          );
        this.log(
          `Opened map ${bench.map?.map.meta.vendor} ${bench.map?.map.meta.model}`,
        );
        return this.state();
      }

      case "set_map_view": {
        this.guard(tool, false);
        if (!bench.map) throw new Error("no map is open");
        const done: string[] = [];
        if ("address_format" in args) {
          const f = oneOf(
            args.address_format,
            ADDRESS_FORMATS.map((x) => x.id),
            "address_format",
          );
          settings.addressFormat = f as AddressFormat;
          settings.save();
          done.push(`address notation ${f}`);
        }
        if ("byte_order" in args) {
          const o = oneOf(
            args.byte_order,
            ["map", ...ORDERS] as const,
            "byte_order",
          );
          bench.setOrderOverride(o === "map" ? null : (o as ByteOrder));
          done.push(`byte order ${o}`);
        }
        if ("read_all" in args) {
          bench.setAllRead(args.read_all === true);
          done.push(
            args.read_all === true ? "read all registers" : "read no register",
          );
        }
        for (const [key, on] of [
          ["read_registers", true],
          ["skip_registers", false],
        ] as const) {
          if (!(key in args)) continue;
          for (const i of this.registers(args[key])) {
            if (bench.skipped.has(i) === on) bench.toggleRead(i);
          }
          done.push(
            `${on ? "read" : "skip"} ${(args[key] as unknown[]).length} register(s)`,
          );
        }
        this.log(
          done.length
            ? `Map view: ${done.join(", ")}`
            : "Map view (nothing changed)",
        );
        return this.state();
      }

      case "set_manual_read": {
        this.guard(tool, true);
        const done: string[] = [];
        if ("fc" in args) {
          const fc = int(args.fc, "fc", 1, 4);
          bench.fc = fc;
          done.push(`FC${fc.toString().padStart(2, "0")}`);
        }
        if ("address" in args) {
          bench.address = int(args.address, "address", 0, 65535);
          done.push(`address ${bench.address}`);
        }
        if ("quantity" in args) {
          bench.setQuantity(int(args.quantity, "quantity", 1, 2000));
          done.push(`quantity ${bench.quantity}`);
        }
        bench.source = "manual";
        this.log(`Manual read: ${done.join(", ") || "nothing changed"}`);
        return this.state();
      }

      case "run": {
        const action = oneOf(args.action, ["start", "stop"] as const, "action");
        // Stopping is always allowed, unless paused.
        this.guard(tool, false);
        if ("loop" in args) bench.setLoop(args.loop === true);
        if ("interval_ms" in args)
          bench.setLoopMs(int(args.interval_ms, "interval_ms", 0, 3600000));
        if (action === "stop") {
          await bench.stop();
          this.log("Stopped reading");
          return this.state();
        }
        if (bench.running) return this.state();
        if (bench.source === "manual" && !(bench.fc in READ_AREAS))
          throw new Refused(
            "the manual request is a write; the assistant only starts reads",
          );
        if (bench.source === "map" && !bench.map)
          throw new Error("no map is open");
        // start() runs until the test ends; the answer is the state right after it began.
        void bench.start();
        await new Promise((r) => setTimeout(r, 600));
        this.log(`Started reading${bench.loop ? " (loop)" : ""}`);
        return this.state();
      }

      case "chart": {
        this.guard(tool, false);
        const action = oneOf(
          args.action,
          ["add", "remove", "clear"] as const,
          "action",
        );
        if (!bench.map) throw new Error("no map is open");
        if (action === "clear") {
          for (const s of [...bench.chartSeries])
            bench.removeFromChart(s.index);
        } else {
          for (const i of this.registers(args.registers)) {
            if (action === "add") bench.addToChart(i);
            else bench.removeFromChart(i);
          }
        }
        bench.tab = "chart";
        this.log(`Chart: ${action}`);
        return this.state();
      }

      case "navigate": {
        this.guard(tool, false);
        if ("page" in args) bench.page = oneOf(args.page, PAGES, "page");
        if ("tab" in args) {
          bench.tab = oneOf(args.tab, TABS, "tab");
          bench.page = "main";
        }
        if ("tool" in args) {
          bench.tool = oneOf(args.tool, TOOLS, "tool");
          bench.page = "tools";
        }
        this.log(
          `Showing ${bench.page}${bench.page === "main" ? ` / ${bench.tab}` : ""}`,
        );
        return this.state();
      }

      default:
        throw new Error(`unknown tool ${tool}`);
    }
  }

  /** The visible state, for the assistant to read before it changes anything. */
  state() {
    const open = bench.map;
    const regs = open?.map.registers ?? [];
    return {
      paused: this.paused,
      read_only_mode: settings.readOnly,
      screen: { page: bench.page, tab: bench.tab, tool: bench.tool },
      connection: {
        transport: bench.transport,
        host: bench.host,
        port: bench.tcpPort,
        serial_port: bench.serialPort,
        framing: bench.framing,
        baud: bench.baudrate,
        parity: bench.parity,
        data_bits: bench.dataBits,
        stop_bits: bench.stopBits,
        unit: bench.unitId,
        timeout_ms: bench.timeoutMs,
        connected: bench.connected,
      },
      test: {
        running: bench.running,
        source: bench.source,
        loop: bench.loop,
        interval_ms: bench.loopMs,
        cycle: bench.lastCycle?.cycle ?? null,
        status: bench.status,
        counters: bench.counters,
      },
      manual_read: {
        fc: bench.fc,
        address: bench.address,
        quantity: bench.quantity,
      },
      address_notation: settings.addressFormat,
      byte_order_override: bench.orderOverride,
      open_maps: bench.maps.map((m, i) => ({
        path: m.path,
        vendor: m.map.meta.vendor,
        model: m.map.meta.model,
        active: i === bench.activeMap,
      })),
      chart: bench.chartSeries.map((s) => regs[s.index]?.name ?? s.index),
      registers: open
        ? {
            count: regs.length,
            listed: Math.min(regs.length, MAX_REGISTERS),
            items: regs.slice(0, MAX_REGISTERS).map((r, index) => {
              const res = bench.mapResults.get(index);
              return {
                index,
                name: r.name,
                group: r.group || undefined,
                area: r.area,
                address: r.address,
                unit: r.unit || undefined,
                read: !bench.skipped.has(index),
                value: res?.value ?? null,
                error: res?.error ?? undefined,
              };
            }),
          }
        : null,
    };
  }
}

export const assistant = new AssistantControl();
