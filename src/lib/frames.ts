// Field-by-field reading of Modbus frames, for the Raw data tab and the
// frame decoder: MBAP header (TCP), unit ID and CRC (RTU) or LRC (ASCII),
// then the PDU.

import { FUNCTION_NAME, hex } from "./format";
import { td, t } from "./i18n.svelte.ts";

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

const EXCEPTION_CODES = [1, 2, 3, 4, 5, 6, 8, 10, 11];

/** What an exception code means, in the current language. */
export function exceptionName(code: number): string {
  return EXCEPTION_CODES.includes(code)
    ? td(`frame.exc.${code}`)
    : t("frame.exc.unknown");
}

export function functionName(fc: number): string {
  if (FUNCTION_NAME[fc]) return FUNCTION_NAME[fc];
  return fc === 43
    ? t("frame.fc43")
    : [7, 8, 11, 12, 17, 20, 21, 22, 23, 24].includes(fc)
      ? td(`fc.${fc}`)
      : t("frame.function_n", { fc });
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
      error ??= t("frame.ends_before", { label });
      at = b.length;
      return null;
    }
    const part = b.slice(at, at + n);
    at += n;
    fields.push({ label, bytes: hexOf(part), meaning: meaning(part) });
    return part;
  };
  const fcByte = take(1, t("frame.function"), (x) =>
    x[0] & 0x80
      ? t("frame.exception_of", { n: x[0], name: functionName(x[0] & 0x7f) })
      : `${x[0]} = ${functionName(x[0])}`,
  );
  if (!fcByte) return { fields, summary: t("frame.empty"), error };
  const fc = fcByte[0] & 0x7f;
  let summary = functionName(fc);

  if (fcByte[0] & 0x80) {
    const code = take(
      1,
      t("frame.exception"),
      (x) => `${x[0]} = ${exceptionName(x[0])}`,
    );
    summary = t("frame.exception_sum", {
      n: code?.[0] ?? "?",
      name: summary,
    });
  } else if (direction === "request" && fc >= 1 && fc <= 6) {
    const a = take(2, t("frame.address"), (x) => String(word(x, 0)));
    const v = take(2, fc <= 4 ? t("frame.quantity") : t("frame.value"), (x) =>
      fc === 5
        ? word(x, 0) === 0xff00
          ? t("frame.on")
          : word(x, 0) === 0
            ? t("frame.off")
            : t("frame.bad_coil", { n: word(x, 0) })
        : String(word(x, 0)),
    );
    if (a && v)
      summary += ` ${word(a, 0)}${fc <= 4 ? ` × ${word(v, 0)}` : ` ← ${fields.at(-1)!.meaning}`}`;
  } else if (direction === "request" && (fc === 15 || fc === 16)) {
    const a = take(2, t("frame.address"), (x) => String(word(x, 0)));
    const q = take(2, t("frame.quantity"), (x) => String(word(x, 0)));
    const n = take(1, t("frame.byte_count"), (x) => String(x[0]));
    if (n) dataFields(fc === 15 ? "bits" : "words", n[0]);
    if (a && q) summary += ` ${word(a, 0)} × ${word(q, 0)}`;
  } else if (direction === "response" && fc >= 1 && fc <= 4) {
    const n = take(1, t("frame.byte_count"), (x) => String(x[0]));
    if (n) {
      dataFields(fc <= 2 ? "bits" : "words", n[0]);
      summary +=
        fc <= 2
          ? t("frame.sum_bits", { count: n[0] })
          : `: ${td("count.registers", { count: n[0] / 2 })}`;
    }
  } else if (direction === "response" && (fc === 5 || fc === 6)) {
    take(2, t("frame.address"), (x) => String(word(x, 0)));
    take(2, t("frame.value"), (x) =>
      fc === 5
        ? word(x, 0)
          ? t("frame.on")
          : t("frame.off")
        : String(word(x, 0)),
    );
    summary += ` ${t("frame.echo")}`;
  } else if (direction === "response" && (fc === 15 || fc === 16)) {
    const a = take(2, t("frame.address"), (x) => String(word(x, 0)));
    const q = take(2, t("frame.quantity"), (x) => String(word(x, 0)));
    if (a && q) summary += ` ${word(a, 0)} × ${word(q, 0)} ${t("frame.done")}`;
  } else if (fc === 22) {
    // Same fields both ways: the answer echoes the request
    const a = take(2, t("frame.address"), (x) => String(word(x, 0)));
    take(2, t("frame.and_mask"), (x) => `0x${hex(word(x, 0))}`);
    take(2, t("frame.or_mask"), (x) => `0x${hex(word(x, 0))}`);
    if (a)
      summary += ` ${word(a, 0)}${direction === "response" ? ` ${t("frame.echo")}` : ""}`;
  } else if (fc === 23 && direction === "request") {
    const ra = take(2, t("frame.read_address"), (x) => String(word(x, 0)));
    const rn = take(2, t("frame.read_quantity"), (x) => String(word(x, 0)));
    const wa = take(2, t("frame.write_address"), (x) => String(word(x, 0)));
    const wn = take(2, t("frame.write_quantity"), (x) => String(word(x, 0)));
    const n = take(1, t("frame.byte_count"), (x) => String(x[0]));
    if (n) dataFields("words", n[0]);
    if (ra && rn && wa && wn)
      summary += t("frame.sum_rw", {
        ra: word(ra, 0),
        rn: word(rn, 0),
        wa: word(wa, 0),
        wn: word(wn, 0),
      });
  } else if (fc === 23) {
    const n = take(1, t("frame.byte_count"), (x) => String(x[0]));
    if (n) {
      dataFields("words", n[0]);
      summary += `: ${td("count.registers", { count: n[0] / 2 })}`;
    }
  } else if (fc === 43) {
    take(1, t("frame.mei_type"), (x) =>
      x[0] === 14 ? t("frame.mei14") : String(x[0]),
    );
    take(1, t("frame.read_code"), (x) =>
      x[0] >= 1 && x[0] <= 4 ? td(`frame.code.${x[0]}`) : String(x[0]),
    );
    if (direction === "request") {
      take(1, t("frame.object_id"), (x) => String(x[0]));
    } else {
      take(1, t("frame.conformity"), (x) => `0x${hex(x[0], 2)}`);
      take(1, t("frame.more_follows"), (x) =>
        x[0] ? t("frame.yes") : t("frame.no"),
      );
      take(1, t("frame.next_object"), (x) => String(x[0]));
      const n = take(1, t("frame.objects"), (x) => String(x[0]));
      for (let k = 0; n && k < n[0] && at < b.length; k++) {
        const id = take(1, t("frame.object_id"), (x) => String(x[0]));
        const len = take(1, t("frame.length"), (x) => String(x[0]));
        if (!id || !len) break;
        take(len[0], t("frame.object_n", { id: id[0] }), (x) =>
          JSON.stringify(String.fromCharCode(...x)),
        );
      }
    }
  }
  if (at < b.length)
    fields.push({
      label: t("frame.data"),
      bytes: hexOf(b.slice(at)),
      meaning: t("frame.bytes_n", { count: b.length - at }),
    });
  return { fields, summary, error };

  function dataFields(kind: "bits" | "words", n: number) {
    const data = b.slice(at, at + n);
    if (data.length < n) error ??= t("frame.ends_data");
    at += data.length;
    if (kind === "bits") {
      const bits = data.flatMap((byte) =>
        Array.from({ length: 8 }, (_, i) => (byte >> i) & 1),
      );
      fields.push({
        label: t("frame.bits"),
        bytes: hexOf(data),
        meaning: t("frame.bits_note", { bits: bits.join("") }),
      });
    } else {
      const words = [];
      for (let i = 0; i + 1 < data.length; i += 2) words.push(word(data, i));
      fields.push({
        label: t("frame.registers"),
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
        summary: t("frame.short_tcp"),
        error: t("frame.short_tcp_why"),
      };
    const length = word(bytes, 4);
    head.push(
      {
        label: t("frame.transaction"),
        bytes: hexOf(bytes.slice(0, 2)),
        meaning: String(word(bytes, 0)),
      },
      {
        label: t("frame.protocol"),
        bytes: hexOf(bytes.slice(2, 4)),
        meaning:
          word(bytes, 2) === 0 ? t("frame.modbus") : String(word(bytes, 2)),
        bad: word(bytes, 2) !== 0,
      },
      {
        label: t("frame.length"),
        bytes: hexOf(bytes.slice(4, 6)),
        meaning: t("frame.follow", { count: length }),
        bad: length !== bytes.length - 6,
      },
      {
        label: t("frame.unit_id"),
        bytes: hex(bytes[6], 2),
        meaning: String(bytes[6]),
      },
    );
    if (length !== bytes.length - 6)
      error = t("frame.header_length", {
        said: length,
        real: bytes.length - 6,
      });
    body = bytes.slice(7);
  } else if (framing === "rtu") {
    if (bytes.length < 4)
      return {
        fields: [],
        summary: t("frame.short_rtu"),
        error: t("frame.short_rtu_why"),
      };
    head.push({
      label: t("frame.unit_id"),
      bytes: hex(bytes[0], 2),
      meaning: bytes[0] === 0 ? t("frame.broadcast") : String(bytes[0]),
    });
    const n = bytes.length;
    const sent = bytes[n - 2] | (bytes[n - 1] << 8);
    const crc = crc16(bytes.slice(0, n - 2));
    tail.push({
      label: "CRC",
      bytes: hexOf(bytes.slice(n - 2)),
      meaning:
        sent === crc
          ? t("frame.ok")
          : t("frame.wrong", { expected: hexOf([crc & 0xff, crc >> 8]) }),
      bad: sent !== crc,
    });
    if (sent !== crc) error = t("frame.crc_bad");
    body = bytes.slice(1, n - 2);
  } else {
    const text = String.fromCharCode(...bytes).trim();
    const raw = text.startsWith(":") ? parseHex(text.slice(1)) : null;
    if (!raw || raw.length < 3)
      return {
        fields: [],
        summary: t("frame.not_ascii"),
        error: t("frame.not_ascii_why"),
      };
    head.push({
      label: t("frame.unit_id"),
      bytes: hex(raw[0], 2),
      meaning: String(raw[0]),
    });
    const sent = raw[raw.length - 1];
    const sum = lrc(raw.slice(0, -1));
    tail.push({
      label: "LRC",
      bytes: hex(sent, 2),
      meaning:
        sent === sum
          ? t("frame.ok")
          : t("frame.wrong", { expected: hex(sum, 2) }),
      bad: sent !== sum,
    });
    if (sent !== sum) error = t("frame.lrc_bad");
    body = raw.slice(1, -1);
  }

  const inner = pdu(body, direction);
  return {
    fields: [...head, ...inner.fields, ...tail],
    summary: inner.summary,
    error: error ?? inner.error,
  };
}
