// Computed values: registers of a map view whose value is a formula over the
// other registers. They are custom registers (kept per map file on this
// computer, never written into it) that are not read from the device: each
// cycle works their value out from the values just read.

import { CUSTOM_MARK } from "./custom-registers";
import { FormulaError, evaluate, parse, resolve } from "./formula";
import type { PointResult, Register } from "./types";

/** `extra` column that holds a computed value's formula. */
export const COMPUTED_MARK = "x_formula";

export function formulaOf(r: Register): string | null {
  return r.extra.find(([k]) => k === COMPUTED_MARK)?.[1] ?? null;
}

export function isComputed(r: Register): boolean {
  return formulaOf(r) !== null;
}

/** A computed value as a register: never read from the device. */
export function computedRegister(
  name: string,
  unit: string,
  formula: string,
): Register {
  return {
    name: name.trim(),
    group: "Computed",
    area: "holding",
    address: 0,
    type: "float64",
    length: null,
    scale: 1,
    scale_factor: null,
    offset: 0,
    unit: unit.trim(),
    access: "R",
    write_min: null,
    write_max: null,
    reference: null,
    byte_order: null,
    bit: null,
    bits: null,
    enum: [],
    enum_ref: null,
    description: `= ${formula.trim()}`,
    extra: [
      [CUSTOM_MARK, "true"],
      [COMPUTED_MARK, formula.trim()],
    ],
    comment: "",
    firmware_min: null,
    firmware_max: null,
    role: null,
  };
}

/** A register's value as a number for a formula, or why there is none. */
function numberOf(name: string, r: PointResult | undefined): number {
  if (!r) throw new FormulaError(`${name} has not been read`);
  if (r.error) throw new FormulaError(`${name} failed: ${r.error}`);
  if (r.sentinel)
    throw new FormulaError(`${name} has no value (${r.sentinel})`);
  const v = r.value;
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "boolean") return v ? 1 : 0;
  throw new FormulaError(`${name} is not a number`);
}

/**
 * The results of the computed values, from the results of the other
 * registers (`results`, by index). In map order, so a computed value can
 * use one above it. A value whose input failed is an error: a gap in charts
 * and recordings, never a made-up number.
 */
export function computeResults(
  registers: Register[],
  results: Map<number, PointResult>,
  skipped: Set<number> = new Set(),
): PointResult[] {
  const names = registers.map((r) => r.name);
  const byName = new Map(names.map((n, i) => [n, i]));
  const all = new Map(results);
  const out: PointResult[] = [];
  registers.forEach((reg, index) => {
    const formula = formulaOf(reg);
    if (formula === null || skipped.has(index)) return;
    let result: PointResult;
    try {
      const node = parse(formula);
      const names = resolve(
        node,
        registers.map((r) => r.name),
      );
      const value = evaluate(node, (written) => {
        const name = names.get(written)!;
        return numberOf(name, all.get(byName.get(name)!));
      });
      result = Number.isFinite(value)
        ? {
            index,
            reference: null,
            value,
            raw: null,
            error: null,
            sentinel: null,
          }
        : {
            index,
            reference: null,
            value: null,
            raw: null,
            error: "no number (division by zero?)",
            sentinel: null,
          };
    } catch (e) {
      result = {
        index,
        reference: null,
        value: null,
        raw: null,
        error: e instanceof Error ? e.message : String(e),
        sentinel: null,
      };
    }
    all.set(index, result);
    out.push(result);
  });
  return out;
}
