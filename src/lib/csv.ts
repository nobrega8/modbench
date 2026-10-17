// CSV text for the log, map values and recordings.

import { FUNCTION_NAME, formatRaw, formatValue } from "./format";
import type { PointResult, Register, TrafficEntry } from "./types";

export type Separator = "," | ";";

/** One CSV line. Fields with the separator, quotes or line breaks are quoted. */
export function csvRow(fields: (string | number)[], sep: Separator): string {
  return fields
    .map((f) => {
      const s = String(f);
      return s.includes(sep) || /["\r\n]/.test(s)
        ? `"${s.replaceAll('"', '""')}"`
        : s;
    })
    .join(sep);
}

/** Local date and time with milliseconds, e.g. 2026-10-14 09:30:05.120. */
export function stamp(ms: number): string {
  const d = new Date(ms);
  const p = (n: number, w = 2) => n.toString().padStart(w, "0");
  return (
    `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ` +
    `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`
  );
}

export function logCsv(entries: TrafficEntry[], sep: Separator): string {
  const lines = [
    csvRow(
      ["time", "function", "name", "address", "count", "duration_ms", "result"],
      sep,
    ),
    ...entries.map((t) =>
      csvRow(
        t.note
          ? [stamp(t.time_ms), "", "note", "", "", "", t.note]
          : [
              stamp(t.time_ms),
              t.function,
              FUNCTION_NAME[t.function] ?? "",
              t.address,
              t.count,
              t.duration_ms.toFixed(1),
              t.error ?? "OK",
            ],
        sep,
      ),
    ),
  ];
  return lines.join("\n") + "\n";
}

/** What a register shows right now, as CSV text (empty when not read). */
export function cellText(reg: Register, r: PointResult | undefined): string {
  if (!r || r.error) return "";
  if (r.sentinel) return r.sentinel;
  return formatValue(reg, r.value);
}

/** The current values of the given map registers, one per line. */
export function valuesCsv(
  registers: Register[],
  indexes: number[],
  results: Map<number, PointResult>,
  sep: Separator,
): string {
  const lines = [
    csvRow(
      ["name", "group", "area", "address", "value", "unit", "raw", "error"],
      sep,
    ),
    ...indexes.map((i) => {
      const reg = registers[i];
      const r = results.get(i);
      return csvRow(
        [
          reg.name,
          reg.group,
          reg.area,
          reg.address,
          cellText(reg, r),
          reg.unit,
          formatRaw(r?.raw ?? null),
          r?.error ?? "",
        ],
        sep,
      );
    }),
  ];
  return lines.join("\n") + "\n";
}

/** Header of a recording: time, then one column per register. */
export function recordHeader(registers: Register[], sep: Separator): string {
  return (
    csvRow(
      [
        "time",
        ...registers.map((r) => (r.unit ? `${r.name} (${r.unit})` : r.name)),
      ],
      sep,
    ) + "\n"
  );
}

/**
 * A recorded cell: the value; `#failed` when the read failed, so a failure
 * is never mistaken for a value or for a register that was not read; the
 * sentinel label for a reserved value.
 */
export function recordCell(reg: Register, r: PointResult | undefined): string {
  if (r?.error) return FAILED_CELL;
  return cellText(reg, r);
}

/** What a recording holds for a failed read. Older readers see a gap. */
export const FAILED_CELL = "#failed";

/** One recorded cycle, in the column order of `indexes`. */
export function recordRow(
  time_ms: number,
  registers: Register[],
  indexes: number[],
  results: Map<number, PointResult>,
  sep: Separator,
): string {
  return (
    csvRow(
      [
        stamp(time_ms),
        ...indexes.map((i) => recordCell(registers[i], results.get(i))),
      ],
      sep,
    ) + "\n"
  );
}
