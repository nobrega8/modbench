// Pure parts of the bus monitor: what a heard request shows, which ones a
// filter keeps, and the list as CSV.

import { csvRow, stamp, type Separator } from "./csv";
import type { MonitorEvent } from "./types";

/** The values a request read or wrote, short: registers in decimal, bits as
 * 0 and 1. */
export function valuesText(e: MonitorEvent, max = 8): string {
  if (e.registers) {
    const shown = e.registers.slice(0, max).join(" ");
    return e.registers.length > max ? `${shown} …` : shown;
  }
  if (e.bits) {
    const shown = e.bits
      .slice(0, max * 4)
      .map((b) => (b ? "1" : "0"))
      .join("");
    return e.bits.length > max * 4 ? `${shown}…` : shown;
  }
  return "";
}

/** Address and count, like `100 ×2`. */
export function whereText(e: MonitorEvent): string {
  if (e.address === null) return "";
  return e.count !== null && e.count > 1
    ? `${e.address} ×${e.count}`
    : String(e.address);
}

export interface MonitorFilter {
  /** Only this unit ID; null for all. */
  unit: number | null;
  /** Only this function code; null for all. */
  fn: number | null;
  /** Only requests that went wrong (exception, no answer, bad frame). */
  problems: boolean;
}

export function keeps(f: MonitorFilter, e: MonitorEvent): boolean {
  if (f.unit !== null && e.unit !== f.unit) return false;
  if (f.fn !== null && e.function !== f.fn) return false;
  if (f.problems && (e.outcome === "ok" || e.outcome === "broadcast"))
    return false;
  return true;
}

/** The requests heard, oldest first, as CSV. */
export function monitorCsv(events: MonitorEvent[], sep: Separator): string {
  const rows = [
    csvRow(
      [
        "time",
        "unit",
        "function",
        "address",
        "count",
        "outcome",
        "exception",
        "ms",
        "values",
        "request",
        "response",
      ],
      sep,
    ),
  ];
  for (const e of events)
    rows.push(
      csvRow(
        [
          stamp(e.time_ms),
          e.unit ?? "",
          e.function ?? "",
          e.address ?? "",
          e.count ?? "",
          e.outcome,
          e.exception ?? "",
          e.ms ?? "",
          valuesText(e, Infinity),
          e.request,
          e.response ?? "",
        ],
        sep,
      ),
    );
  return rows.join("\r\n") + "\r\n";
}
