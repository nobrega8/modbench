import { describe, expect, it } from "vitest";
import {
  endpointOf,
  forgetAll,
  idsOf,
  indexesOf,
  loadMapDefault,
  loadProfile,
  profileKey,
  registerId,
  saveMapDefault,
  saveProfile,
  type KeyValueStore,
} from "./profiles";
import type { Register } from "./types";

class MemoryStore implements KeyValueStore {
  data = new Map<string, string>();
  get length() {
    return this.data.size;
  }
  key(i: number) {
    return [...this.data.keys()][i] ?? null;
  }
  getItem(k: string) {
    return this.data.get(k) ?? null;
  }
  setItem(k: string, v: string) {
    this.data.set(k, v);
  }
  removeItem(k: string) {
    this.data.delete(k);
  }
}

function reg(name: string, address: number, extra: Partial<Register> = {}) {
  return {
    name,
    group: "",
    area: "holding",
    address,
    type: "uint16",
    bit: null,
    bits: null,
    ...extra,
  } as Register;
}

const regs = [
  reg("A", 0),
  reg("B", 1),
  reg("Flag", 2, { type: "bool", bit: 3 }),
];

describe("register ids", () => {
  it("do not depend on the position in the map", () => {
    const moved = [regs[2], regs[0], regs[1]];
    const ids = idsOf(new Set([1]), regs); // B
    expect([...indexesOf(ids, moved)]).toEqual([2]);
  });

  it("ignore registers the map no longer has", () => {
    const ids = idsOf(new Set([0, 1]), regs);
    expect([...indexesOf(ids, [regs[0]])]).toEqual([0]);
  });

  it("tell alternative readings of one address apart", () => {
    expect(registerId(reg("Code", 5))).not.toBe(
      registerId(reg("Code", 5, { type: "string" })),
    );
  });
});

describe("device keys", () => {
  const c = {
    transport: "tcp" as const,
    host: " Meter.LAN ",
    tcpPort: 502,
    serialPort: "",
  };

  it("separate devices by endpoint and unit", () => {
    const a = profileKey("/maps/x.mbmap", endpointOf(c), 1);
    expect(profileKey("/maps/x.mbmap", endpointOf(c), 2)).not.toBe(a);
    expect(
      profileKey("/maps/x.mbmap", endpointOf({ ...c, host: "10.0.0.5" }), 1),
    ).not.toBe(a);
    expect(profileKey("/maps/y.mbmap", endpointOf(c), 1)).not.toBe(a);
  });

  it("ignore case and spaces in the host", () => {
    expect(endpointOf(c)).toBe("tcp:meter.lan:502");
  });

  it("use the serial port for RTU, not the TCP fields", () => {
    const rtu = { ...c, transport: "rtu" as const, serialPort: "COM3" };
    expect(endpointOf(rtu)).toBe("rtu:COM3");
    expect(endpointOf({ ...rtu, host: "other" })).toBe("rtu:COM3");
  });
});

describe("profiles", () => {
  it("round trip and stay inside their own key", () => {
    const store = new MemoryStore();
    const one = profileKey("m", "tcp:a:502", 1);
    const two = profileKey("m", "tcp:a:502", 2);
    saveProfile(
      one,
      { v: 1, skipped: ["x"], order: "CDAB", loop: false },
      store,
    );
    expect(loadProfile(one, store)).toEqual({
      v: 1,
      skipped: ["x"],
      order: "CDAB",
      loop: false,
    });
    expect(loadProfile(two, store)).toBeNull();
  });

  it("are discarded when damaged", () => {
    const store = new MemoryStore();
    store.setItem("k", "{not json");
    expect(loadProfile("k", store)).toBeNull();
    store.setItem("k", JSON.stringify({ v: 2, skipped: [] }));
    expect(loadProfile("k", store)).toBeNull();
    store.setItem(
      "k",
      JSON.stringify({ v: 1, skipped: [1, "a"], order: "XYZ" }),
    );
    expect(loadProfile("k", store)).toEqual({
      v: 1,
      skipped: ["a"],
      order: null,
      loop: true,
    });
  });

  it("work without storage", () => {
    expect(loadProfile("k", null)).toBeNull();
    expect(() =>
      saveProfile("k", { v: 1, skipped: [], order: null, loop: true }, null),
    ).not.toThrow();
  });
});

describe("map defaults", () => {
  it("convert the older index list once", () => {
    const store = new MemoryStore();
    store.setItem("modbench.skipped:/maps/x.mbmap", JSON.stringify([1]));
    const ids = loadMapDefault("/maps/x.mbmap", regs, store);
    expect(ids).toEqual([registerId(regs[1])]);
    expect(store.getItem("modbench.skipped:/maps/x.mbmap")).toBeNull();
    expect(loadMapDefault("/maps/x.mbmap", regs, store)).toEqual(ids);
  });

  it("are null for a map never seen", () => {
    expect(loadMapDefault("/none", regs, new MemoryStore())).toBeNull();
  });
});

describe("forgetting", () => {
  it("removes device choices and nothing else", () => {
    const store = new MemoryStore();
    saveProfile(
      profileKey("m", "tcp:a:502", 1),
      { v: 1, skipped: [], order: null, loop: true },
      store,
    );
    saveMapDefault("m", ["a"], store);
    store.setItem("modbench.settings", "{}");
    store.setItem("modbench.connection", "{}");
    expect(forgetAll(store)).toBe(1);
    expect([...store.data.keys()].sort()).toEqual([
      "modbench.connection",
      "modbench.settings",
    ]);
  });
});
