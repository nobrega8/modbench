import { describe, expect, it } from "vitest";
import { formatAddress, formatValue, valueTitle } from "./format";
import type { Register } from "./types";

describe("formatAddress", () => {
  it("shows wire, 1-based and hexadecimal addresses", () => {
    expect(formatAddress(0, "base0")).toBe("0");
    expect(formatAddress(0, "base1")).toBe("1");
    expect(formatAddress(1029, "hex")).toBe("0405h");
    expect(formatAddress(42, "base0", true)).toBe("00042");
    expect(formatAddress(65535, "base1", true)).toBe("65536");
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
