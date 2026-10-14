import { describe, expect, it } from "vitest";
import { isFresh, markPoll, markRead, prune } from "./changes";
import type { PointResult } from "./types";

const regs = (values: number[]) => ({ kind: "registers" as const, values });

function point(
  index: number,
  raw: number[] | null,
  error: string | null = null,
) {
  return {
    index,
    value: raw ? raw[0] : null,
    raw: raw ? regs(raw) : null,
    error,
    sentinel: null,
    reference: null,
  } as PointResult;
}

describe("change highlighting", () => {
  it("marks the offsets that changed between two reads", () => {
    const empty = new Map();
    const t = markRead(empty, regs([1, 2, 3]), regs([1, 9, 3]), 100);
    expect([...t.keys()]).toEqual(["g:1"]);
    expect(markRead(empty, regs([1]), regs([1]), 100)).toBe(empty);
    expect(markRead(empty, null, regs([1]), 100)).toBe(empty);
  });

  it("marks map registers whose raw value changed, ignoring failures", () => {
    const prev = new Map([
      [0, point(0, [5])],
      [1, point(1, [7])],
      [2, point(2, null, "timed out")],
    ]);
    const t = markPoll(
      new Map(),
      prev,
      [point(0, [6]), point(1, null, "timed out"), point(2, [1])],
      50,
    );
    expect([...t.keys()]).toEqual(["m:0"]);
  });

  it("keeps a change fresh for the hold time, then prunes it", () => {
    const t = new Map([["m:0", 1000]]);
    expect(isFresh(t, "m:0", 2999, 2000)).toBe(true);
    expect(isFresh(t, "m:0", 3000, 2000)).toBe(false);
    expect(prune(t, 2000, 2000)).toBe(t);
    expect(prune(t, 3000, 2000).size).toBe(0);
  });
});
