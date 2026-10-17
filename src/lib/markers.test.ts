import { describe, expect, it } from "vitest";
import { Markers, markerStats } from "./markers.svelte";

const samples = [
  { t: 0, v: 10 },
  { t: 1000, v: 12 },
  { t: 2000, v: NaN, q: "failed" as const },
  { t: 3000, v: 18 },
  { t: 4000, v: 20 },
];

describe("chart markers", () => {
  it("give values at A and B, their difference, and min, max and average between", () => {
    const s = markerStats(samples, 900, 3100);
    expect(s).toEqual({
      a: 12,
      b: 18,
      diff: 6,
      min: 12,
      max: 18,
      avg: 15,
      count: 2,
    });
    // Order does not matter for the range; B − A keeps the sign
    expect(markerStats(samples, 3100, 900).diff).toBe(-6);
    expect(markerStats([], 0, 1).count).toBe(0);
  });

  it("are placed A first, then B, then the nearer one moves", () => {
    const m = new Markers();
    m.place(100);
    m.place(500);
    expect([m.a, m.b]).toEqual([100, 500]);
    m.place(450);
    expect([m.a, m.b]).toEqual([100, 450]);
    m.place(120);
    expect([m.a, m.b]).toEqual([120, 450]);
    m.clear();
    expect(m.any).toBe(false);
  });
});
