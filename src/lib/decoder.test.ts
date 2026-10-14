import { describe, expect, it } from "vitest";
import { bcd, normalize, parseWords, wideRows, wordRows } from "./decoder";

describe("decoder", () => {
  it("reads registers as hex or decimal", () => {
    expect(parseWords("4148 0000")).toEqual([0x4148, 0]);
    expect(parseWords("41 48 00 00")).toEqual([0x4148, 0]);
    expect(parseWords("0x4148, 0x0000")).toEqual([0x4148, 0]);
    expect(parseWords("41 48 00")).toBeNull();
    expect(parseWords("16712 0", true)).toEqual([16712, 0]);
    expect(parseWords("-1", true)).toEqual([0xffff]);
    expect(parseWords("70000", true)).toBeNull();
  });

  it("orders bytes like the backend codec", () => {
    const w = [0x0102, 0x0304];
    expect(normalize(w, "ABCD")).toEqual([1, 2, 3, 4]);
    expect(normalize(w, "CDAB")).toEqual([3, 4, 1, 2]);
    expect(normalize(w, "BADC")).toEqual([2, 1, 4, 3]);
    expect(normalize(w, "DCBA")).toEqual([4, 3, 2, 1]);
  });

  it("gives every wide reading in each byte order", () => {
    // 12.5 as float32 is 0x41480000.
    const rows = wideRows([0x4148, 0x0000]);
    const f = rows.find((r) => r.label === "32-bit float")!;
    expect(f.values[0]).toBe("12.5");
    const swapped = wideRows([0x0000, 0x4148]).find(
      (r) => r.label === "32-bit float",
    )!;
    expect(swapped.values[1]).toBe("12.5");
    expect(rows.some((r) => r.label.startsWith("64-bit"))).toBe(false);
    const long = wideRows([0, 0, 0, 1]);
    expect(long.find((r) => r.label === "64-bit unsigned")!.values[0]).toBe(
      "1",
    );
  });

  it("reads text and BCD", () => {
    const t = wideRows([0x4142, 0x4300]).find(
      (r) => r.label === "Text (ASCII)",
    )!;
    expect(t.values[0]).toBe("ABC·");
    expect(t.values[2]).toBe("BA·C");
    expect(bcd(0x1234)).toBe("1234");
    expect(bcd(0x12a4)).toBeNull();
  });

  it("describes each register", () => {
    const [r] = wordRows([0xfffe]);
    expect(r).toMatchObject({
      hex: "FFFE",
      unsigned: 65534,
      signed: -2,
      swapped: 0xfeff,
    });
    expect(r.binary).toBe("1111 1111 1111 1110");
  });
});
