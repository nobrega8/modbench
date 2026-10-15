// Registers a person adds to an open map, for ones the map is missing. They
// are kept per map file on this computer (like alarms), not written into the
// file, and marked so the UI can tell them apart and remove them.

import type { Access, Area, DataType, Register } from "./types";

const PREFIX = "modbench.custom.";
/** `extra` column that marks a register added here. */
export const CUSTOM_MARK = "x_custom";

export interface CustomFields {
  name: string;
  area: Area;
  address: number;
  type: DataType;
  /** Registers, only for strings. */
  length: number | null;
  scale: number;
  offset: number;
  unit: string;
  access: Access;
}

export function isCustom(r: Register): boolean {
  return r.extra.some(([k]) => k === CUSTOM_MARK);
}

/** Registers one value takes. */
export function wordCount(type: DataType, length: number | null): number {
  if (type === "string") return length ?? 1;
  if (["int32", "uint32", "float32"].includes(type)) return 2;
  if (["int64", "uint64", "float64"].includes(type)) return 4;
  return 1;
}

/** What is wrong with the fields, or null. */
export function customProblem(
  f: CustomFields,
  taken: Register[],
): string | null {
  if (!f.name.trim()) return "Give the register a name.";
  if (
    taken.some(
      (r) => r.name.trim().toLowerCase() === f.name.trim().toLowerCase(),
    )
  )
    return "The map already has a register with this name.";
  if (!Number.isInteger(f.address) || f.address < 0)
    return "Type an address from 0 to 65535.";
  if (f.address + wordCount(f.type, f.length) > 0x10000)
    return "The value does not fit below address 65535.";
  const bits = f.area === "coil" || f.area === "discrete";
  if (bits && f.type !== "bool")
    return "Coils and discrete inputs hold single bits: use bool.";
  if (!bits && f.type === "bool")
    return "A bool in a register needs a bit; pick a number type.";
  if (f.type === "string" && !(f.length && f.length > 0))
    return "A string needs its length in registers.";
  if (!Number.isFinite(f.scale) || f.scale === 0)
    return "The scale must be a number other than 0.";
  if (!Number.isFinite(f.offset)) return "The offset must be a number.";
  return null;
}

/** A full register from the form's fields, marked as custom. */
export function customRegister(f: CustomFields): Register {
  const readOnly = f.area === "input" || f.area === "discrete";
  return {
    name: f.name.trim(),
    group: "",
    area: f.area,
    address: f.address,
    type: f.type,
    length: f.type === "string" ? f.length : null,
    scale: f.scale,
    scale_factor: null,
    offset: f.offset,
    unit: f.unit.trim(),
    access: readOnly ? "R" : f.access,
    write_min: null,
    write_max: null,
    reference: null,
    byte_order: null,
    bit: null,
    bits: null,
    enum: [],
    enum_ref: null,
    description: "",
    extra: [[CUSTOM_MARK, "true"]],
    comment: "",
    firmware_min: null,
    firmware_max: null,
    role: null,
  };
}

type Store = Pick<Storage, "getItem" | "setItem" | "removeItem">;

function defaultStore(): Store | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export function loadCustom(
  mapPath: string,
  store = defaultStore(),
): Register[] {
  try {
    const raw = store?.getItem(PREFIX + mapPath);
    const list = raw ? (JSON.parse(raw) as Register[]) : [];
    return Array.isArray(list) ? list.filter((r) => r && isCustom(r)) : [];
  } catch {
    return [];
  }
}

export function saveCustom(
  mapPath: string,
  registers: Register[],
  store = defaultStore(),
) {
  try {
    const kept = registers.filter(isCustom);
    if (kept.length) store?.setItem(PREFIX + mapPath, JSON.stringify(kept));
    else store?.removeItem(PREFIX + mapPath);
  } catch {
    // Storage unavailable: they last for this session only.
  }
}
