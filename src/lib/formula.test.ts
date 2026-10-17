import { describe, expect, it } from "vitest";
import { computeResults, computedRegister } from "./computed";
import { evaluate, formulaProblem, parse } from "./formula";
import type { PointResult, Register } from "./types";

const calc = (text: string, vars: Record<string, number> = {}) =>
  evaluate(parse(text), (n) => vars[n]);

describe("formulas", () => {
  it("follow the usual precedence", () => {
    expect(calc("1 + 2 * 3")).toBe(7);
    expect(calc("(1 + 2) * 3")).toBe(9);
    expect(calc("2 ** 3 ** 2")).toBe(512);
    expect(calc("-2 ** 2")).toBe(-4);
    expect(calc("10 % 4 + 7 / 2")).toBe(5.5);
    expect(calc("0x10 | 0b0001")).toBe(17);
    expect(calc("(6 >> 1) & 1")).toBe(1);
    expect(calc("3 > 2 && 2 >= 3")).toBe(0);
    expect(calc("round(2.345, 2) + max(1, 4, 2) + abs(-1)")).toBe(7.35);
  });

  it("refuse mistakes and unknown registers with a clear message", () => {
    const regs = ["Voltage L1", "Current L1", "Power"];
    expect(formulaProblem("Voltage_L1 * Current_L1 / 1000", regs)).toBeNull();
    expect(formulaProblem("[Voltage L1] * 2", regs)).toBeNull();
    expect(formulaProblem("Voltage_L2 * 2", regs)).toBe(
      'No register is called "Voltage_L2"',
    );
    expect(formulaProblem("Power *", regs)).toBe("The formula ends too early.");
    expect(formulaProblem("(Power", regs)).toMatch(/"\)" missing/);
    expect(formulaProblem("Power $ 2", regs)).toBe('Unexpected "$" at 7');
    expect(formulaProblem("sin(Power)", regs)).toMatch(
      /Unknown function "sin"/,
    );
    expect(formulaProblem("round()", regs)).toBe("round takes 1 to 2 values");
    expect(formulaProblem("", regs)).toBe("The formula is empty.");
  });
});

describe("computed values", () => {
  const reg = (name: string) => ({ name, extra: [] }) as unknown as Register;
  const ok = (index: number, value: number | boolean): PointResult => ({
    index,
    reference: null,
    value,
    raw: null,
    error: null,
    sentinel: null,
  });

  it("work out each cycle, use one another, and fail when an input failed", () => {
    const regs = [
      reg("Voltage L1"),
      reg("Current L1"),
      computedRegister("Power", "kW", "Voltage_L1 * Current_L1 / 1000"),
      computedRegister("Power W", "W", "Power * 1000"),
    ];
    const good = computeResults(
      regs,
      new Map([
        [0, ok(0, 230)],
        [1, ok(1, 10)],
      ]),
    );
    expect(good.map((r) => r.value)).toEqual([2.3, 2300]);

    const failed = computeResults(
      regs,
      new Map([
        [0, ok(0, 230)],
        [1, { ...ok(1, 0), value: null, error: "timed out" }],
      ]),
    );
    expect(failed[0].error).toBe("Current L1 failed: timed out");
    expect(failed[1].error).toBe("Power failed: Current L1 failed: timed out");
    expect(failed.every((r) => r.value === null)).toBe(true);

    const zero = computeResults(
      [reg("A"), computedRegister("X", "", "1 / A")],
      new Map([[0, ok(0, 0)]]),
    );
    expect(zero[0].error).toMatch(/division by zero/);
  });
});
