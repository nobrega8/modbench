import { describe, expect, it } from "vitest";
import { FUNCTIONS, buildData, hexBytes, onlyReads } from "./requests";

const spec = (fc: number) => FUNCTIONS.find((f) => f.fc === fc)!;

describe("hand-built requests", () => {
  it("build FC22 and FC23 data from typed fields", () => {
    expect(
      hexBytes(buildData(spec(22), { a: "4", and: "0x0F0F", or: "0x3000" })),
    ).toBe("00 04 0F 0F 30 00");
    expect(
      buildData(spec(23), { ra: "10", rn: "2", wa: "10", values: "7, 8" }),
    ).toEqual([0, 10, 0, 2, 0, 10, 0, 2, 4, 0, 7, 0, 8]);
  });

  it("pack coils, and read the others' fields", () => {
    expect(buildData(spec(15), { a: "0", bits: "1 0 1 1 0 0 0 0 1" })).toEqual([
      0, 0, 0, 9, 2, 0x0d, 0x01,
    ]);
    expect(buildData(spec(8), { sub: "0", data: "A5 37" })).toEqual([
      0, 0, 0xa5, 0x37,
    ]);
    expect(buildData(spec(43), { code: "1", object: "0" })).toEqual([14, 1, 0]);
  });

  it("name the field that is wrong", () => {
    expect(() =>
      buildData(spec(22), { a: "70000", and: "0", or: "0" }),
    ).toThrow(/Address/);
    expect(() => buildData(spec(16), { a: "0", values: "" })).toThrow(/Values/);
  });

  it("tell reads from writes as the backend does", () => {
    expect(onlyReads(3, [])).toBe(true);
    expect(onlyReads(8, [0, 0, 1, 2])).toBe(true);
    expect(onlyReads(8, [0, 1, 0, 0])).toBe(false);
    expect(onlyReads(22, [])).toBe(false);
    expect(onlyReads(99, [])).toBe(false);
  });
});
