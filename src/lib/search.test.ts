import { describe, expect, it } from "vitest";
import { modelMatches } from "./search";

describe("modelMatches", () => {
  it("finds a map by any of the models it applies to", () => {
    expect(modelMatches("em-210", "EM-200", ["EM-200", "EM-210"])).toBe(true);
    expect(modelMatches("em-200", "EM-200")).toBe(true);
    expect(modelMatches("em-3", "EM-200", ["EM-210"])).toBe(false);
  });
});
