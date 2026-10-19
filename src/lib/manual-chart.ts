// The Chart tab of manual reads: addresses of the Read registers grid, each
// with the type the grid shows it as, plotted while the loop runs.

import { formatRaw } from "./format";
import { MAX_POINTS, type Sample } from "./trend";
import type { Area, ByteOrder, RawData } from "./types";

/** How the Read registers grid shows values. */
export type GridFormat =
  | "unsigned"
  | "signed"
  | "hex"
  | "binary"
  | "ascii"
  | "int32"
  | "uint32"
  | "float32"
  | "bits";

/** The number a charted address is read as. */
export type ManualType =
  "bool" | "uint16" | "int16" | "uint32" | "int32" | "float32";

export interface ManualSeries {
  area: Area;
  address: number;
  type: ManualType;
  /** Byte order of 32-bit types. */
  order: ByteOrder;
  /** Color slot 1-8; also tells the series apart. */
  slot: number;
}

/** What a grid format plots as in an area; null when it cannot be charted. */
export function chartType(format: GridFormat, area: Area): ManualType | null {
  if (area === "coil" || area === "discrete") return "bool";
  switch (format) {
    case "ascii":
      return null;
    case "signed":
      return "int16";
    case "int32":
    case "uint32":
    case "float32":
      return format;
    default:
      return "uint16";
  }
}

/** Registers (or bits) a type takes. */
export function width(type: ManualType): number {
  return type === "uint32" || type === "int32" || type === "float32" ? 2 : 1;
}

/** Two registers as a 32-bit value in a byte order. */
export function decodePair(
  a: number,
  b: number,
  type: "uint32" | "int32" | "float32",
  order: ByteOrder,
): number {
  let regs = [a, b];
  if (order === "CDAB" || order === "DCBA") regs = regs.reverse();
  const swap = order === "BADC" || order === "DCBA";
  const view = new DataView(new ArrayBuffer(4));
  regs.forEach((w, k) =>
    view.setUint16(k * 2, swap ? ((w & 0xff) << 8) | (w >> 8) : w),
  );
  if (type === "int32") return view.getInt32(0);
  if (type === "uint32") return view.getUint32(0);
  return Number(view.getFloat32(0).toPrecision(7));
}

/** One way of copying a word: which reading, and the text to copy. */
export interface CopyForm {
  id: "hex" | "unsigned" | "signed" | "binary" | "int32" | "uint32" | "float32";
  text: string;
}

/**
 * The ways to copy the word at `at` (or the pair of words, when `span` is
 * 2), whatever format the grid shows. Hex is the text the Device map copies
 * as a raw value; the 32-bit readings pair the word with the next one, in
 * the grid's byte order, and are left out at the end of the read.
 */
export function copyForms(
  words: number[],
  at: number,
  span: 1 | 2,
  order: ByteOrder,
): CopyForm[] {
  const first = words[at];
  if (first === undefined) return [];
  const used = words.slice(at, at + span);
  const out: CopyForm[] = [
    { id: "hex", text: formatRaw({ kind: "registers", values: used }) },
  ];
  if (span === 1) {
    out.push(
      { id: "unsigned", text: String(first) },
      { id: "signed", text: String(first > 0x7fff ? first - 0x10000 : first) },
      { id: "binary", text: first.toString(2).padStart(16, "0") },
    );
  }
  const next = words[at + 1];
  if (next !== undefined) {
    for (const id of ["int32", "uint32", "float32"] as const) {
      out.push({ id, text: String(decodePair(first, next, id, order)) });
    }
  }
  return out;
}

/** Same address, area and reading: the same series. */
export function sameSeries(
  s: Omit<ManualSeries, "slot">,
  t: Omit<ManualSeries, "slot">,
): boolean {
  return (
    s.area === t.area &&
    s.address === t.address &&
    s.type === t.type &&
    (width(s.type) === 1 || s.order === t.order)
  );
}

export interface ManualRead {
  address: number;
  area: Area;
  data: RawData;
}

/** Whether a read has every register of a series. */
export function covers(read: ManualRead, s: ManualSeries): boolean {
  const n = read.data.values.length;
  return (
    read.area === s.area &&
    s.address >= read.address &&
    s.address + width(s.type) <= read.address + n
  );
}

/** A series' value in a read that covers it. */
export function valueIn(read: ManualRead, s: ManualSeries): number {
  const i = s.address - read.address;
  if (read.data.kind === "bits") return read.data.values[i] ? 1 : 0;
  const w = read.data.values;
  switch (s.type) {
    case "int16":
      return w[i] > 0x7fff ? w[i] - 0x10000 : w[i];
    case "uint32":
    case "int32":
    case "float32":
      return decodePair(w[i], w[i + 1], s.type, s.order);
    default:
      return w[i];
  }
}

/**
 * Adds one read to the history: a sample for every series the read still
 * covers, and the series it does not cover are dropped (step 2 now reads
 * other addresses). `read` null is a failed read: a gap in every line.
 */
export function addRead(
  series: ManualSeries[],
  history: Map<number, Sample[]>,
  read: ManualRead | null,
  t: number,
): { series: ManualSeries[]; history: Map<number, Sample[]> } {
  const kept = read ? series.filter((s) => covers(read, s)) : series;
  const next = new Map<number, Sample[]>();
  for (const s of kept) {
    const list = history.get(s.slot) ?? [];
    list.push(read ? { t, v: valueIn(read, s) } : { t, v: NaN, q: "failed" });
    if (list.length > MAX_POINTS) list.splice(0, list.length - MAX_POINTS);
    next.set(s.slot, list);
  }
  return {
    series: kept.length === series.length ? series : kept,
    history: next,
  };
}

const AREA_SHORT: Record<Area, string> = {
  coil: "CO",
  discrete: "DI",
  input: "IR",
  holding: "HR",
};

/** A series' name in the legend: area, address and how it is read. */
export function seriesName(s: ManualSeries, address: string): string {
  const how =
    s.type === "bool"
      ? ""
      : width(s.type) === 2
        ? ` · ${s.type} ${s.order}`
        : ` · ${s.type}`;
  return `${AREA_SHORT[s.area]} ${address}${how}`;
}
