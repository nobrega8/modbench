import { describe as group, expect, it } from "vitest";
import {
  describe,
  draftFor,
  fromDraft,
  newDraft,
  span,
  toDraft,
} from "./generators";
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

group("draftFor", () => {
  const base = {
    group: "",
    length: null,
    scale: 1,
    offset: 0,
    unit: "",
    write_min: null,
    write_max: null,
    reference: null,
    bit: null,
    bits: null,
    enum: [],
  };
  it("swings around the reference, in engineering units", () => {
    const r = {
      ...base,
      name: "U1",
      area: "input",
      address: 0,
      type: "uint16",
      scale: 0.1,
      reference: "2300",
      unit: "V",
    } as unknown as Register;
    expect(fromDraft(draftFor(r, [r]))).toMatchObject({
      register: "U1",
      kind: "sine",
      offset: 230,
      amplitude: 11.5,
    });
  });

  it("spans the write limits", () => {
    const r = {
      ...base,
      name: "Set",
      area: "holding",
      address: 0,
      type: "int16",
      write_min: 0,
      write_max: 100,
    } as unknown as Register;
    expect(draftFor(r, [r])).toMatchObject({
      offset: 50,
      amplitude: 25,
      min: 0,
      max: 100,
    });
  });

  it("toggles bits and steps through enums", () => {
    const bit = {
      ...base,
      name: "On",
      area: "coil",
      address: 0,
      type: "bool",
    } as unknown as Register;
    expect(fromDraft(draftFor(bit, [bit]))).toMatchObject({
      kind: "increment",
      min: 0,
      max: 1,
    });
    const mode = {
      ...base,
      name: "Mode",
      area: "holding",
      address: 1,
      type: "uint16",
      enum: [
        [1, "Idle"],
        [3, "Run"],
      ],
    } as unknown as Register;
    expect(fromDraft(draftFor(mode, [mode]))).toMatchObject({
      kind: "increment",
      min: 1,
      max: 3,
      step: 1,
    });
  });
});
