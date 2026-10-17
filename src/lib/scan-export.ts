// CSV files of scan results: a few lines that say which scan ran and with
// which settings, a blank line, then one row per result. Kept apart from
// the log and the recordings, which have their own formats.

import { csvRow, stamp, type Separator } from "./csv";
import { AREA_NAME } from "./format";
import type {
  AddressRange,
  AddressScan,
  NetHit,
  NetScan,
  ScanHit,
  SerialHit,
  SerialScan,
} from "./types";

/** How far a scan got: all of it, or up to where it was stopped. */
export interface Reach {
  /** The last item asked (a unit ID or an address), or null when done. */
  stoppedAt: number | null;
}

function file(
  kind: string,
  settings: [string, string | number][],
  header: string[],
  rows: (string | number)[][],
  sep: Separator,
  now: number,
): string {
  const lines = [
    csvRow(["scan", kind], sep),
    csvRow(["date", stamp(now)], sep),
    ...settings.map((s) => csvRow(s, sep)),
    "",
    csvRow(header, sep),
    ...rows.map((r) => csvRow(r, sep)),
  ];
  return lines.join("\n") + "\n";
}

/** Unit ID scan: one row per unit ID, answered, silent or not asked. */
export function unitScanCsv(
  hits: ScanHit[],
  scan: { from: number; to: number; timeout_ms: number; connection: string },
  reach: Reach,
  sep: Separator,
  now = Date.now(),
): string {
  const byUnit = new Map(hits.map((h) => [h.unit, h.response]));
  const rows: (string | number)[][] = [];
  for (let unit = scan.from; unit <= scan.to; unit++) {
    const answer = byUnit.get(unit);
    const asked = reach.stoppedAt === null || unit <= reach.stoppedAt;
    rows.push([
      unit,
      answer !== undefined
        ? "answered"
        : asked
          ? "no answer"
          : "not asked (scan stopped)",
      answer ?? "",
    ]);
  }
  return file(
    "unit IDs",
    [
      ["connection", scan.connection],
      ["unit IDs", `${scan.from}-${scan.to}`],
      ["wait_ms", scan.timeout_ms],
      ["request", "read 1 holding register at address 0 (FC03)"],
      ["found", hits.length],
    ],
    ["unit_id", "status", "response"],
    rows,
    sep,
    now,
  );
}

/** Serial settings scan: one row per setting and unit ID that answered. */
export function serialScanCsv(
  hits: SerialHit[],
  scan: SerialScan,
  reach: { stopped: boolean },
  sep: Separator,
  now = Date.now(),
): string {
  return file(
    "serial settings",
    [
      ["port", scan.port],
      ["baud rates", scan.baudrates.join(" ")],
      ["parities", scan.parities.join(" ")],
      ["stop bits", scan.stop_bits.join(" ")],
      ["data bits", scan.data_bits],
      ["unit IDs", `${scan.from}-${scan.to}`],
      [
        "request",
        `read 1 ${AREA_NAME[scan.area].toLowerCase()} at address ${scan.address}`,
      ],
      ["wait_ms", scan.timeout_ms],
      ["result", reach.stopped ? "stopped before the end" : "complete"],
      ["found", hits.length],
    ],
    ["baud_rate", "parity", "data_bits", "stop_bits", "unit_id", "response"],
    hits.length
      ? hits.map((h) => [
          h.baudrate,
          h.parity,
          h.data_bits,
          h.stop_bits,
          h.unit,
          h.response,
        ])
      : [["", "", "", "", "", "no setting answered"]],
    sep,
    now,
  );
}

/** Address scan: one row per range that answered. */
export function addressScanCsv(
  ranges: AddressRange[],
  scan: AddressScan & { connection: string; unit: number },
  reach: Reach,
  sep: Separator,
  now = Date.now(),
): string {
  const found = ranges.reduce((n, r) => n + r.count, 0);
  return file(
    "addresses",
    [
      ["connection", scan.connection],
      ["unit ID", scan.unit],
      ["area", AREA_NAME[scan.area].toLowerCase()],
      ["addresses", `${scan.from}-${scan.to}`],
      ["block", scan.block],
      ["wait_ms", scan.timeout_ms],
      [
        "result",
        reach.stoppedAt === null
          ? "complete"
          : `stopped at address ${reach.stoppedAt}`,
      ],
      ["addresses that answered", found],
    ],
    ["first", "last", "count"],
    ranges.length
      ? ranges.map((r) => [r.start, r.start + r.count - 1, r.count])
      : [["", "", 0]],
    sep,
    now,
  );
}

/** Network scan: one row per address that opened the port. */
export function networkScanCsv(
  hits: NetHit[],
  scan: NetScan,
  progress: { done: number; total: number } | null,
  sep: Separator,
  now = Date.now(),
): string {
  const done = progress?.done ?? 0;
  const total = progress?.total ?? 0;
  return file(
    "network",
    [
      ["addresses", scan.targets],
      ["port", scan.port],
      ["send from", scan.local_address ?? "automatic"],
      ["unit ID", scan.unit],
      ["wait_ms", scan.timeout_ms],
      ["retries", scan.retries],
      ["identify", scan.identify ? "yes" : "no"],
      ["scanned", `${done} of ${total}`],
      ["modbus devices", hits.filter((h) => h.status === "modbus").length],
      [
        "port open, no Modbus answer",
        hits.filter((h) => h.status !== "modbus").length,
      ],
      ["no connection", Math.max(0, done - hits.length)],
    ],
    ["ip", "port", "unit_id", "status", "response_ms", "response", "identity"],
    hits.map((h) => [
      h.ip,
      h.port,
      h.unit,
      h.status === "modbus" ? "modbus" : "port open",
      h.response_ms.toFixed(1),
      h.response,
      h.identity ?? "",
    ]),
    sep,
    now,
  );
}
