import { describe, expect, it } from "vitest";
import {
  appliesTo,
  findVersion,
  identify,
  inRange,
  parseVersion,
  rangeText,
} from "./identity";
import type { DeviceMap, PointResult, Register } from "./types";

function reg(name: string, extra: Partial<Register> = {}): Register {
  return {
    name,
    enum: [],
    scale: 1,
    offset: 0,
    firmware_min: null,
    firmware_max: null,
    role: null,
    ...extra,
  } as unknown as Register;
}

function result(index: number, value: PointResult["value"]): PointResult {
  return {
    index,
    value,
    reference: null,
    raw: null,
    error: null,
    sentinel: null,
  };
}

describe("versions", () => {
  it("parses map versions and finds them in device text", () => {
    expect(parseVersion("1.02.7")).toEqual([1, 2, 7]);
    expect(parseVersion("2.x")).toBeNull();
    expect(findVersion("V2.01b")).toEqual([2, 1]);
    expect(findVersion("none")).toBeNull();
  });

  it("treats a bound as covering every version that starts with it", () => {
    expect(inRange([2, 9, 1], "1", "2")).toBe(true);
    expect(inRange([3, 0], "1", "2")).toBe(false);
    expect(inRange([2, 0, 9], "2.1", null)).toBe(false);
    expect(inRange([1, 10], "1.9", null)).toBe(true);
    expect(rangeText("1", "2")).toBe("1 to 2");
    expect(rangeText("2.1", null)).toBe("2.1 and later");
    expect(rangeText(null, null)).toBeNull();
  });

  it("keeps every register while the firmware is unknown", () => {
    const newer = reg("PF", { firmware_min: "2.1" });
    expect(appliesTo(newer, null)).toBe(true);
    expect(appliesTo(newer, [2, 0])).toBe(false);
    expect(appliesTo(newer, [2, 1, 3])).toBe(true);
  });
});

describe("identify", () => {
  const map = (meta: object, registers: Register[]) =>
    ({
      meta: {
        model: "EM-200 / EM-200-MID",
        models: ["EM-200", "EM-200-MID"],
        ...meta,
      },
      registers,
    }) as unknown as DeviceMap;
  const regs = [
    reg("Model", { role: "model", enum: [[201, "EM-200-MID"]] }),
    reg("Fw major", { role: "firmware" }),
    reg("Fw minor", { role: "firmware" }),
    reg("Serial", { role: "serial" }),
  ];
  const read = (values: PointResult["value"][]) =>
    new Map(values.map((v, i) => [i, result(i, v)]));

  it("recognises the model by its label and joins firmware parts", () => {
    const id = identify(
      map({ firmware_min: "1", firmware_max: "2" }, regs),
      read([201, 2, 1, "A12345"]),
    );
    expect(id.model).toEqual({ status: "match", found: "EM-200-MID" });
    expect(id.firmware).toEqual({ status: "match", version: [2, 1] });
    expect(id.serial).toBe("A12345");
  });

  it("reports a mismatch with what the device said", () => {
    const id = identify(
      map({ firmware_min: "1", firmware_max: "2" }, regs),
      read([7, 3, 0, null]),
    );
    expect(id.model).toEqual({ status: "mismatch", found: "7" });
    expect(id.firmware.status).toBe("mismatch");
    expect(id.firmware.version).toEqual([3, 0]);
  });

  it("is unknown without readings or without a declared range", () => {
    expect(identify(map({}, regs), new Map()).model.status).toBe("unknown");
    // The version is still found, for registers with their own range.
    const id = identify(map({}, regs), read([201, 2, 1, null]));
    expect(id.firmware).toEqual({ status: "unknown", version: [2, 1] });
    // A part that could not be read leaves the version unknown.
    expect(identify(map({}, regs), read([201, 2])).firmware.version).toBeNull();
  });
});
