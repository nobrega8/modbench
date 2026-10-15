import { describe as group, expect, it } from "vitest";
import { describe, fromDraft, newDraft, span, toDraft } from "./generators";
import type { Generator, Register } from "./types";

const voltage = {
  name: "Voltage",
  area: "holding",
  address: 10,
  type: "float32",
  unit: "V",
  bit: null,
} as Register;

group("generators", () => {
  it("round-trips through the form", () => {
    const list: Generator[] = [
      {
        register: "Voltage",
        kind: "sine",
        period_s: 10,
        amplitude: 5,
        offset: 230,
        every_ms: 500,
      },
      {
        area: "input",
        address: 3,
        kind: "increment",
        step: 2,
        min: 0,
        max: 9,
        every_ms: 1000,
      },
      {
        area: "input",
        address: 4,
        kind: "random",
        min: 1,
        max: 2,
        every_ms: 200,
      },
      {
        area: "discrete",
        address: 0,
        kind: "copy",
        from: { area: "coil", address: 0 },
        every_ms: 100,
      },
    ];
    for (const g of list) expect(fromDraft(toDraft(g, [voltage]))).toEqual(g);
  });

  it("starts on a map register when there is one", () => {
    expect(fromDraft(newDraft([voltage]))).toMatchObject({
      register: "Voltage",
    });
    expect(fromDraft(newDraft([]))).toMatchObject({
      area: "holding",
      address: 0,
    });
  });

  it("knows the addresses a target takes", () => {
    expect(span({ register: "voltage" }, [voltage])).toEqual({
      area: "holding",
      address: 10,
      count: 2,
    });
    expect(span({ register: "Other" }, [voltage])).toBeNull();
    // An address where a register starts takes the whole register.
    expect(span({ area: "holding", address: 10 }, [voltage])).toMatchObject({
      count: 2,
    });
    expect(span({ area: "holding", address: 11 }, [voltage])).toMatchObject({
      count: 1,
    });
  });

  it("describes each kind", () => {
    const name = () => "Coil 0";
    const sine = fromDraft({ ...newDraft([]), offset: 230, amplitude: 5 });
    expect(describe(sine, name, "V")).toBe("230 ± 5 V, 10 s period");
    const down = fromDraft({ ...newDraft([]), kind: "increment", step: -0.5 });
    expect(describe(down, name)).toBe("−0.5 each time, 0 to 100");
    const copy = fromDraft({ ...newDraft([]), kind: "copy" });
    expect(describe(copy, name)).toBe("Follows Coil 0");
  });
});
