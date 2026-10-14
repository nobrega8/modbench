// Alarm limits on map registers: a value below Low or above High (or a bit
// in its alarm state) is shown in red. Kept per map file, on this machine.

import { registerId, type KeyValueStore } from "./profiles";
import type { Register, Value } from "./types";

export interface Alarm {
  /** The register, by registerId(). */
  id: string;
  /** Limits in engineering units; null leaves that side open. */
  low: number | null;
  high: number | null;
  /** For bits: the state that is an alarm. */
  when: boolean | null;
}

export type AlarmState = "low" | "high" | "bit" | null;

const PREFIX = "modbench.alarms:";

function defaultStore(): KeyValueStore | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

/** Whether a value is in alarm, and on which side. */
export function check(alarm: Alarm, value: Value | null): AlarmState {
  if (value === null || value === undefined) return null;
  if (typeof value === "boolean")
    return alarm.when !== null && value === alarm.when ? "bit" : null;
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  if (alarm.low !== null && value < alarm.low) return "low";
  if (alarm.high !== null && value > alarm.high) return "high";
  return null;
}

/** The limits as short text, e.g. "< 10 or > 250 V". */
export function limitsText(alarm: Alarm, unit = ""): string {
  const u = unit ? ` ${unit}` : "";
  if (alarm.when !== null) return alarm.when ? "when ON" : "when OFF";
  const parts = [];
  if (alarm.low !== null) parts.push(`< ${alarm.low}`);
  if (alarm.high !== null) parts.push(`> ${alarm.high}`);
  return parts.length ? parts.join(" or ") + u : "no limits";
}

/** Whether an alarm has anything to check. */
export function isSet(alarm: Alarm): boolean {
  return alarm.low !== null || alarm.high !== null || alarm.when !== null;
}

export function loadAlarms(mapPath: string, store = defaultStore()): Alarm[] {
  try {
    const raw = store?.getItem(PREFIX + mapPath);
    const list = raw ? (JSON.parse(raw) as Alarm[]) : [];
    return Array.isArray(list)
      ? list.filter((a) => typeof a.id === "string")
      : [];
  } catch {
    return [];
  }
}

export function saveAlarms(
  mapPath: string,
  alarms: Alarm[],
  store = defaultStore(),
) {
  try {
    const kept = alarms.filter(isSet);
    if (kept.length) store?.setItem(PREFIX + mapPath, JSON.stringify(kept));
    else store?.removeItem(PREFIX + mapPath);
  } catch {
    // Storage unavailable: alarms last for this session only.
  }
}

/** The alarms of a map's registers, by register index. */
export function byIndex(
  alarms: Alarm[],
  registers: Register[],
): Map<number, Alarm> {
  const ids = new Map(alarms.map((a) => [a.id, a]));
  const out = new Map<number, Alarm>();
  registers.forEach((r, i) => {
    const a = ids.get(registerId(r));
    if (a) out.set(i, a);
  });
  return out;
}
