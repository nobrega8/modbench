import { describe, expect, it } from "vitest";
import {
  compareRows,
  isMark,
  isMarksView,
  shown,
  wordKey,
  type Mark,
} from "./highlights";

const rows = [0, 1, 2, 3, 4].map((index) => ({ index }));

describe("highlight ordering", () => {
  const marks = new Map<number, Mark>([
    [3, 1],
    [4, 2],
  ]);

  it("keeps the map order when showing everything", () => {
    const sorted = [...rows].sort((a, b) =>
      compareRows(a, b, new Set(), marks, "all"),
    );
    expect(sorted.map((r) => r.index)).toEqual([0, 1, 2, 3, 4]);
  });

  it("lists the highlighted first, then the rest, in map order", () => {
    const sorted = [...rows].sort((a, b) =>
      compareRows(a, b, new Set(), marks, "first"),
    );
    expect(sorted.map((r) => r.index)).toEqual([3, 4, 0, 1, 2]);
  });

  it("still puts registers that are not read at the end", () => {
    const sorted = [...rows].sort((a, b) =>
      compareRows(a, b, new Set([3, 1]), marks, "first"),
    );
    expect(sorted.map((r) => r.index)).toEqual([4, 0, 2, 3, 1]);
  });

  it("shows only the highlighted in the only view", () => {
    expect(rows.filter((r) => shown(r.index, marks, "only"))).toEqual([
      { index: 3 },
      { index: 4 },
    ]);
    expect(rows.filter((r) => shown(r.index, marks, "first"))).toHaveLength(5);
  });
});

describe("validation", () => {
  it("accepts only the colours and views on offer", () => {
    expect(isMark(1)).toBe(true);
    expect(isMark(5)).toBe(false);
    expect(isMark("1")).toBe(false);
    expect(isMarksView("only")).toBe(true);
    expect(isMarksView("some")).toBe(false);
  });

  it("names a word by area and address", () => {
    expect(wordKey("holding", 40)).toBe("holding:40");
  });
});
