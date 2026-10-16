import { describe, expect, it } from "vitest";
import {
  areaAccess,
  areaOfFunction,
  formatAddress,
  formatValue,
  parseAddress,
  valueTitle,
} from "./format";
import type { Register } from "./types";

describe("formatAddress", () => {
  it("shows wire, 1-based and hexadecimal addresses", () => {
    expect(formatAddress(0, "base0")).toBe("0");
    expect(formatAddress(0, "base1")).toBe("1");
    expect(formatAddress(1029, "hex")).toBe("0405h");
    expect(formatAddress(42, "base0", null, true)).toBe("00042");
    expect(formatAddress(65535, "base1", null, true)).toBe("65536");
  });

  it("shows Modicon and IEC 61131-3 addresses by area", () => {
    expect(formatAddress(100, "modicon", "holding")).toBe("400101");
    expect(formatAddress(0, "modicon", "coil")).toBe("000001");
    expect(formatAddress(65535, "modicon", "input")).toBe("365536");
    expect(formatAddress(100, "iec", "holding")).toBe("%MW100");
    expect(formatAddress(7, "iec", "input")).toBe("%IW7");
    expect(formatAddress(3, "iec", "discrete")).toBe("%I3");
    expect(formatAddress(3, "iec", "coil")).toBe("%Q3");
    // Without an area: 1-based and 0-based.
    expect(formatAddress(100, "modicon")).toBe("101");
    expect(formatAddress(100, "iec")).toBe("100");
  });

  it("knows the area of each function code", () => {
    expect([1, 2, 3, 4, 5, 6, 15, 16, 43].map(areaOfFunction)).toEqual([
      "coil",
      "discrete",
      "holding",
      "input",
      "coil",
      "holding",
      "coil",
      "holding",
      null,
    ]);
  });
});

describe("parseAddress", () => {
  it("reads Modicon and IEC addresses with their area, in any format", () => {
    const hr100 = { address: 100, area: "holding" };
    expect(parseAddress("400101", "base0")).toEqual(hr100);
    expect(parseAddress("%MW100", "hex")).toEqual(hr100);
    expect(parseAddress("%mw 16#64", "base0")).toEqual(hr100);
    expect(parseAddress("%MW0x64", "base0")).toEqual(hr100);
    expect(parseAddress("%IW5", "base0")).toEqual({
      address: 5,
      area: "input",
    });
    expect(parseAddress("%IX5", "base0")).toEqual({
      address: 5,
      area: "discrete",
    });
    expect(parseAddress("%Q0", "base0")).toEqual({ address: 0, area: "coil" });
    expect(parseAddress("300001", "base0")).toEqual({
      address: 0,
      area: "input",
    });
    expect(parseAddress("%MW70000", "base0")).toBeNull();
  });

  it("reads five-digit Modicon addresses only in that format", () => {
    expect(parseAddress("40101", "modicon")).toEqual({
      address: 100,
      area: "holding",
    });
    expect(parseAddress("000101", "modicon")).toEqual({
      address: 100,
      area: "coil",
    });
    expect(parseAddress("40101", "base0")).toEqual({
      address: 40101,
      area: null,
    });
    expect(parseAddress("000101", "base0")).toEqual({
      address: 101,
      area: null,
    });
  });

  it("reads plain numbers as typed", () => {
    expect(parseAddress("0x10", "base0")).toEqual({ address: 16, area: null });
    expect(parseAddress("12", "modicon")).toEqual({ address: 12, area: null });
    expect(parseAddress("abc", "base0")).toBeNull();
  });
});

describe("valueTitle", () => {
  const reg = (enumLabels: [number, string][], scale = 1) =>
    ({ enum: enumLabels, scale, offset: 0 }) as unknown as Register;

  it("shows the real value behind an enum label", () => {
    const r = reg([[80, "80: group error, timeout, no unit found"]]);
    expect(formatValue(r, 80)).toBe("80: group error, timeout, no unit found");
    expect(valueTitle(r, 80)).toBe(
      "80: group error, timeout, no unit found\nValue: 80",
    );
  });

  it("uses the raw value, before scale, and 0/1 for booleans", () => {
    expect(valueTitle(reg([[5, "High"]], 0.1), 0.5)).toBe("High\nValue: 5");
    expect(valueTitle(reg([[1, "Closed"]]), true)).toBe("Closed\nValue: 1");
  });

  it("is the plain text when no label applies", () => {
    expect(valueTitle(reg([[1, "On"]]), 2)).toBe("2");
    expect(valueTitle(reg([]), false)).toBe("OFF");
  });
});

describe("area access", () => {
  it("tells writable areas from read-only ones and read-only mode", () => {
    expect(areaAccess("holding", false)).toEqual({
      writable: true,
      text: "Holding registers · writable",
    });
    expect(areaAccess("coil", true)).toEqual({
      writable: false,
      text: "Coils · read-only mode is on",
    });
    expect(areaAccess("input", false).text).toBe("Input registers · read only");
    expect(areaAccess("discrete", false).writable).toBe(false);
  });
});
