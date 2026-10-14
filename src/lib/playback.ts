// Reads a CSV recording made with Record… on the Device map tab back into
// values over time, for the trend player.

import type { Sample } from "./trend";

export interface Column {
  name: string;
  unit: string;
  samples: Sample[];
}

export interface Recording {
  columns: Column[];
  /** Time of the first and last row, in ms since epoch. */
  start: number;
  end: number;
  rows: number;
}

/** Splits CSV text into rows of fields, honouring quotes. */
export function parseCsv(text: string, sep: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        field += '"';
        i++;
      } else if (c === '"') quoted = false;
      else field += c;
    } else if (c === '"') quoted = true;
    else if (c === sep) {
      row.push(field);
      field = "";
    } else if (c === "\n" || c === "\r") {
      if (c === "\r" && text[i + 1] === "\n") i++;
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
    } else field += c;
  }
  if (field || row.length) {
    row.push(field);
    rows.push(row);
  }
  return rows.filter((r) => r.some((f) => f !== ""));
}

/** "2026-10-14 09:30:05.120" (local time) in ms, or NaN. */
export function parseStamp(text: string): number {
  const m =
    /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?$/.exec(
      text.trim(),
    );
  if (!m) return NaN;
  const [, y, mo, d, h, mi, s, ms = "0"] = m;
  return new Date(+y, +mo - 1, +d, +h, +mi, +s, +ms.padEnd(3, "0")).getTime();
}

/** A recorded cell as a number: ON/OFF are 1/0; labels and blanks are gaps. */
export function cellNumber(text: string): number {
  const t = text.trim();
  if (t === "ON") return 1;
  if (t === "OFF") return 0;
  if (t === "") return NaN;
  const n = Number(t.replace(",", "."));
  return Number.isFinite(n) ? n : NaN;
}

/** Reads a recording, or explains why the file is not one. */
export function parseRecording(text: string): Recording | { error: string } {
  const first = text.split(/\r?\n/, 1)[0] ?? "";
  const sep = first.startsWith("time;") ? ";" : ",";
  const rows = parseCsv(text, sep);
  const header = rows[0];
  if (!header || header[0] !== "time" || header.length < 2)
    return {
      error:
        'This is not a Modbench recording: its first column must be "time".',
    };
  const columns: Column[] = header.slice(1).map((h) => {
    const m = /^(.*) \(([^()]*)\)$/.exec(h);
    return m
      ? { name: m[1], unit: m[2], samples: [] }
      : { name: h, unit: "", samples: [] };
  });
  let start = Infinity;
  let end = -Infinity;
  let count = 0;
  for (const row of rows.slice(1)) {
    const t = parseStamp(row[0] ?? "");
    if (!Number.isFinite(t)) continue;
    count++;
    start = Math.min(start, t);
    end = Math.max(end, t);
    columns.forEach((c, i) =>
      c.samples.push({ t, v: cellNumber(row[i + 1] ?? "") }),
    );
  }
  if (!count) return { error: "The recording has no rows." };
  for (const c of columns) c.samples.sort((a, b) => a.t - b.t);
  return { columns, start, end, rows: count };
}

/** The samples up to time `t` (samples are in time order). */
export function upTo(samples: Sample[], t: number): Sample[] {
  let lo = 0;
  let hi = samples.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (samples[mid].t <= t) lo = mid + 1;
    else hi = mid;
  }
  return lo === samples.length ? samples : samples.slice(0, lo);
}
