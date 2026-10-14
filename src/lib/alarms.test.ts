import { describe, expect, it } from "vitest";
import {
  byIndex,
  check,
  limitsText,
  loadAlarms,
  saveAlarms,
  type Alarm,
} from "./alarms";
import { registerId, type KeyValueStore } from "./profiles";
import type { Register } from "./types";

function memoryStore(): KeyValueStore {
  const m = new Map<string, string>();
  return {
    getItem: (k) => m.get(k) ?? null,
    setItem: (k, v) => void m.set(k, v),
    removeItem: (k) => void m.delete(k),
    get length() {
      return m.size;
    },
    key: (i) => [...m.keys()][i] ?? null,
  };
}

const alarm = (a: Partial<Alarm>): Alarm => ({
  id: "x",
  low: null,
  high: null,
  when: null,
  ...a,
});

describe("alarms", () => {
  it("checks values against open or closed limits", () => {
    const a = alarm({ low: 10, high: 250 });
    expect(check(a, 9.9)).toBe("low");
    expect(check(a, 10)).toBeNull();
    expect(check(a, 251)).toBe("high");
    expect(check(a, null)).toBeNull();
    expect(check(alarm({ high: 5 }), -100)).toBeNull();
    expect(check(alarm({ when: true }), true)).toBe("bit");
    expect(check(alarm({ when: true }), false)).toBeNull();
    expect(check(a, "text")).toBeNull();
  });

  it("describes the limits", () => {
    expect(limitsText(alarm({ low: 10, high: 250 }), "V")).toBe(
      "< 10 or > 250 V",
    );
    expect(limitsText(alarm({ when: false }))).toBe("when OFF");
  });

  it("stores alarms per map and finds their registers", () => {
    const store = memoryStore();
    const reg = {
      area: "holding",
      address: 4,
      type: "uint16",
      bit: null,
      bits: null,
      name: "V",
    } as Register;
    const a = alarm({ id: registerId(reg), high: 3 });
    saveAlarms("a.mbmap.json", [a, alarm({ id: "empty" })], store);
    const loaded = loadAlarms("a.mbmap.json", store);
    expect(loaded).toEqual([a]);
    expect(loadAlarms("b.mbmap.json", store)).toEqual([]);
    expect(byIndex(loaded, [{ ...reg, name: "other" }, reg]).get(1)).toEqual(a);
    saveAlarms("a.mbmap.json", [], store);
    expect(store.length).toBe(0);
  });
});
