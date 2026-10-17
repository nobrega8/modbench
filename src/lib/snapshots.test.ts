import { describe, expect, it } from "vitest";
import {
  differs,
  liveMap,
  parseSnapshot,
  snapMap,
  snapRead,
  snapText,
  snapWord,
  snapshotFile,
} from "./snapshots";
import type { PointResult, Register } from "./types";

const reg = (name: string, address: number) =>
  ({
    name,
    area: "holding",
    address,
    type: "uint16",
    bit: null,
    bits: null,
    scale: 1,
    offset: 0,
    enum: [],
    unit: "",
  }) as unknown as Register;
const result = (
  index: number,
  value: number | null,
  error?: string,
): PointResult => ({
  index,
  reference: null,
  value,
  raw: { kind: "registers", values: [value ?? 0] },
  error: error ?? null,
  sentinel: null,
});

describe("snapshots", () => {
  const regs = [reg("A", 0), reg("B", 1), reg("C", 2)];

  it("keep what each register showed and tell which ones changed", () => {
    const before = snapMap(
      { vendor: "V", model: "M" },
      regs,
      new Map([
        [0, result(0, 5)],
        [1, result(1, 7)],
      ]),
      "before",
      1000,
    );
    expect(snapText(before, regs[0])).toBe("5");
    // C was not read: it is in neither side and never counts as different
    expect(snapText(before, regs[2])).toBeUndefined();
    const now = liveMap(
      regs,
      new Map([
        [0, result(0, 5)],
        [1, result(1, null, "timed out")],
        [2, result(2, 1)],
      ]),
    );
    const id = (r: Register) =>
      [...now.keys()].find((k) => k.endsWith(r.name))!;
    expect(differs(snapText(before, regs[0]), now.get(id(regs[0])))).toBe(
      false,
    );
    expect(now.get(id(regs[1]))).toBe("read failed");
    expect(differs(snapText(before, regs[1]), now.get(id(regs[1])))).toBe(true);
    expect(differs(snapText(before, regs[2]), now.get(id(regs[2])))).toBe(
      false,
    );
  });

  it("round-trip through a file, and refuse other files", () => {
    const s = snapRead("input", 100, [1, 2, 3], "unit 3", 5);
    const back = parseSnapshot(snapshotFile(s));
    expect(back).toEqual(s);
    if ("error" in back) throw new Error();
    expect(snapWord(back, "input", 101)).toBe(2);
    expect(snapWord(back, "holding", 101)).toBeUndefined();
    expect(snapWord(back, "input", 99)).toBeUndefined();
    expect(parseSnapshot("{}")).toEqual({
      error: "This is not a Modbench snapshot.",
    });
    expect("error" in parseSnapshot("nope")).toBe(true);
  });
});
