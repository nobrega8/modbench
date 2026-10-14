// Which device is connected, from the map's identity registers, and whether
// the map fits it (section 9 of the .mbmap spec). Firmware versions are whole
// numbers separated by dots, compared as numbers from the left, and a bound
// covers every version that starts with it ("2" covers 2.9.1).

import { formatValue } from "./format";
import type { DeviceMap, PointResult, Register } from "./types";

export type Version = number[];
export type CheckStatus = "match" | "mismatch" | "unknown";

export interface Identity {
  model: { status: CheckStatus; found: string | null };
  firmware: {
    status: CheckStatus;
    /** The device's version, when its firmware registers gave one. */
    version: Version | null;
  };
  serial: string | null;
}

/** A version string written in a map: "2", "2.1", "1.02.7". */
export function parseVersion(text: string | null | undefined): Version | null {
  if (!text || !/^[0-9]+(\.[0-9]+)*$/.test(text)) return null;
  return text.split(".").map(Number);
}

/** The first version string in a device's text: "V2.01b" -> 2.01. */
export function findVersion(text: string): Version | null {
  const m = /[0-9]+(\.[0-9]+)*/.exec(text);
  return m ? parseVersion(m[0]) : null;
}

/** Compares a version with a bound on the components the bound gives. */
function compareToBound(v: Version, bound: Version): number {
  for (let i = 0; i < bound.length; i++) {
    const d = (v[i] ?? 0) - bound[i];
    if (d !== 0) return d;
  }
  return 0;
}

/** Whether a version is within firmware_min ... firmware_max (either open). */
export function inRange(
  v: Version,
  min: string | null | undefined,
  max: string | null | undefined,
): boolean {
  const lo = parseVersion(min);
  const hi = parseVersion(max);
  return (
    (!lo || compareToBound(v, lo) >= 0) && (!hi || compareToBound(v, hi) <= 0)
  );
}

/** "2.1 and later", "up to 2.0", "1 to 2", or null when open. */
export function rangeText(
  min: string | null | undefined,
  max: string | null | undefined,
): string | null {
  if (min && max) return min === max ? min : `${min} to ${max}`;
  if (min) return `${min} and later`;
  if (max) return `up to ${max}`;
  return null;
}

export function versionText(v: Version): string {
  return v.join(".");
}

/** Whether a register exists on a device with this firmware (always, when
 * the firmware is not known). */
export function appliesTo(reg: Register, version: Version | null): boolean {
  return !version || inRange(version, reg.firmware_min, reg.firmware_max);
}

/** Text of a register's reading, with its enum label when it has one. */
function readingText(reg: Register, r: PointResult | undefined): string | null {
  if (!r || r.error || r.sentinel || r.value === null) return null;
  const text = formatValue(reg, r.value).trim();
  return text === "—" || text === "" ? null : text;
}

/** Checks the device behind `results` against the map. */
export function identify(
  map: DeviceMap,
  results: Map<number, PointResult>,
): Identity {
  const m = map.meta;
  const regs = map.registers;
  const indexOf = (role: string) => regs.findIndex((r) => r.role === role);

  const modelIndex = indexOf("model");
  const modelFound =
    modelIndex < 0
      ? null
      : readingText(regs[modelIndex], results.get(modelIndex));
  const names = (m.models?.length ? m.models : [m.model]).map((n) =>
    n.trim().toLowerCase(),
  );
  const model = {
    found: modelFound,
    status: (modelFound === null
      ? "unknown"
      : names.includes(modelFound.toLowerCase())
        ? "match"
        : "mismatch") as CheckStatus,
  };

  // Several firmware registers are joined with dots, in map order.
  let version: Version | null = null;
  const parts = regs
    .map((reg, i) => (reg.role === "firmware" ? i : -1))
    .filter((i) => i >= 0);
  if (parts.length) {
    const found = parts.map((i) => {
      const text = readingText(regs[i], results.get(i));
      return text === null ? null : findVersion(text);
    });
    if (found.every((v) => v !== null)) version = found.flat() as Version;
  }
  const declared = !!(m.firmware_min || m.firmware_max);
  const firmware = {
    version,
    status: (!declared || !version
      ? "unknown"
      : inRange(version, m.firmware_min, m.firmware_max)
        ? "match"
        : "mismatch") as CheckStatus,
  };

  const serialIndex = indexOf("serial");
  const serial =
    serialIndex < 0
      ? null
      : readingText(regs[serialIndex], results.get(serialIndex));

  return { model, firmware, serial };
}
