// Formatting of values for display and parsing of what people type.

import type { Area, RawData, Register, Value } from "./types";

export const AREA_TAG: Record<Area, string> = {
  coil: "CO",
  discrete: "DI",
  input: "IR",
  holding: "HR",
};

export const AREA_NAME: Record<Area, string> = {
  coil: "Coil",
  discrete: "Discrete input",
  input: "Input register",
  holding: "Holding register",
};

export const FUNCTION_NAME: Record<number, string> = {
  1: "Read coils",
  2: "Read discrete inputs",
  3: "Read holding registers",
  4: "Read input registers",
  5: "Write single coil",
  6: "Write single register",
  15: "Write multiple coils",
  16: "Write multiple registers",
};

export function hex(n: number, digits = 4): string {
  return n.toString(16).toUpperCase().padStart(digits, "0");
}

/**
 * Integer typed by a person: decimal, `0x1F` or `1Fh` (the suffix form is
 * common in device manuals). Returns null if it is not a whole number.
 */
export function parseInteger(text: string): number | null {
  const t = text.trim();
  let m = /^([+-]?)0x([0-9a-f]+)$/i.exec(t) ?? /^([+-]?)([0-9a-f]+)h$/i.exec(t);
  if (m) return (m[1] === "-" ? -1 : 1) * parseInt(m[2], 16);
  m = /^[+-]?\d+$/.exec(t);
  return m ? parseInt(t, 10) : null;
}

export function parseBool(text: string): boolean | null {
  const t = text.trim().toLowerCase();
  if (["1", "true", "on", "yes"].includes(t)) return true;
  if (["0", "false", "off", "no"].includes(t)) return false;
  return null;
}

/** Turns typed text into a value for a register, or explains why not. */
export function parseForRegister(
  reg: Register,
  text: string,
): { value: Value } | { error: string } {
  if (reg.type === "bool") {
    const b = parseBool(text);
    return b === null ? { error: "Type 1 or 0 (on or off)." } : { value: b };
  }
  if (reg.type === "string") return { value: text };
  const label = reg.enum.find(
    ([, l]) => l.toLowerCase() === text.trim().toLowerCase(),
  );
  if (label) return { value: label[0] };
  const asInt = parseInteger(text);
  if (asInt !== null) return { value: asInt };
  const n = Number(text.trim().replace(",", "."));
  if (text.trim() !== "" && Number.isFinite(n)) return { value: n };
  return { error: "Type a number, e.g. 12.5, 0x1F or 1Fh." };
}

export function formatValue(reg: Register, value: Value | null): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "boolean") {
    const label = reg.enum.find(([v]) => v === (value ? 1 : 0));
    return label ? label[1] : value ? "ON" : "OFF";
  }
  if (typeof value === "string") return value;
  // Labels match the raw value, before scale and offset.
  const raw = Math.round((value - reg.offset) / reg.scale);
  const label = reg.enum.find(([v]) => v === raw);
  if (label) return label[1];
  if (Number.isInteger(value)) return value.toString();
  // Enough digits for the scale, without float noise.
  const decimals =
    reg.scale !== 1 && reg.scale > 0
      ? Math.min(6, Math.max(0, Math.ceil(-Math.log10(reg.scale))))
      : 3;
  return value.toFixed(decimals).replace(/\.?0+$/, "") || "0";
}

export function formatRaw(raw: RawData | null): string {
  if (!raw) return "";
  if (raw.kind === "bits")
    return raw.values.map((b) => (b ? "1" : "0")).join("");
  return raw.values.map((w) => hex(w)).join(" ");
}

export function clock(ms: number): string {
  const d = new Date(ms);
  const p = (n: number, w = 2) => n.toString().padStart(w, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`;
}
