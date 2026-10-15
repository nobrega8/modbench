import { describe, expect, it } from "vitest";
import { tidy } from "./contextmenu.svelte";

const item = (label: string) => ({ label, action: () => {} });

describe("context menu entries", () => {
  it("drops left-out items and stray separators", () => {
    const a = item("a");
    const b = item("b");
    expect(tidy([null, a, false, null, null, undefined, b, null])).toEqual([
      a,
      null,
      b,
    ]);
  });

  it("leaves nothing when every item is left out", () => {
    expect(tidy([false, null, undefined])).toEqual([]);
  });
});
