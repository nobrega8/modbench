// Remembered register choices per device.
//
// A device profile is what the user chose for ONE device (a map, reached over
// one connection endpoint, with one unit ID): which registers are read, the
// byte order override and Loop. Profiles only describe the screen; restoring
// one never sends anything to a device.
//
// Registers are remembered by an id made of what the map says about them
// (area, address, type, bit, name), not by their position, so editing a map
// file does not switch the wrong registers off.

import type { ByteOrder, Register } from "./types";

/** Where choices are kept: the webview's localStorage, or a stand-in in tests. */
export type KeyValueStore = Pick<
  Storage,
  "getItem" | "setItem" | "removeItem" | "length" | "key"
>;

export interface DeviceProfile {
  v: 1;
  /** Ids of the registers switched off. */
  skipped: string[];
  /** Byte order forced for every value, or null to use the map's. */
  order: ByteOrder | null;
  loop: boolean;
}

const PROFILE_PREFIX = "modbench.device:";
/** The registers last used with a map, whichever device: seeds new profiles. */
const MAP_PREFIX = "modbench.map:";
/** Older versions stored switched-off register indexes per map file. */
const LEGACY_PREFIX = "modbench.skipped:";

function defaultStore(): KeyValueStore | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export function registerId(r: Register): string {
  return [r.area, r.address, r.type, r.bit ?? "", r.bits ?? "", r.name].join(
    ":",
  );
}

/** Where a device is reached, without the unit ID. */
export function endpointOf(c: {
  transport: "tcp" | "rtuovertcp" | "udp" | "rtuoverudp" | "rtu";
  framing?: "rtu" | "ascii";
  host: string;
  tcpPort: number;
  serialPort: string;
}): string {
  return c.transport === "rtu"
    ? `${c.framing === "ascii" ? "ascii" : "rtu"}:${c.serialPort}`
    : `${c.transport}:${c.host.trim().toLowerCase()}:${c.tcpPort}`;
}

export function profileKey(
  mapPath: string,
  endpoint: string,
  unit: number,
): string {
  return `${PROFILE_PREFIX}${encodeURIComponent(mapPath)}|${endpoint}|${unit}`;
}

/** Indexes of the registers whose ids are listed; unknown ids are ignored. */
export function indexesOf(ids: string[], registers: Register[]): Set<number> {
  const wanted = new Set(ids);
  const out = new Set<number>();
  registers.forEach((r, i) => {
    if (wanted.has(registerId(r))) out.add(i);
  });
  return out;
}

export function idsOf(indexes: Set<number>, registers: Register[]): string[] {
  return [...indexes]
    .filter((i) => i >= 0 && i < registers.length)
    .map((i) => registerId(registers[i]));
}

function readJson<T>(store: KeyValueStore | null, key: string): T | null {
  try {
    const raw = store?.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function writeJson(store: KeyValueStore | null, key: string, value: unknown) {
  try {
    store?.setItem(key, JSON.stringify(value));
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}

export function loadProfile(
  key: string,
  store: KeyValueStore | null = defaultStore(),
): DeviceProfile | null {
  const p = readJson<Partial<DeviceProfile>>(store, key);
  if (!p || p.v !== 1 || !Array.isArray(p.skipped)) return null;
  return {
    v: 1,
    skipped: p.skipped.filter((s): s is string => typeof s === "string"),
    order:
      p.order === "ABCD" ||
      p.order === "CDAB" ||
      p.order === "BADC" ||
      p.order === "DCBA"
        ? p.order
        : null,
    loop: p.loop !== false,
  };
}

export function saveProfile(
  key: string,
  profile: DeviceProfile,
  store: KeyValueStore | null = defaultStore(),
) {
  writeJson(store, key, profile);
}

/**
 * The switched-off registers last used with a map (any device), or null when
 * nothing was saved. Reads the older per-map format once and converts it.
 */
export function loadMapDefault(
  mapPath: string,
  registers: Register[],
  store: KeyValueStore | null = defaultStore(),
): string[] | null {
  const saved = readJson<string[]>(store, MAP_PREFIX + mapPath);
  if (Array.isArray(saved)) return saved;
  const legacy = readJson<number[]>(store, LEGACY_PREFIX + mapPath);
  if (Array.isArray(legacy)) {
    const ids = idsOf(new Set(legacy), registers);
    writeJson(store, MAP_PREFIX + mapPath, ids);
    try {
      store?.removeItem(LEGACY_PREFIX + mapPath);
    } catch {
      // Left in place; it is only read when the new key is missing.
    }
    return ids;
  }
  return null;
}

export function saveMapDefault(
  mapPath: string,
  ids: string[],
  store: KeyValueStore | null = defaultStore(),
) {
  writeJson(store, MAP_PREFIX + mapPath, ids);
}

/** Deletes every remembered device choice (Settings, "Forget"). */
export function forgetAll(
  store: KeyValueStore | null = defaultStore(),
): number {
  if (!store) return 0;
  const keys: string[] = [];
  for (let i = 0; i < store.length; i++) {
    const k = store.key(i);
    if (
      k &&
      (k.startsWith(PROFILE_PREFIX) ||
        k.startsWith(MAP_PREFIX) ||
        k.startsWith(LEGACY_PREFIX))
    )
      keys.push(k);
  }
  for (const k of keys) {
    try {
      store.removeItem(k);
    } catch {
      // Nothing more to do for this key.
    }
  }
  return keys.filter((k) => k.startsWith(PROFILE_PREFIX)).length;
}
