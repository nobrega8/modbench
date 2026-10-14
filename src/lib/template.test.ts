import { describe, expect, it } from "vitest";
import { collect, parse, plan, serialize } from "./template";
import type { PointResult, Register } from "./types";

function reg(over: Partial<Register>): Register {
  return {
    name: "R",
    group: "",
    area: "holding",
    address: 0,
    type: "uint16",
    access: "RW",
    scale: 1,
    offset: 0,
    unit: "",
    enum: [],
    bit: null,
    bits: null,
    write_min: null,
    write_max: null,
    ...over,
  } as Register;
}

const ok = (index: number, value: PointResult["value"]): PointResult => ({
  index,
  reference: null,
  value,
  raw: null,
  error: null,
  sentinel: null,
});

describe("template file", () => {
  it("round-trips values, bits and strings", () => {
    const regs = [
      reg({ name: "Setpoint", address: 16, scale: 0.1 }),
      reg({ name: "Alarm", address: 17, type: "bool", bit: 3 }),
      reg({ name: "Site", address: 30, type: "string" }),
      reg({ name: "Energy", address: 40, access: "R" }),
    ];
    const results = new Map([
      [0, ok(0, 25.5)],
      [1, ok(1, true)],
      [2, ok(2, 'Plant "A" // 1')],
      [3, ok(3, 9)],
    ]);
    const entries = collect(regs, results);
    expect(entries).toHaveLength(3); // the read-only register is left out
    const text = serialize("Example EM-100", entries, new Date(0));
    const back = parse(text);
    expect(back.errors).toEqual([]);
    expect(back.device).toBe("Example EM-100");
    expect(back.entries.map((e) => [e.address, e.bit, e.value])).toEqual([
      [16, null, 25.5],
      [17, 3, 1],
      [30, null, 'Plant "A" // 1'],
    ]);
  });

  it("accepts hex and comments, and reports bad lines", () => {
    const t = parse(
      "// note\nholding 0x10 = 0x1F // hex\nholding 5 = 2,5\nfoo 1 = 2\nholding 70000 = 1\nholding 3 =\n",
    );
    expect(t.entries.map((e) => [e.address, e.value])).toEqual([
      [16, 31],
      [5, 2.5],
    ]);
    expect(t.errors.map((e) => e.line)).toEqual([4, 5, 6]);
  });
});

describe("applying a template", () => {
  const regs = [
    reg({ name: "A", address: 1, write_max: 10 }),
    reg({ name: "B", address: 2 }),
    reg({ name: "C", address: 3, access: "R" }),
  ];
  const results = new Map([
    [0, ok(0, 5)],
    [1, ok(1, 7)],
  ]);

  it("separates changed, unchanged, refused and unmatched", () => {
    const t = parse(
      "holding 1 = 5\nholding 2 = 8\nholding 3 = 1\nholding 1 = 11\nholding 99 = 1\n",
    );
    const p = plan(regs, results, t.entries);
    expect(p.unmatched).toHaveLength(1);
    expect(p.writes.map((w) => [w.index, w.changed, !!w.problem])).toEqual([
      [0, false, false],
      [1, true, false],
      [2, true, true], // read-only
      [0, true, true], // above write_max
    ]);
  });

  it("treats everything as changed when nothing was read", () => {
    const p = plan(regs, new Map(), parse("holding 1 = 5\n").entries);
    expect(p.writes[0].changed).toBe(true);
    expect(p.writes[0].current).toBeNull();
  });
});
