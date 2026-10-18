// Simulator setups: how the simulator listens and its devices (unit ID, map,
// generators), saved to a file so a test bench does not have to be set up
// again by hand. The map is kept in the file too, so a setup still opens
// when the map file has moved.

import type { DeviceMap, Generator, Parity, Transport } from "./types";
import { t } from "./i18n.svelte.ts";

export const SETUP_EXTENSION = "mbsim";
const VERSION = 1;

export interface SetupListen {
  kind: Transport["kind"];
  port: number;
  /** Listen on every network interface, not only this computer. */
  public: boolean;
  serial_port: string;
  baudrate: number;
  parity: Parity;
  data_bits: number;
  stop_bits: number;
}

export interface SetupDevice {
  unit: number | null;
  strict: boolean;
  /** The map file it was shaped after; null when none or not from a file. */
  source: string | null;
  map: DeviceMap | null;
  generators: Generator[];
}

export interface SimSetup {
  modbench_sim_setup: number;
  listen: SetupListen;
  devices: SetupDevice[];
}

export function setupText(listen: SetupListen, devices: SetupDevice[]): string {
  const setup: SimSetup = { modbench_sim_setup: VERSION, listen, devices };
  return JSON.stringify(setup, null, 2) + "\n";
}

const KINDS = ["tcp", "rtuovertcp", "udp", "rtuoverudp", "rtu", "ascii"];
const PARITIES = ["N", "E", "O"];

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);
const isInt = (v: unknown, min: number, max: number): v is number =>
  Number.isInteger(v) && (v as number) >= min && (v as number) <= max;

/** Reads a setup file; says what is wrong with it otherwise. */
export function parseSetup(text: string): SimSetup | { error: string } {
  let data: unknown;
  try {
    data = JSON.parse(text);
  } catch {
    return { error: t("setup.notjson") };
  }
  if (!isObject(data) || typeof data.modbench_sim_setup !== "number")
    return { error: t("setup.notsetup") };
  if (data.modbench_sim_setup > VERSION)
    return {
      error: t("setup.newer"),
    };

  const l = data.listen;
  if (
    !isObject(l) ||
    !KINDS.includes(l.kind as string) ||
    !isInt(l.port, 0, 65535) ||
    typeof l.public !== "boolean" ||
    typeof l.serial_port !== "string" ||
    !isInt(l.baudrate, 1, 10_000_000) ||
    !PARITIES.includes(l.parity as string) ||
    !isInt(l.data_bits, 5, 8) ||
    !isInt(l.stop_bits, 1, 2)
  )
    return { error: t("setup.listen") };

  const devices = data.devices;
  if (!Array.isArray(devices) || !devices.length)
    return { error: t("setup.nodevices") };
  for (const [i, d] of devices.entries()) {
    const ok =
      isObject(d) &&
      (d.unit === null || isInt(d.unit, 0, 255)) &&
      typeof d.strict === "boolean" &&
      (d.source === null || typeof d.source === "string") &&
      (d.map === null || (isObject(d.map) && Array.isArray(d.map.registers))) &&
      Array.isArray(d.generators) &&
      d.generators.every(isObject);
    if (!ok) return { error: t("setup.baddevice", { n: i + 1 }) };
  }
  // As the simulator checks it: one device per unit ID, one for all.
  const units = devices.map((d) => d.unit as number | null);
  if (units.filter((u) => u === null).length > 1)
    return { error: t("setup.oneall") };
  const twice = units.find((u, i) => u !== null && units.indexOf(u) !== i);
  if (twice !== undefined)
    return { error: t("setup.twice", { unit: twice ?? "" }) };
  return data as unknown as SimSetup;
}
