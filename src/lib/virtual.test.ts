import { describe, expect, it } from "vitest";
import { VIRTUAL_FROM, windowOf } from "./virtual";

describe("windowOf", () => {
  it("renders a short table whole", () => {
    expect(windowOf(VIRTUAL_FROM - 1, 500, 400, 28)).toEqual({
      start: 0,
      end: VIRTUAL_FROM - 1,
      before: 0,
      after: 0,
    });
  });

  it("renders the rows in view with a margin, and spaces the rest", () => {
    const w = windowOf(1000, 2800, 280, 28, 10);
    expect(w.start).toBe(90);
    expect(w.end).toBe(120);
    expect(w.before).toBe(90 * 28);
    expect(w.after).toBe(880 * 28);
  });

  it("stays inside the table at both ends", () => {
    expect(windowOf(1000, 0, 280, 28, 10)).toMatchObject({ start: 0, end: 20 });
    expect(windowOf(1000, 1e9, 280, 28, 10)).toMatchObject({
      start: 1000,
      end: 1000,
      after: 0,
    });
  });

  it("falls back to the whole table when the row height is unknown", () => {
    expect(windowOf(1000, 0, 280, 0)).toMatchObject({ start: 0, end: 1000 });
  });
});
