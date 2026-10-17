import { describe, expect, it } from "vitest";
import { qualityNotes } from "./quality";
import type { PointResult, Register } from "./types";

const reg = (name: string) => ({ name }) as Register;
const regs = ["V1", "V2", "V3", "V4", "V5"].map(reg);
const ok = (index: number): PointResult => ({
  index,
  reference: null,
  value: 1,
  raw: null,
  error: null,
  sentinel: null,
});
const bad = (index: number, error = "timed out after 1000 ms") => ({
  ...ok(index),
  value: null,
  error,
});

describe("quality notes", () => {
  it("name the registers that start failing, by error, and those that recover", () => {
    const before = new Map([0, 1, 2, 3, 4].map((i) => [i, ok(i)]));
    expect(
      qualityNotes(regs, before, [bad(0), bad(1), bad(2), bad(3), ok(4)]),
    ).toEqual(["V1, V2, V3 and 1 more: read failed (timed out after 1000 ms)"]);

    const failing = new Map([
      [0, bad(0)],
      [1, bad(1, "device exception 0x02 (illegal data address)")],
    ]);
    expect(qualityNotes(regs, failing, [ok(0), bad(1)])).toEqual([
      "V1: read again",
    ]);
  });

  it("say nothing while the state stays the same", () => {
    expect(qualityNotes(regs, new Map([[0, bad(0)]]), [bad(0)])).toEqual([]);
    expect(qualityNotes(regs, new Map(), [ok(0)])).toEqual([]);
    expect(qualityNotes(regs, new Map(), [bad(2)])).toEqual([
      "V3: read failed (timed out after 1000 ms)",
    ]);
  });
});
