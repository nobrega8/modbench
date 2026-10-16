import { describe, expect, it, vi } from "vitest";
import type { NetHit, NetScan } from "./types";

// The scan answers once the test says so; hits arrive before it ends.
let finish: (hits: NetHit[]) => void = () => {};
const asked: NetScan[] = [];
vi.mock("./api", () => ({
  errorText: (e: unknown) => String(e),
  api: {
    scanNetwork: (scan: NetScan) => {
      asked.push(scan);
      return new Promise<NetHit[]>((resolve) => (finish = resolve));
    },
    cancelScan: async () => {},
  },
}));

const { netScanner } = await import("./netscan.svelte");

const hit = (ip: string): NetHit => ({
  ip,
  port: 502,
  unit: 1,
  status: "modbus",
  response_ms: 3,
  response: "responds",
  identity: null,
});

describe("network scan", () => {
  it("shows hits as they arrive, in address order, then the final list", async () => {
    netScanner.targets = "10.0.0.0/24";
    const done = netScanner.start();
    expect(netScanner.running).toBe(true);
    expect(asked[0]).toMatchObject({
      targets: "10.0.0.0/24",
      port: 502,
      timeout_ms: 150,
      retries: 0,
    });
    netScanner.onHit(hit("10.0.0.20"));
    netScanner.onHit(hit("10.0.0.3"));
    expect(netScanner.hits?.map((h) => h.ip)).toEqual([
      "10.0.0.3",
      "10.0.0.20",
    ]);
    netScanner.onProgress({ done: 30, total: 254, found: 2 });
    finish([hit("10.0.0.3"), hit("10.0.0.20")]);
    await done;
    expect(netScanner.running).toBe(false);
    expect(netScanner.hits).toHaveLength(2);
    // Late events of a scan that ended change nothing
    netScanner.onHit(hit("10.0.0.99"));
    expect(netScanner.hits).toHaveLength(2);
  });
});

describe("scan modes", () => {
  it("keep a wait each, and only the thorough mode retries", () => {
    netScanner.mode = "thorough";
    expect([netScanner.timeoutMs, netScanner.retries]).toEqual([1000, 1]);
    netScanner.timeoutMs = 2000;
    netScanner.mode = "fast";
    expect([netScanner.timeoutMs, netScanner.retries]).toEqual([150, 0]);
    netScanner.mode = "thorough";
    expect(netScanner.timeoutMs).toBe(2000);
  });
});
