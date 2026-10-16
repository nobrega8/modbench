import { describe, expect, it } from "vitest";
import {
  addCycle,
  freeSlot,
  groupByUnit,
  nearest,
  niceTicks,
  plotValue,
  yRange,
} from "./trend";
import type { PointResult, Register } from "./types";

function result(
  index: number,
  value: number | boolean | null,
  error: string | null = null,
) {
  return {
    index,
    value,
    raw: null,
    error,
    sentinel: null,
    reference: null,
  } as PointResult;
}

describe("trend", () => {
  it("turns values into numbers", () => {
    expect(plotValue(3.5)).toBe(3.5);
    expect(plotValue(true)).toBe(1);
    expect(plotValue("text")).toBeNull();
    expect(plotValue(null)).toBeNull();
  });

  it("gives each series the first free color slot", () => {
    expect(freeSlot([])).toBe(1);
    expect(
      freeSlot([
        { index: 4, slot: 1 },
        { index: 9, slot: 3 },
      ]),
    ).toBe(2);
    const full = Array.from({ length: 8 }, (_, i) => ({
      index: i,
      slot: i + 1,
    }));
    expect(freeSlot(full)).toBeNull();
  });

  it("records charted registers only, with gaps for failed reads", () => {
    const series = [
      { index: 0, slot: 1 },
      { index: 2, slot: 2 },
    ];
    let h = addCycle(
      new Map(),
      series,
      [result(0, 5), result(1, 9), result(2, 1)],
      10,
    );
    h = addCycle(h, series, [result(0, null, "timed out"), result(2, 2)], 20);
    expect(h.get(0)?.map((s) => s.v)).toEqual([5, NaN]);
    expect(h.get(2)?.map((s) => s.t)).toEqual([10, 20]);
    expect(h.has(1)).toBe(false);
  });

  it("picks round ticks and a range that holds every value", () => {
    expect(niceTicks(0, 10)).toEqual([0, 2, 4, 6, 8, 10]);
    expect(niceTicks(229.4, 231.2)).toEqual([229.5, 230, 230.5, 231]);
    expect(yRange([229.4, 231.2])).toEqual([229, 231.5]);
    expect(yRange([5, 5])).not.toBeNull();
    expect(yRange([NaN])).toBeNull();
  });

  it("groups series by unit", () => {
    const regs = [{ unit: "V" }, { unit: "A" }, { unit: "V" }] as Register[];
    const groups = groupByUnit(
      [0, 1, 2].map((index) => ({ index, slot: index + 1 })),
      regs,
    );
    expect(groups.map((g) => [g.unit, g.series.map((s) => s.index)])).toEqual([
      ["V", [0, 2]],
      ["A", [1]],
    ]);
  });

  it("finds the sample nearest a time", () => {
    const s = [10, 20, 30].map((t) => ({ t, v: t }));
    expect(nearest(s, 0)?.t).toBe(10);
    expect(nearest(s, 24)?.t).toBe(20);
    expect(nearest(s, 26)?.t).toBe(30);
    expect(nearest([], 5)).toBeNull();
  });
});

describe("sample quality", () => {
  const result = (over: Partial<PointResult>): PointResult => ({
    index: 0,
    reference: null,
    value: 0,
    raw: null,
    error: null,
    sentinel: null,
    ...over,
  });

  it("keeps a real zero apart from a failed read and a missing value", async () => {
    const { sampleOf, lastGood } = await import("./trend");
    expect(sampleOf(result({ value: 0 }), 5)).toEqual({ t: 5, v: 0 });
    expect(sampleOf(result({ error: "timed out", value: null }), 6)).toEqual({
      t: 6,
      v: NaN,
      q: "failed",
    });
    expect(sampleOf(result({ sentinel: "Unimplemented" }), 7).q).toBe(
      "unavailable",
    );
    const samples = [
      { t: 1, v: 3 },
      { t: 2, v: NaN, q: "failed" as const },
      { t: 3, v: NaN, q: "failed" as const },
    ];
    expect(lastGood(samples, 2)).toEqual({ t: 1, v: 3 });
    expect(lastGood([{ t: 1, v: NaN }], 0)).toBeNull();
  });
});
