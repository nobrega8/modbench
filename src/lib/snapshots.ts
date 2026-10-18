// Snapshots of the values shown, to compare with the live values or with
// another snapshot: "what changes when I press this button?", "what is
// different between these two units?".

import { formatRaw, formatValue, hex } from "./format";
import { registerId } from "./profiles";
import type { Area, PointResult, Register } from "./types";
import { t } from "./i18n.svelte.ts";

/** Saved snapshot files end in this. */
export const SNAPSHOT_EXTENSION = "mbsnap.json";

/** One value as it was shown: text to compare, and the raw words. */
export interface SnapValue {
  name: string;
  text: string;
  raw: string;
}

export interface Snapshot {
  kind: "modbench-snapshot";
  v: 1;
  /** When it was taken, in ms since epoch. */
  taken_ms: number;
  /** Free label, e.g. the file name or "Unit 3". */
  label: string;
  /** A device map's registers, by register id… */
  map?: { vendor: string; model: string; values: Record<string, SnapValue> };
  /** …or the words of a manual read, from `address`. */
  read?: { area: Area; address: number; words: number[] };
}

/** What a register shows, as compared: its value, sentinel or failure. */
export function shownText(reg: Register, r: PointResult | undefined): string {
  if (!r) return "";
  if (r.error) return "read failed";
  if (r.sentinel) return r.sentinel;
  return formatValue(reg, r.value);
}

/** The values of a map's registers now (only those that were read). */
export function snapMap(
  meta: { vendor: string; model: string },
  registers: Register[],
  results: Map<number, PointResult>,
  label: string,
  now = Date.now(),
): Snapshot {
  const values: Record<string, SnapValue> = {};
  registers.forEach((reg, i) => {
    const r = results.get(i);
    if (!r) return;
    values[registerId(reg)] = {
      name: reg.name,
      text: shownText(reg, r),
      raw: formatRaw(r.raw ?? null),
    };
  });
  return {
    kind: "modbench-snapshot",
    v: 1,
    taken_ms: now,
    label,
    map: { vendor: meta.vendor, model: meta.model, values },
  };
}

/** The words of a manual read now. */
export function snapRead(
  area: Area,
  address: number,
  words: number[],
  label: string,
  now = Date.now(),
): Snapshot {
  return {
    kind: "modbench-snapshot",
    v: 1,
    taken_ms: now,
    label,
    read: { area, address, words: [...words] },
  };
}

/** The live values of a map, as a snapshot would hold them. */
export function liveMap(
  registers: Register[],
  results: Map<number, PointResult>,
): Map<string, string> {
  const out = new Map<string, string>();
  registers.forEach((reg, i) => {
    const r = results.get(i);
    if (r) out.set(registerId(reg), shownText(reg, r));
  });
  return out;
}

/** A register's value in a snapshot, or undefined when it was not read. */
export function snapText(s: Snapshot, reg: Register): string | undefined {
  return s.map?.values[registerId(reg)]?.text;
}

/**
 * Whether a register differs between a snapshot and what it is compared
 * with. Registers missing on either side do not count as different.
 */
export function differs(before: string | undefined, after: string | undefined) {
  return before !== undefined && after !== undefined && before !== after;
}

/** The word at an address in a read snapshot, or undefined. */
export function snapWord(s: Snapshot, area: Area, address: number) {
  const r = s.read;
  if (!r || r.area !== area) return undefined;
  return r.words[address - r.address];
}

/** A word as shown in the hint of a cell: decimal and hex. */
export function wordText(w: number): string {
  return `${w} (0x${hex(w)})`;
}

export function snapshotFile(s: Snapshot): string {
  return JSON.stringify(s, null, 2) + "\n";
}

/** Reads a snapshot file, or says why it is not one. */
export function parseSnapshot(text: string): Snapshot | { error: string } {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return { error: t("snap.notjson") };
  }
  const s = data as Partial<Snapshot>;
  if (s?.kind !== "modbench-snapshot" || s.v !== 1)
    return { error: t("snap.notsnap") };
  if (!s.map && !s.read) return { error: t("snap.empty") };
  return {
    kind: "modbench-snapshot",
    v: 1,
    taken_ms: Number(s.taken_ms) || 0,
    label: String(s.label ?? ""),
    map: s.map,
    read: s.read,
  };
}
