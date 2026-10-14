// Value templates: a plain-text list of register values that can be saved
// from one device and written to another of the same model.
//
//   // Modbench value template
//   // Device: Example Corp EM-100
//   holding 16 = 25.5     // Setpoint
//   holding 17.3 = 1      // Alarm enable (bit 3)
//   holding 30 = "Plant"  // Site name
//
// Values are in engineering units, like the ones shown in the app. A line is
// `area address[.bit] = value`; `//` starts a comment.

import { limitError, parseBool, parseInteger } from "./format";
import type { Area, PointResult, Register, Value } from "./types";

export interface TemplateEntry {
  area: Area;
  address: number;
  bit: number | null;
  value: Value;
  /** Register name, kept only to make the file readable. */
  name: string;
}

export interface ParsedTemplate {
  device: string;
  entries: TemplateEntry[];
  errors: { line: number; message: string }[];
}

const AREAS: Area[] = ["coil", "discrete", "input", "holding"];

export function entryKey(area: Area, address: number, bit: number | null) {
  return `${area}:${address}:${bit ?? ""}`;
}

function formatValue(v: Value): string {
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "boolean") return v ? "1" : "0";
  return String(v);
}

/** The writable registers of a map that have a value from the last read. */
export function collect(
  registers: Register[],
  results: Map<number, PointResult>,
): TemplateEntry[] {
  const entries: TemplateEntry[] = [];
  registers.forEach((reg, index) => {
    if (reg.access === "R") return;
    const r = results.get(index);
    if (!r || r.error || r.sentinel || r.value === null) return;
    entries.push({
      area: reg.area,
      address: reg.address,
      bit: reg.bit,
      value: r.value,
      name: reg.name,
    });
  });
  return entries;
}

export function serialize(
  device: string,
  entries: TemplateEntry[],
  date = new Date(),
): string {
  const lines = [
    "// Modbench value template",
    `// Device: ${device}`,
    `// Date: ${date.toISOString().slice(0, 16).replace("T", " ")}`,
    "",
  ];
  for (const e of entries) {
    const where = `${e.area} ${e.address}${e.bit === null ? "" : `.${e.bit}`}`;
    const note = e.name.replace(/[\r\n]+/g, " ");
    lines.push(`${where} = ${formatValue(e.value)}  // ${note}`);
  }
  return lines.join("\n") + "\n";
}

function parseValue(text: string): Value | null {
  if (text.startsWith('"')) {
    try {
      const s: unknown = JSON.parse(text);
      return typeof s === "string" ? s : null;
    } catch {
      return null;
    }
  }
  const asInt = parseInteger(text);
  if (asInt !== null) return asInt;
  const n = Number(text.replace(",", "."));
  if (text !== "" && Number.isFinite(n)) return n;
  return parseBool(text);
}

/** Splits a line into its code and its `//` comment, ignoring quoted text. */
function splitComment(line: string): [string, string] {
  let quoted = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (quoted && c === "\\") i++;
    else if (c === '"') quoted = !quoted;
    else if (!quoted && c === "/" && line[i + 1] === "/")
      return [line.slice(0, i), line.slice(i + 2).trim()];
  }
  return [line, ""];
}

export function parse(text: string): ParsedTemplate {
  const out: ParsedTemplate = { device: "", entries: [], errors: [] };
  text.split(/\r?\n/).forEach((raw, i) => {
    const line = i + 1;
    const [code, comment] = splitComment(raw);
    const header = /^\s*Device:\s*(.*)$/i.exec(code.trim() ? "" : comment);
    if (header) out.device = header[1].trim();
    const trimmed = code.trim();
    if (!trimmed) return;
    const m = /^([a-z]+)\s+([^\s=.]+)(?:\.(\d+))?\s*=\s*(.+)$/i.exec(trimmed);
    const area = m ? AREAS.find((a) => a === m[1].toLowerCase()) : undefined;
    if (!m || !area) {
      out.errors.push({
        line,
        message: "Expected `holding 16 = 25.5` (area, address, value).",
      });
      return;
    }
    const address = parseInteger(m[2]);
    if (address === null || address < 0 || address > 65535) {
      out.errors.push({ line, message: `Bad address ${m[2]}.` });
      return;
    }
    const bit = m[3] === undefined ? null : Number(m[3]);
    if (bit !== null && bit > 15) {
      out.errors.push({ line, message: `Bad bit ${bit} (0-15).` });
      return;
    }
    const value = parseValue(m[4].trim());
    if (value === null) {
      out.errors.push({ line, message: `Bad value ${m[4].trim()}.` });
      return;
    }
    out.entries.push({ area, address, bit, value, name: comment });
  });
  return out;
}

export interface PlannedWrite {
  index: number;
  register: Register;
  value: Value;
  /** The value the device showed at the last read, if any. */
  current: Value | null;
  /** True when the device does not already hold this value. */
  changed: boolean;
  /** Why this value cannot be written, or null. */
  problem: string | null;
}

export interface TemplatePlan {
  writes: PlannedWrite[];
  /** Entries of the template that match no register of the map. */
  unmatched: TemplateEntry[];
}

function same(a: Value | null, b: Value, reg: Register): boolean {
  if (a === null) return false;
  if (typeof a === "number" && typeof b === "number")
    return Math.abs(a - b) < Math.max(reg.scale, 1e-9) / 2;
  return a === b;
}

/** Matches a template against a map and the values last read from it. */
export function plan(
  registers: Register[],
  results: Map<number, PointResult>,
  entries: TemplateEntry[],
): TemplatePlan {
  const byKey = new Map<string, number>();
  registers.forEach((r, i) => byKey.set(entryKey(r.area, r.address, r.bit), i));
  const out: TemplatePlan = { writes: [], unmatched: [] };
  for (const e of entries) {
    const index = byKey.get(entryKey(e.area, e.address, e.bit));
    if (index === undefined) {
      out.unmatched.push(e);
      continue;
    }
    const register = registers[index];
    const r = results.get(index);
    const current = r && !r.error && !r.sentinel ? r.value : null;
    let problem: string | null;
    if (register.access === "R") problem = `${register.name} is read-only.`;
    else if (typeof e.value === "string" && register.type !== "string")
      problem = `${register.name} does not take text.`;
    else problem = limitError(register, e.value);
    out.writes.push({
      index,
      register,
      value: e.value,
      current,
      changed: !same(current, e.value, register),
      problem,
    });
  }
  return out;
}
