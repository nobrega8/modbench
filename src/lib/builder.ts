// Pure parts of the map builder: blank maps and registers, the keeping of a
// register's type and access valid when its area changes, state labels as
// text, addresses as the manual numbers them, and the validator's messages
// sorted by the row they belong to.

import type { Area, Diagnostic, Meta, Register } from "./types";

export const isBitArea = (area: Area) => area === "coil" || area === "discrete";

/** A map header with the factory defaults a map usually starts from. */
export function blankMeta(): Meta {
  return {
    vendor: "",
    model: "",
    vendor_website: null,
    description: null,
    firmware: null,
    source: null,
    protocol: null,
    unit_id: 1,
    port: 502,
    baudrate: 9600,
    parity: "N",
    data_bits: 8,
    stop_bits: 1,
    byte_order: "ABCD",
    address_base: 0,
    max_read_registers: null,
    max_read_bits: null,
    enums: [],
    sentinels: [],
    extra: [],
    notes: [],
    models: [],
    revision: 1,
    revised: null,
    sources: [],
  };
}

/** A register with every field at its default. */
export function blankRegister(over: Partial<Register> = {}): Register {
  return {
    name: "",
    group: "",
    area: "holding",
    address: 0,
    type: "uint16",
    length: null,
    scale: 1,
    scale_factor: null,
    offset: 0,
    unit: "",
    access: "R",
    write_min: null,
    write_max: null,
    reference: null,
    byte_order: null,
    bit: null,
    bits: null,
    enum: [],
    enum_ref: null,
    description: "",
    extra: [],
    ...over,
  };
}

/**
 * The register after its area changed: bits hold booleans only, input and
 * discrete areas cannot be written, and a word register cannot be a bool
 * unless it picks one bit of the word.
 */
export function withArea(r: Register, area: Area): Register {
  const next: Register = { ...r, area };
  if (isBitArea(area)) {
    next.type = "bool";
    next.scale = 1;
    next.offset = 0;
    next.unit = "";
    next.bit = null;
    next.bits = null;
    next.length = null;
  } else if (r.type === "bool" && r.bit === null) {
    next.type = "uint16";
  }
  if ((area === "input" || area === "discrete") && next.access !== "R") {
    next.access = "R";
  }
  return next;
}

/** State labels as `0=Off, 1=On`. */
export function enumText(pairs: [number, string][]): string {
  return pairs.map(([v, l]) => `${v}=${l}`).join(", ");
}

/**
 * State labels from text like `0=Off, 1=On` (values in decimal or 0x hex),
 * or null when a part is not a `value=label` pair. Empty text is no labels.
 */
export function parseEnum(text: string): [number, string][] | null {
  const parts = text
    .split(",")
    .map((p) => p.trim())
    .filter((p) => p !== "");
  const out: [number, string][] = [];
  for (const part of parts) {
    const eq = part.indexOf("=");
    if (eq <= 0) return null;
    const key = part.slice(0, eq).trim();
    const label = part.slice(eq + 1).trim();
    if (!/^-?(\d+|0x[0-9a-f]+)$/i.test(key) || label === "") return null;
    const value = Number(key);
    if (!Number.isSafeInteger(value) || out.some(([v]) => v === value))
      return null;
    out.push([value, label]);
  }
  return out;
}

/** The address as the manual (and the file) numbers it. */
export const fileAddress = (r: Register, base: number | null) =>
  r.address + (base ?? 0);

/** The wire address for a number typed as in the manual, or null. */
export function wireAddress(typed: number, base: number | null): number | null {
  const wire = typed - (base ?? 0);
  return Number.isInteger(wire) && wire >= 0 && wire <= 0xffff ? wire : null;
}

/** The validator's messages for each register row, and those for the file. */
export function sortDiagnostics(list: Diagnostic[]): {
  rows: Map<number, Diagnostic[]>;
  file: Diagnostic[];
} {
  const rows = new Map<number, Diagnostic[]>();
  const file: Diagnostic[] = [];
  for (const d of list) {
    const m = d.pointer?.match(/^\/registers\/(\d+)(\/|$)/);
    if (!m) {
      file.push(d);
      continue;
    }
    const index = Number(m[1]);
    rows.set(index, [...(rows.get(index) ?? []), d]);
  }
  return { rows, file };
}

/** A file name for a map: `vendor-model.mbmap.json`, lowercase with dashes. */
export function mapFileName(meta: Pick<Meta, "vendor" | "model">): string {
  const slug = (s: string) =>
    s
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
  const parts = [slug(meta.vendor), slug(meta.model)].filter(Boolean);
  return `${parts.length ? parts.join("-") : "map"}.mbmap.json`;
}
