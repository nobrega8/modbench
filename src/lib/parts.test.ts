import { describe, expect, it } from "vitest";
import { parseParts, partsLine } from "./parts.ts";
import type { Part } from "./types.ts";

const PARTS: Part[] = [
  { name: "Recipient", offset: 0, type: "string", length: 2 },
  { name: "Port", offset: 2, type: "uint16", length: 1 },
];

describe("parts", () => {
  it("parse each part, an empty one as null", () => {
    expect(parseParts(PARTS, ["0035", ""])).toEqual({
      values: ["0035", null],
    });
    expect(parseParts(PARTS, ["", "0x10"])).toEqual({ values: [null, 16] });
  });

  it("refuse text longer than the part and a bad number", () => {
    expect(parseParts(PARTS, ["12345", ""])).toHaveProperty("error");
    expect(parseParts(PARTS, ["", "abc"])).toHaveProperty("error");
  });

  it("show every part on one line", () => {
    expect(partsLine(PARTS, ["0035", null])).toBe("Recipient: 0035 · Port: —");
  });
});
