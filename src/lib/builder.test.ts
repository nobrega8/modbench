import { describe, expect, it } from "vitest";
import {
  blankRegister,
  diffRegisters,
  enumText,
  findRegister,
  fileAddress,
  mapFileName,
  parseEnum,
  sortDiagnostics,
  wireAddress,
  withArea,
} from "./builder";
import type { Diagnostic } from "./types";

const diag = (pointer: string | null, message = "m"): Diagnostic => ({
  severity: "error",
  line: null,
  pointer,
  code: "c",
  message,
});

describe("withArea", () => {
  it("makes bits booleans and drops scaling", () => {
    const r = withArea(
      blankRegister({ scale: 0.1, unit: "V", access: "RW" }),
      "coil",
    );
    expect(r).toMatchObject({ type: "bool", scale: 1, unit: "", area: "coil" });
  });

  it("makes read-only areas read-only", () => {
    expect(withArea(blankRegister({ access: "RW" }), "input").access).toBe("R");
    expect(withArea(blankRegister({ access: "RW" }), "holding").access).toBe(
      "RW",
    );
  });

  it("turns a bool back into a word when it leaves the bit areas", () => {
    const coil = withArea(blankRegister(), "coil");
    expect(withArea(coil, "holding").type).toBe("uint16");
  });

  it("keeps a bool that picks one bit of a word", () => {
    const r = blankRegister({ type: "bool", bit: 3 });
    expect(withArea(r, "input").type).toBe("bool");
  });
});

describe("state labels", () => {
  it("round trip", () => {
    const pairs: [number, string][] = [
      [0, "Off"],
      [1, "On"],
    ];
    expect(parseEnum(enumText(pairs))).toEqual(pairs);
  });

  it("accept hex values and ignore empty parts", () => {
    expect(parseEnum("0x10=High, , 2=Low")).toEqual([
      [16, "High"],
      [2, "Low"],
    ]);
    expect(parseEnum("")).toEqual([]);
  });

  it("refuse text that is not value=label pairs", () => {
    expect(parseEnum("Off")).toBeNull();
    expect(parseEnum("a=Off")).toBeNull();
    expect(parseEnum("1=")).toBeNull();
    expect(parseEnum("1=On, 1=Again")).toBeNull();
  });
});

describe("addresses", () => {
  it("follow the address base", () => {
    expect(fileAddress(blankRegister({ address: 9 }), 1)).toBe(10);
    expect(fileAddress(blankRegister({ address: 9 }), null)).toBe(9);
    expect(wireAddress(10, 1)).toBe(9);
    expect(wireAddress(0, 1)).toBeNull();
    expect(wireAddress(65536, 1)).toBe(65535);
    expect(wireAddress(65536, 0)).toBeNull();
    expect(wireAddress(1.5, 0)).toBeNull();
  });
});

describe("sortDiagnostics", () => {
  it("puts register messages on their row and the rest on the file", () => {
    const { rows, file } = sortDiagnostics([
      diag("/registers/2/address"),
      diag("/registers/2"),
      diag("/registers/10/type"),
      diag("/vendor"),
      diag(null),
    ]);
    expect(rows.get(2)).toHaveLength(2);
    expect(rows.get(10)).toHaveLength(1);
    expect(file).toHaveLength(2);
  });
});

describe("mapFileName", () => {
  it("makes a library style name", () => {
    expect(mapFileName({ vendor: "Lovato Electric", model: "DCRG8" })).toBe(
      "lovato-electric-dcrg8.mbmap.json",
    );
    expect(mapFileName({ vendor: "", model: "" })).toBe("map.mbmap.json");
  });
});

describe("diffRegisters", () => {
  const a = blankRegister({ name: "A", address: 0 });
  const b = blankRegister({ name: "B", address: 1, unit: "V" });

  it("finds nothing when nothing changed", () => {
    expect(diffRegisters([a, b], [{ ...a }, { ...b }])).toEqual({
      added: [],
      removed: [],
      changed: [],
    });
  });

  it("names the fields that changed", () => {
    const d = diffRegisters([a, b], [a, { ...b, unit: "A", scale: 0.1 }]);
    expect(d.changed).toHaveLength(1);
    expect(d.changed[0].fields).toEqual(["scale", "unit"]);
  });

  it("finds added and removed registers", () => {
    const c = blankRegister({ name: "C", address: 5 });
    const d = diffRegisters([a, b], [a, c]);
    expect(d.added).toEqual([c]);
    expect(d.removed).toEqual([b]);
  });

  it("sees a register that moved as removed and added", () => {
    const d = diffRegisters([a], [{ ...a, address: 9 }]);
    expect(d.added).toHaveLength(1);
    expect(d.removed).toHaveLength(1);
  });

  it("pairs registers that share a place in order", () => {
    const x = blankRegister({ name: "X", address: 7 });
    const y = blankRegister({ name: "Y", address: 7 });
    const d = diffRegisters([x, y], [x, { ...y, description: "new" }]);
    expect(d.changed.map((c) => c.after.name)).toEqual(["Y"]);
  });
});

describe("findRegister", () => {
  const list = [
    blankRegister({ name: "A", address: 3 }),
    blankRegister({ name: "B", address: 3 }),
    blankRegister({ name: "C", address: 4, area: "input" }),
  ];

  it("prefers the name, then the place", () => {
    expect(findRegister(list, { area: "holding", address: 3, name: "B" })).toBe(
      1,
    );
    expect(findRegister(list, { area: "holding", address: 3, name: "?" })).toBe(
      0,
    );
    expect(findRegister(list, { area: "input", address: 9, name: "C" })).toBe(
      -1,
    );
  });
});
