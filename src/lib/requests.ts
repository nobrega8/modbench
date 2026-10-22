// Requests built by hand for the Send request tool: the data bytes that
// follow the function code, from typed fields or from hex.

import { parseHex } from "./frames";
import { td, t } from "./i18n.svelte.ts";

/** Whether a request only reads (as the backend decides; see modbus.rs). */
export function onlyReads(fc: number, data: number[]): boolean {
  if ([1, 2, 3, 4, 7, 11, 12, 17, 20, 24, 43].includes(fc)) return true;
  return fc === 8 && data[0] === 0 && data[1] === 0;
}

/** A field of a typed request. */
export interface RequestField {
  key: string;
  label: string;
  /** "word": 0-65535, decimal or 0x hex; "byte": 0-255; "words": a list. */
  kind: "word" | "byte" | "words" | "hex";
  initial: string;
  hint?: string;
}

export interface FunctionSpec {
  fc: number;
  name: string;
  fields: RequestField[];
}

/** A field whose label and hint are looked up when read, so they follow the language. */
const field = (
  key: string,
  label: string,
  kind: RequestField["kind"],
  initial: string,
  hint?: string,
): RequestField => ({
  key,
  get label() {
    return td(label);
  },
  kind,
  initial,
  get hint() {
    return hint ? td(hint) : undefined;
  },
});

const word = (key: string, label: string, initial = "0", hint?: string) =>
  field(key, label, "word", initial, hint);

const spec = (fc: number, fields: RequestField[]): FunctionSpec => ({
  fc,
  get name() {
    return td(`fc.${fc}`);
  },
  fields,
});

/** The functions offered, with their fields; others take hex data. */
export const FUNCTIONS: FunctionSpec[] = [
  spec(1, [word("a", "reqf.address"), word("n", "reqf.count", "1")]),
  spec(2, [word("a", "reqf.address"), word("n", "reqf.count", "1")]),
  spec(3, [word("a", "reqf.address"), word("n", "reqf.count", "1")]),
  spec(4, [word("a", "reqf.address"), word("n", "reqf.count", "1")]),
  spec(5, [
    word("a", "reqf.address"),
    word("v", "reqf.value", "0xFF00", "reqf.hint.coil"),
  ]),
  spec(6, [word("a", "reqf.address"), word("v", "reqf.value")]),
  spec(7, []),
  spec(8, [
    word("sub", "reqf.subfunction", "0", "reqf.hint.echo"),
    field("data", "reqf.data_hex", "hex", "A5 37"),
  ]),
  spec(11, []),
  spec(12, []),
  spec(15, [
    word("a", "reqf.address"),
    field("bits", "reqf.bits", "words", "1 0 1"),
  ]),
  spec(16, [
    word("a", "reqf.address"),
    field("values", "reqf.values", "words", "1 2"),
  ]),
  spec(17, []),
  spec(20, [
    word("file", "reqf.file", "1"),
    word("record", "reqf.record"),
    word("n", "reqf.registers", "1"),
  ]),
  spec(22, [
    word("a", "reqf.address"),
    word("and", "reqf.and_mask", "0xFFFF"),
    word("or", "reqf.or_mask", "0x0000"),
  ]),
  spec(23, [
    word("ra", "reqf.read_address"),
    word("rn", "reqf.read_count", "1"),
    word("wa", "reqf.write_address"),
    field("values", "reqf.write_values", "words", "0"),
  ]),
  spec(24, [word("a", "reqf.fifo_address")]),
  spec(43, [
    field("code", "reqf.read_code", "byte", "1", "reqf.hint.read_code"),
    field("object", "reqf.object_id", "byte", "0"),
  ]),
];

function num(text: string, max: number, label: string): number {
  const trimmed = text.trim();
  const n = /^0x[0-9a-f]+$/i.test(trimmed)
    ? parseInt(trimmed, 16)
    : /^\d+$/.test(trimmed)
      ? Number(trimmed)
      : NaN;
  if (!Number.isInteger(n) || n < 0 || n > max)
    throw new Error(t("reqf.err.number", { label, max }));
  return n;
}

function list(text: string, max: number, label: string): number[] {
  const parts = text.split(/[\s,;]+/).filter(Boolean);
  if (!parts.length) throw new Error(t("reqf.err.list", { label }));
  return parts.map((p) => num(p, max, label));
}

const be = (w: number) => [w >> 8, w & 0xff];

/** The data bytes of a typed request, or an error naming the field. */
export function buildData(
  spec: FunctionSpec,
  values: Record<string, string>,
): number[] {
  const v = (key: string) => values[key] ?? "";
  const w = (key: string) => {
    const f = spec.fields.find((x) => x.key === key)!;
    return num(v(key), 0xffff, f.label);
  };
  switch (spec.fc) {
    case 1:
    case 2:
    case 3:
    case 4:
      return [...be(w("a")), ...be(w("n"))];
    case 5:
    case 6:
      return [...be(w("a")), ...be(w("v"))];
    case 8: {
      const hex = parseHex(v("data"));
      if (hex === null)
        throw new Error(t("reqf.err.hex", { label: td("reqf.data_hex") }));
      return [...be(w("sub")), ...hex];
    }
    case 15: {
      const bits = list(v("bits"), 1, td("reqf.values"));
      const bytes = Array.from({ length: Math.ceil(bits.length / 8) }, (_, i) =>
        bits.slice(i * 8, i * 8 + 8).reduce((acc, b, k) => acc | (b << k), 0),
      );
      return [...be(w("a")), ...be(bits.length), bytes.length, ...bytes];
    }
    case 16: {
      const values = list(v("values"), 0xffff, td("reqf.values"));
      return [
        ...be(w("a")),
        ...be(values.length),
        values.length * 2,
        ...values.flatMap(be),
      ];
    }
    case 20:
      // One sub-request: reference type 6, file, record, length
      return [7, 6, ...be(w("file")), ...be(w("record")), ...be(w("n"))];
    case 22:
      return [...be(w("a")), ...be(w("and")), ...be(w("or"))];
    case 23: {
      const values = list(v("values"), 0xffff, td("reqf.write_values"));
      return [
        ...be(w("ra")),
        ...be(w("rn")),
        ...be(w("wa")),
        ...be(values.length),
        values.length * 2,
        ...values.flatMap(be),
      ];
    }
    case 24:
      return be(w("a"));
    case 43:
      return [
        14,
        num(v("code"), 255, td("reqf.read_code")),
        num(v("object"), 255, td("reqf.object_id")),
      ];
    default:
      return [];
  }
}

/** Bytes as spaced hex, e.g. "16 00 04". */
export function hexBytes(bytes: number[]): string {
  return bytes
    .map((b) => b.toString(16).toUpperCase().padStart(2, "0"))
    .join(" ");
}
