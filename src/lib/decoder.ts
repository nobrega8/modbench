// Every reading of some registers at once, for the data decoder: integers,
// floats and text in the four byte orders, and each register on its own.

import { hex } from "./format";
import { t } from "./i18n.svelte.ts";
import type { ByteOrder } from "./types";

export const ORDERS: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];

/**
 * Registers from typed text. Hex is the default ("4148 0000", "41 48 00
 * 00", "0x4148"); with `decimal`, whole registers ("16712 0").
 * Returns null when the text cannot be read.
 */
export function parseWords(text: string, decimal = false): number[] | null {
  const t = text.trim();
  if (!t) return null;
  if (decimal) {
    const parts = t.split(/[\s,;]+/);
    const words = parts.map((p) => Number(p));
    return words.every((w) => Number.isInteger(w) && w >= -32768 && w <= 65535)
      ? words.map((w) => w & 0xffff)
      : null;
  }
  const clean = t.replace(/0x/gi, "").replace(/[\s,;:-]+/g, "");
  if (!clean || /[^0-9a-f]/i.test(clean) || clean.length % 4) return null;
  const words: number[] = [];
  for (let i = 0; i < clean.length; i += 4)
    words.push(parseInt(clean.slice(i, i + 4), 16));
  return words;
}

/**
 * Bytes most significant first, from registers in wire order (the same
 * rule as the backend's codec: CDAB/DCBA reverse the registers, BADC/DCBA
 * swap the bytes inside each one).
 */
export function normalize(words: number[], order: ByteOrder): number[] {
  const list =
    order === "CDAB" || order === "DCBA" ? [...words].reverse() : words;
  const swap = order === "BADC" || order === "DCBA";
  return list.flatMap((w) => (swap ? [w & 0xff, w >> 8] : [w >> 8, w & 0xff]));
}

function view(bytes: number[]): DataView {
  return new DataView(Uint8Array.from(bytes).buffer);
}

function float(f: number, digits: number): string {
  if (Number.isNaN(f)) return "NaN";
  if (!Number.isFinite(f)) return f > 0 ? "+∞" : "−∞";
  return String(Number(f.toPrecision(digits)));
}

function text(bytes: number[]): string {
  return bytes
    .map((c) => (c >= 32 && c < 127 ? String.fromCharCode(c) : "·"))
    .join("");
}

/** A register as binary-coded decimal, or null when a nibble is above 9. */
export function bcd(w: number): string | null {
  const digits = [12, 8, 4, 0].map((s) => (w >> s) & 0xf);
  return digits.every((d) => d <= 9) ? digits.join("") : null;
}

export interface Row {
  label: string;
  /** One reading per byte order, in the order of ORDERS. */
  values: string[];
}

/** Multi-register readings, one row per type, one column per byte order. */
export function wideRows(words: number[]): Row[] {
  const rows: Row[] = [];
  const each = (
    label: string,
    count: number,
    read: (v: DataView) => string,
  ) => {
    if (words.length < count) return;
    rows.push({
      label,
      values: ORDERS.map((o) =>
        read(view(normalize(words.slice(0, count), o))),
      ),
    });
  };
  each(t("decoder.int32"), 2, (v) => String(v.getInt32(0)));
  each(t("decoder.uint32"), 2, (v) => String(v.getUint32(0)));
  each(t("decoder.float32"), 2, (v) => float(v.getFloat32(0), 7));
  each(t("decoder.int64"), 4, (v) => String(v.getBigInt64(0)));
  each(t("decoder.uint64"), 4, (v) => String(v.getBigUint64(0)));
  each(t("decoder.float64"), 4, (v) => float(v.getFloat64(0), 15));
  // Text keeps the register order; only the byte swap applies.
  rows.push({
    label: t("decoder.text"),
    values: ORDERS.map((o) =>
      text(normalize(words, o === "BADC" || o === "DCBA" ? "BADC" : "ABCD")),
    ),
  });
  return rows;
}

export interface WordRow {
  hex: string;
  unsigned: number;
  signed: number;
  binary: string;
  bcd: string | null;
  /** Unsigned value with the two bytes swapped. */
  swapped: number;
}

export function wordRows(words: number[]): WordRow[] {
  return words.map((w) => ({
    hex: hex(w),
    unsigned: w,
    signed: w > 0x7fff ? w - 0x10000 : w,
    binary: w
      .toString(2)
      .padStart(16, "0")
      .replace(/(.{4})(?!$)/g, "$1 "),
    bcd: bcd(w),
    swapped: ((w & 0xff) << 8) | (w >> 8),
  }));
}
