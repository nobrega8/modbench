// Field-by-field reading of Modbus frames, for the Raw data tab and the
// frame decoder: MBAP header (TCP), unit ID and CRC (RTU) or LRC (ASCII),
// then the PDU.

import { FUNCTION_NAME, hex } from "./format";

export type Framing = "tcp" | "rtu" | "ascii";
export type Direction = "request" | "response";

export interface Field {
  label: string;
  /** The bytes the field takes, as hex. */
  bytes: string;
  /** What they mean. */
  meaning: string;
  bad?: boolean;
}

export interface Decoded {
  fields: Field[];
  /** One line about the whole frame, e.g. "Read holding registers 0 × 10". */
  summary: string;
  /** Why the frame could not be read completely, if so. */
  error: string | null;
}

export const EXCEPTION_NAME: Record<number, string> = {
  1: "illegal function",
  2: "illegal data address",
  3: "illegal data value",
  4: "server device failure",
  5: "acknowledge",
  6: "server device busy",
  8: "memory parity error",
  10: "gateway path unavailable",
  11: "gateway target device failed to respond",
};

const MORE_FUNCTIONS: Record<number, string> = {
  7: "Read exception status",
  8: "Diagnostics",
  11: "Get comm event counter",
  12: "Get comm event log",
  17: "Report server ID",
  20: "Read file record",
  21: "Write file record",
  22: "Mask write register",
  23: "Read/write multiple registers",
  24: "Read FIFO queue",
  43: "Encapsulated interface (device identification)",
};

export function functionName(fc: number): string {
  return FUNCTION_NAME[fc] ?? MORE_FUNCTIONS[fc] ?? `Function ${fc}`;
}

/** CRC-16/MODBUS of some bytes. */
export function crc16(bytes: number[]): number {
  let crc = 0xffff;
  for (const b of bytes) {
    crc ^= b;
    for (let i = 0; i < 8; i++)
      crc = crc & 1 ? (crc >>> 1) ^ 0xa001 : crc >>> 1;
  }
  return crc;
}

/** LRC of Modbus ASCII: the two's complement of the byte sum. */
export function lrc(bytes: number[]): number {
  return -bytes.reduce((a, b) => a + b, 0) & 0xff;
}

/**
 * Bytes from text such as "01 03 00 00", "0103 0000", "0x01,0x03" or an
 * ASCII frame ":010300000001FB". Returns null when it is not hex.
 */
export function parseHex(text: string): number[] | null {
  const t = text.trim();
  if (t.startsWith(":"))
    return [...t].map((c) => c.charCodeAt(0)).filter((c) => c !== 32);
  const clean = t.replace(/0x/gi, "").replace(/[\s,;:-]+/g, "");
  if (!clean || clean.length % 2 || /[^0-9a-f]/i.test(clean)) return null;
  const out: number[] = [];
  for (let i = 0; i < clean.length; i += 2)
    out.push(parseInt(clean.slice(i, i + 2), 16));
  return out;
}

const hexOf = (bytes: number[]) => bytes.map((b) => hex(b, 2)).join(" ");
const word = (b: number[], i: number) => ((b[i] ?? 0) << 8) | (b[i + 1] ?? 0);

/** Reads a PDU (function code first) into fields. */
function pdu(
  b: number[],
  direction: Direction,
): {
  fields: Field[];
  summary: string;
  error: string | null;
} {
  const fields: Field[] = [];
  let at = 0;
  let error: string | null = null;
  const take = (n: number, label: string, meaning: (x: number[]) => string) => {
    if (at + n > b.length) {
      error ??= `The frame ends before its ${label.toLowerCase()}.`;
      at = b.length;
      return null;
    }
    const part = b.slice(at, at + n);
    at += n;
    fields.push({ label, bytes: hexOf(part), meaning: meaning(part) });
    return part;
  };
  const fcByte = take(1, "Function", (x) =>
    x[0] & 0x80
      ? `${x[0]} = exception to ${functionName(x[0] & 0x7f).toLowerCase()}`
      : `${x[0]} = ${functionName(x[0])}`,
  );
  if (!fcByte) return { fields, summary: "Empty frame", error };
  const fc = fcByte[0] & 0x7f;
  let summary = functionName(fc);

  if (fcByte[0] & 0x80) {
    const code = take(
      1,
      "Exception",
      (x) => `${x[0]} = ${EXCEPTION_NAME[x[0]] ?? "unknown exception"}`,
    );
    summary = `Exception ${code?.[0] ?? "?"} to ${summary.toLowerCase()}`;
  } else if (direction === "request" && fc >= 1 && fc <= 6) {
    const a = take(2, "Address", (x) => String(word(x, 0)));
    const v = take(2, fc <= 4 ? "Quantity" : "Value", (x) =>
      fc === 5
        ? word(x, 0) === 0xff00
          ? "ON"
          : word(x, 0) === 0
            ? "OFF"
            : `${word(x, 0)} (not a valid coil value)`
        : String(word(x, 0)),
    );
    if (a && v)
      summary += ` ${word(a, 0)}${fc <= 4 ? ` × ${word(v, 0)}` : ` ← ${fields.at(-1)!.meaning}`}`;
  } else if (direction === "request" && (fc === 15 || fc === 16)) {
    const a = take(2, "Address", (x) => String(word(x, 0)));
    const q = take(2, "Quantity", (x) => String(word(x, 0)));
    const n = take(1, "Byte count", (x) => String(x[0]));
    if (n) dataFields(fc === 15 ? "bits" : "words", n[0]);
    if (a && q) summary += ` ${word(a, 0)} × ${word(q, 0)}`;
  } else if (direction === "response" && fc >= 1 && fc <= 4) {
    const n = take(1, "Byte count", (x) => String(x[0]));
    if (n) {
      dataFields(fc <= 2 ? "bits" : "words", n[0]);
      summary +=
        fc <= 2 ? `: ${n[0]} bytes of bits` : `: ${n[0] / 2} registers`;
    }
  } else if (direction === "response" && (fc === 5 || fc === 6)) {
    take(2, "Address", (x) => String(word(x, 0)));
    take(2, "Value", (x) =>
      fc === 5 ? (word(x, 0) ? "ON" : "OFF") : String(word(x, 0)),
    );
    summary += " (echo)";
  } else if (direction === "response" && (fc === 15 || fc === 16)) {
    const a = take(2, "Address", (x) => String(word(x, 0)));
    const q = take(2, "Quantity", (x) => String(word(x, 0)));
    if (a && q) summary += ` ${word(a, 0)} × ${word(q, 0)} done`;
  } else if (fc === 43) {
    take(1, "MEI type", (x) =>
      x[0] === 14 ? "14 = read device identification" : String(x[0]),
    );
    take(
      1,
      "Read code",
      (x) =>
        ["", "1 = basic", "2 = regular", "3 = extended", "4 = one object"][
          x[0]
        ] ?? String(x[0]),
    );
    if (direction === "request") {
      take(1, "Object ID", (x) => String(x[0]));
    } else {
      take(1, "Conformity", (x) => `0x${hex(x[0], 2)}`);
      take(1, "More follows", (x) => (x[0] ? "yes" : "no"));
      take(1, "Next object", (x) => String(x[0]));
      const n = take(1, "Objects", (x) => String(x[0]));
      for (let k = 0; n && k < n[0] && at < b.length; k++) {
        const id = take(1, "Object ID", (x) => String(x[0]));
        const len = take(1, "Length", (x) => String(x[0]));
        if (!id || !len) break;
        take(len[0], `Object ${id[0]}`, (x) =>
          JSON.stringify(String.fromCharCode(...x)),
        );
      }
    }
  }
  if (at < b.length)
    fields.push({
      label: "Data",
      bytes: hexOf(b.slice(at)),
      meaning: `${b.length - at} bytes`,
    });
  return { fields, summary, error };

  function dataFields(kind: "bits" | "words", n: number) {
    const data = b.slice(at, at + n);
    if (data.length < n) error ??= "The frame ends before its data.";
    at += data.length;
    if (kind === "bits") {
      const bits = data.flatMap((byte) =>
        Array.from({ length: 8 }, (_, i) => (byte >> i) & 1),
      );
      fields.push({
        label: "Bits",
        bytes: hexOf(data),
        meaning: bits.join("") + " (first bit first)",
      });
    } else {
      const words = [];
      for (let i = 0; i + 1 < data.length; i += 2) words.push(word(data, i));
      fields.push({
        label: "Registers",
        bytes: hexOf(data),
        meaning: words.join(", "),
      });
    }
  }
}

/** Reads a whole frame as it travels on the wire. */
export function decodeFrame(
  bytes: number[],
  framing: Framing,
  direction: Direction,
): Decoded {
  const head: Field[] = [];
  const tail: Field[] = [];
  let body = bytes;
  let error: string | null = null;

  if (framing === "tcp") {
    if (bytes.length < 8)
      return {
        fields: [],
        summary: "Too short for Modbus TCP",
        error: "A Modbus TCP frame has at least 8 bytes.",
      };
    const length = word(bytes, 4);
    head.push(
      {
        label: "Transaction",
        bytes: hexOf(bytes.slice(0, 2)),
        meaning: String(word(bytes, 0)),
      },
      {
        label: "Protocol",
        bytes: hexOf(bytes.slice(2, 4)),
        meaning: word(bytes, 2) === 0 ? "0 = Modbus" : String(word(bytes, 2)),
        bad: word(bytes, 2) !== 0,
      },
      {
        label: "Length",
        bytes: hexOf(bytes.slice(4, 6)),
        meaning: `${length} bytes follow`,
        bad: length !== bytes.length - 6,
      },
      {
        label: "Unit ID",
        bytes: hex(bytes[6], 2),
        meaning: String(bytes[6]),
      },
    );
    if (length !== bytes.length - 6)
      error = `The header says ${length} bytes follow; ${bytes.length - 6} do.`;
    body = bytes.slice(7);
  } else if (framing === "rtu") {
    if (bytes.length < 4)
      return {
        fields: [],
        summary: "Too short for Modbus RTU",
        error: "A Modbus RTU frame has at least 4 bytes.",
      };
    head.push({
      label: "Unit ID",
      bytes: hex(bytes[0], 2),
      meaning: bytes[0] === 0 ? "0 = broadcast" : String(bytes[0]),
    });
    const n = bytes.length;
    const sent = bytes[n - 2] | (bytes[n - 1] << 8);
    const crc = crc16(bytes.slice(0, n - 2));
    tail.push({
      label: "CRC",
      bytes: hexOf(bytes.slice(n - 2)),
      meaning:
        sent === crc
          ? "OK"
          : `wrong: expected ${hexOf([crc & 0xff, crc >> 8])}`,
      bad: sent !== crc,
    });
    if (sent !== crc) error = "The CRC does not match: the frame is damaged.";
    body = bytes.slice(1, n - 2);
  } else {
    const text = String.fromCharCode(...bytes).trim();
    const raw = text.startsWith(":") ? parseHex(text.slice(1)) : null;
    if (!raw || raw.length < 3)
      return {
        fields: [],
        summary: "Not a Modbus ASCII frame",
        error: "A Modbus ASCII frame is ':' and hex digits, then CR LF.",
      };
    head.push({
      label: "Unit ID",
      bytes: hex(raw[0], 2),
      meaning: String(raw[0]),
    });
    const sent = raw[raw.length - 1];
    const sum = lrc(raw.slice(0, -1));
    tail.push({
      label: "LRC",
      bytes: hex(sent, 2),
      meaning: sent === sum ? "OK" : `wrong: expected ${hex(sum, 2)}`,
      bad: sent !== sum,
    });
    if (sent !== sum) error = "The LRC does not match: the frame is damaged.";
    body = raw.slice(1, -1);
  }

  const inner = pdu(body, direction);
  return {
    fields: [...head, ...inner.fields, ...tail],
    summary: inner.summary,
    error: error ?? inner.error,
  };
}
