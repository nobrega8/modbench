import { describe, expect, it } from "vitest";
import {
  MIN_SPAN,
  boundsOf,
  clampRange,
  markersRange,
  panRange,
  selectionRange,
  wheelFactor,
  zoomRange,
} from "./zoom";

const bounds = { from: 0, to: 100_000 };

describe("clampRange", () => {
  it("moves a range back inside the bounds", () => {
    expect(clampRange({ from: -500, to: 9_500 }, bounds)).toEqual({
      from: 0,
      to: 10_000,
    });
    expect(clampRange({ from: 95_000, to: 105_000 }, bounds)).toEqual({
      from: 90_000,
      to: 100_000,
    });
  });

  it("cuts a range that is longer than the bounds", () => {
    expect(clampRange({ from: -10, to: 200_000 }, bounds)).toEqual(bounds);
  });

  it("keeps a range at least MIN_SPAN long", () => {
    const r = clampRange({ from: 1_000, to: 1_010 }, bounds);
    expect(r.to - r.from).toBe(MIN_SPAN);
  });
});

describe("zoomRange", () => {
  const r = { from: 20_000, to: 40_000 };

  it("keeps the time under the pointer in place", () => {
    const zoomed = zoomRange(r, 30_000, 0.5, bounds);
    expect(zoomed).toEqual({ from: 25_000, to: 35_000 });
    // A quarter of the way in stays a quarter of the way in.
    const q = zoomRange(r, 25_000, 0.5, bounds);
    expect((25_000 - q.from) / (q.to - q.from)).toBeCloseTo(0.25);
  });

  it("zooms out up to the bounds and no further", () => {
    expect(zoomRange(r, 30_000, 100, bounds)).toEqual(bounds);
  });

  it("does not zoom in past MIN_SPAN", () => {
    const z = zoomRange(r, 30_000, 0.0000001, bounds);
    expect(z.to - z.from).toBe(MIN_SPAN);
  });
});

describe("panRange", () => {
  it("moves the range and stops at the bounds", () => {
    const r = { from: 20_000, to: 30_000 };
    expect(panRange(r, 5_000, bounds)).toEqual({ from: 25_000, to: 35_000 });
    expect(panRange(r, -50_000, bounds)).toEqual({ from: 0, to: 10_000 });
    expect(panRange(r, 500_000, bounds)).toEqual({
      from: 90_000,
      to: 100_000,
    });
  });
});

describe("selectionRange", () => {
  it("takes the two times in either order", () => {
    expect(selectionRange(40_000, 20_000, bounds)).toEqual({
      from: 20_000,
      to: 40_000,
    });
  });

  it("ignores a selection that is too short", () => {
    expect(selectionRange(1_000, 1_100, bounds)).toBeNull();
  });
});

describe("markersRange", () => {
  it("leaves a little room around the markers", () => {
    const r = markersRange(30_000, 20_000, bounds)!;
    expect(r.from).toBeLessThan(20_000);
    expect(r.to).toBeGreaterThan(30_000);
    expect(r.to - r.from).toBeLessThan(11_500);
  });

  it("needs two different times", () => {
    expect(markersRange(5, 5, bounds)).toBeNull();
  });
});

describe("wheelFactor", () => {
  it("zooms in when scrolling up and out when scrolling down", () => {
    expect(wheelFactor(-100)).toBeLessThan(1);
    expect(wheelFactor(100)).toBeGreaterThan(1);
    expect(wheelFactor(0)).toBe(1);
  });

  it("is limited for a fast wheel", () => {
    expect(wheelFactor(1e9)).toBe(wheelFactor(300));
  });
});

describe("boundsOf", () => {
  it("covers the times, and has no bounds without any", () => {
    expect(boundsOf([5_000, 1_000, 9_000])).toEqual({
      from: 1_000,
      to: 9_000,
    });
    expect(boundsOf([])).toBeNull();
  });

  it("is never shorter than MIN_SPAN", () => {
    expect(boundsOf([1_000])).toEqual({ from: 1_000, to: 1_000 + MIN_SPAN });
  });
});
