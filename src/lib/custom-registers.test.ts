import { describe, expect, it } from "vitest";
import {
  customProblem,
  customRegister,
  isCustom,
  loadCustom,
  saveCustom,
  type CustomFields,
} from "./custom-registers";

const fields: CustomFields = {
  name: "Power",
  area: "holding",
  address: 100,
  type: "float32",
  length: null,
  scale: 1,
  offset: 0,
  unit: "W",
  access: "RW",
};

function memory() {
  const data = new Map<string, string>();
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
    removeItem: (k: string) => void data.delete(k),
  };
}

describe("custom registers", () => {
  it("builds a marked register; input areas are read-only", () => {
    const r = customRegister(fields);
    expect(isCustom(r)).toBe(true);
    expect(r).toMatchObject({ name: "Power", access: "RW" });
    expect(customRegister({ ...fields, area: "input" }).access).toBe("R");
  });

  it("explains what is wrong", () => {
    expect(customProblem(fields, [])).toBeNull();
    expect(customProblem({ ...fields, name: " " }, [])).toMatch(/name/);
    expect(customProblem(fields, [customRegister(fields)])).toMatch(/already/);
    expect(customProblem({ ...fields, address: 65535 }, [])).toMatch(/fit/);
    expect(customProblem({ ...fields, area: "coil" }, [])).toMatch(/bool/);
    expect(customProblem({ ...fields, type: "string" }, [])).toMatch(/length/);
    expect(customProblem({ ...fields, scale: 0 }, [])).toMatch(/scale/);
  });

  it("keeps them per map file", () => {
    const store = memory();
    const r = customRegister(fields);
    saveCustom("a.json", [r], store);
    expect(loadCustom("a.json", store)).toEqual([r]);
    expect(loadCustom("b.json", store)).toEqual([]);
    saveCustom("a.json", [], store);
    expect(loadCustom("a.json", store)).toEqual([]);
  });
});
