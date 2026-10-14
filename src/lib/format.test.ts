import { describe, expect, it } from "vitest";
import { formatAddress } from "./format";

describe("formatAddress", () => {
  it("shows wire, 1-based and hexadecimal addresses", () => {
    expect(formatAddress(0, "base0")).toBe("0");
    expect(formatAddress(0, "base1")).toBe("1");
    expect(formatAddress(1029, "hex")).toBe("0405h");
    expect(formatAddress(42, "base0", true)).toBe("00042");
    expect(formatAddress(65535, "base1", true)).toBe("65536");
  });
});
