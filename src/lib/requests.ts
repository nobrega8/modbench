// Requests built by hand for the Send request tool: the data bytes that
// follow the function code, from typed fields or from hex.

import { parseHex } from "./frames";

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

const word = (key: string, label: string, initial = "0", hint?: string) =>
  ({ key, label, kind: "word", initial, hint }) as RequestField;

/** The functions offered, with their fields; others take hex data. */
export const FUNCTIONS: FunctionSpec[] = [
  {
    fc: 1,
    name: "Read coils",
    fields: [word("a", "Address"), word("n", "Count", "1")],
  },
  {
    fc: 2,
    name: "Read discrete inputs",
    fields: [word("a", "Address"), word("n", "Count", "1")],
  },
  {
    fc: 3,
    name: "Read holding registers",
    fields: [word("a", "Address"), word("n", "Count", "1")],
  },
  {
    fc: 4,
    name: "Read input registers",
    fields: [word("a", "Address"), word("n", "Count", "1")],
  },
  {
    fc: 5,
    name: "Write single coil",
    fields: [
      word("a", "Address"),
      word("v", "Value", "0xFF00", "0xFF00 on, 0x0000 off"),
    ],
  },
  {
    fc: 6,
    name: "Write single register",
    fields: [word("a", "Address"), word("v", "Value")],
  },
  { fc: 7, name: "Read exception status", fields: [] },
  {
    fc: 8,
    name: "Diagnostics",
    fields: [
      word("sub", "Sub-function", "0", "0 returns the data (echo)"),
      { key: "data", label: "Data (hex)", kind: "hex", initial: "A5 37" },
    ],
  },
  { fc: 11, name: "Get comm event counter", fields: [] },
  { fc: 12, name: "Get comm event log", fields: [] },
  {
    fc: 15,
    name: "Write multiple coils",
    fields: [
      word("a", "Address"),
      { key: "bits", label: "Values (0/1)", kind: "words", initial: "1 0 1" },
    ],
  },
  {
    fc: 16,
    name: "Write multiple registers",
    fields: [
      word("a", "Address"),
      { key: "values", label: "Values", kind: "words", initial: "1 2" },
    ],
  },
  { fc: 17, name: "Report server ID", fields: [] },
  {
    fc: 20,
    name: "Read file record",
    fields: [
      word("file", "File", "1"),
      word("record", "Record"),
      word("n", "Registers", "1"),
    ],
  },
  {
    fc: 22,
    name: "Mask write register",
    fields: [
      word("a", "Address"),
      word("and", "AND mask", "0xFFFF"),
      word("or", "OR mask", "0x0000"),
    ],
  },
  {
    fc: 23,
    name: "Read/write multiple registers",
    fields: [
      word("ra", "Read address"),
      word("rn", "Read count", "1"),
      word("wa", "Write address"),
      { key: "values", label: "Write values", kind: "words", initial: "0" },
    ],
  },
  { fc: 24, name: "Read FIFO queue", fields: [word("a", "FIFO address")] },
  {
    fc: 43,
    name: "Read device identification",
    fields: [
      {
        key: "code",
        label: "Read code",
        kind: "byte",
        initial: "1",
        hint: "1 basic, 2 regular, 3 extended, 4 one object",
      },
      { key: "object", label: "Object ID", kind: "byte", initial: "0" },
    ],
  },
];

function num(text: string, max: number, label: string): number {
  const t = text.trim();
  const n = /^0x[0-9a-f]+$/i.test(t)
    ? parseInt(t, 16)
    : /^\d+$/.test(t)
      ? Number(t)
      : NaN;
  if (!Number.isInteger(n) || n < 0 || n > max)
    throw new Error(`${label}: a whole number from 0 to ${max}`);
  return n;
}

function list(text: string, max: number, label: string): number[] {
  const parts = text.split(/[\s,;]+/).filter(Boolean);
  if (!parts.length) throw new Error(`${label}: at least one value`);
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
      if (hex === null) throw new Error("Data: hex bytes, e.g. A5 37");
      return [...be(w("sub")), ...hex];
    }
    case 15: {
      const bits = list(v("bits"), 1, "Values");
      const bytes = Array.from({ length: Math.ceil(bits.length / 8) }, (_, i) =>
        bits.slice(i * 8, i * 8 + 8).reduce((acc, b, k) => acc | (b << k), 0),
      );
      return [...be(w("a")), ...be(bits.length), bytes.length, ...bytes];
    }
    case 16: {
      const values = list(v("values"), 0xffff, "Values");
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
      const values = list(v("values"), 0xffff, "Write values");
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
        num(v("code"), 255, "Read code"),
        num(v("object"), 255, "Object ID"),
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
