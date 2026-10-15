import { beforeEach, describe, expect, it, vi } from "vitest";

// A small in-memory localStorage, installed before the app state is loaded.
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => void store.set(k, v),
  removeItem: (k: string) => void store.delete(k),
  clear: () => store.clear(),
  key: (i: number) => [...store.keys()][i] ?? null,
  get length() {
    return store.size;
  },
});

// Nothing in these tests may reach a device: every backend call is recorded.
const calls: string[] = [];
vi.mock("@tauri-apps/plugin-dialog", () => ({ ask: vi.fn(async () => true) }));
vi.mock("./api", () => {
  const record = (name: string) => async () => {
    calls.push(name);
    return undefined;
  };
  return {
    errorText: (e: unknown) => String(e),
    api: new Proxy(
      {},
      {
        get: (_t, name: string) =>
          name === "libraryFolder"
            ? async () => ""
            : name === "libraryList"
              ? async () => []
              : record(name),
      },
    ),
  };
});

const { bench } = await import("./state.svelte");
const { settings } = await import("./settings.svelte");
import type { DeviceMap, Register } from "./types";

function register(name: string, address: number): Register {
  return {
    name,
    group: "",
    area: "holding",
    address,
    type: "uint16",
    length: null,
    scale: 1,
    scale_factor: null,
    offset: 0,
    unit: "",
    access: "R",
    write_min: null,
    write_max: null,
    reference: null,
    byte_order: null,
    bit: null,
    bits: null,
    enum: [],
    enum_ref: null,
    description: "",
    extra: [],
  } as unknown as Register;
}

// Registers keep their address when the map around them changes.
const ADDRESS: Record<string, number> = { V: 0, A: 1, W: 2, PF: 3, Status: 10 };

function openMap(path = "/maps/meter.mbmap", names = ["V", "A", "W", "PF"]) {
  const map = {
    meta: { vendor: "Acme", model: "M1", byte_order: null },
    registers: names.map((n) => register(n, ADDRESS[n])),
  } as unknown as DeviceMap;
  const existing = bench.maps.findIndex((m) => m.path === path);
  if (existing >= 0)
    bench.maps[existing] = { path, map, warnings: [], version: 2 };
  else bench.maps.push({ path, map, warnings: [], version: 2 });
  bench.selectMap(existing >= 0 ? existing : bench.maps.length - 1);
}

/** What the page does when the unit ID or the endpoint changes. */
function useDevice(unit: number, host = "10.0.0.1") {
  bench.unitId = unit;
  bench.host = host;
  bench.restoreDevice();
}

beforeEach(() => {
  store.clear();
  calls.length = 0;
  settings.rememberDevices = true;
  bench.maps.length = 0;
  bench.activeMap = null;
  bench.running = false;
  bench.transport = "tcp";
  bench.tcpPort = 502;
  bench.orderOverride = null;
  bench.loop = true;
  bench.unitId = 1;
  bench.host = "10.0.0.1";
});

describe("remembered choices per device", () => {
  it("keep each unit ID's choices apart", () => {
    openMap();
    useDevice(1);
    bench.toggleRead(0); // unit 1: V off
    useDevice(2);
    // a device seen for the first time starts from the map's last choices
    expect([...bench.skipped]).toEqual([0]);
    bench.toggleRead(0); // unit 2: V back on
    bench.toggleRead(2); // unit 2: W off
    expect([...bench.skipped]).toEqual([2]);

    useDevice(1);
    expect([...bench.skipped]).toEqual([0]);
    useDevice(2);
    expect([...bench.skipped]).toEqual([2]);
  });

  it("keep each endpoint's choices apart", () => {
    openMap();
    useDevice(1, "10.0.0.1");
    bench.toggleRead(1);
    useDevice(1, "10.0.0.9");
    bench.toggleRead(3);
    useDevice(1, "10.0.0.1");
    expect([...bench.skipped]).toEqual([1]);
  });

  it("do not carry a byte order override to another device", () => {
    openMap();
    useDevice(1);
    bench.setOrderOverride("CDAB");
    useDevice(2);
    expect(bench.orderOverride).toBeNull();
    useDevice(1);
    expect(bench.orderOverride).toBe("CDAB");
  });

  it("remember Loop for the device", () => {
    openMap();
    useDevice(1);
    bench.setLoop(false);
    useDevice(2);
    bench.setLoop(true);
    useDevice(1);
    expect(bench.loop).toBe(false);
  });

  it("keep the time between loops with the connection", () => {
    bench.setLoopMs(1500.4);
    expect(bench.loopMs).toBe(1500);
    expect(
      JSON.parse(localStorage.getItem("modbench.connection") ?? "{}").loopMs,
    ).toBe(1500);
    bench.setLoopMs(-5);
    expect(bench.loopMs).toBe(0);
  });

  it("survive the map being edited", () => {
    openMap("/maps/meter.mbmap", ["V", "A", "W", "PF"]);
    useDevice(1);
    bench.toggleRead(2); // W off
    // the same file now has a register in front of the others
    openMap("/maps/meter.mbmap", ["Status", "V", "A", "W", "PF"]);
    useDevice(1);
    expect([...bench.skipped]).toEqual([3]);
  });

  it("start with everything on for a map never seen", () => {
    openMap("/maps/new.mbmap");
    useDevice(1);
    expect(bench.skipped.size).toBe(0);
    expect(bench.orderOverride).toBeNull();
  });

  it("are not saved or restored when the setting is off", () => {
    settings.rememberDevices = false;
    openMap();
    useDevice(1);
    bench.toggleRead(0);
    const saved = [...store.keys()].filter((k) =>
      /^modbench\.(device|map):/.test(k),
    );
    expect(saved).toEqual([]);
    useDevice(2);
    expect(bench.skipped.size).toBe(0);
  });

  it("do not change while a test is running", () => {
    openMap();
    useDevice(1);
    bench.toggleRead(0);
    bench.running = true;
    const before = [...bench.skipped];
    useDevice(2);
    expect([...bench.skipped]).toEqual(before);
  });

  it("never talk to a device when restored", () => {
    openMap();
    useDevice(1);
    bench.toggleRead(0);
    bench.setOrderOverride("DCBA");
    useDevice(2);
    useDevice(1);
    openMap("/maps/other.mbmap");
    useDevice(1);
    expect(calls).toEqual([]);
  });
});

describe("read-only mode", () => {
  it("refuses a manual write command before connecting", async () => {
    calls.length = 0;
    settings.readOnly = true;
    bench.running = false;
    bench.source = "manual";
    bench.fc = 6;
    await bench.start();
    expect(calls).toEqual([]);
    expect(bench.running).toBe(false);
    expect(bench.status?.text).toMatch(/Read-only mode/);
    settings.readOnly = false;
  });
});
