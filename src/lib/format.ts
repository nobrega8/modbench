// Formatting of values for display and parsing of what people type.

import { t, td } from "./i18n.svelte.ts";
import type { Area, RawData, Register, Value } from "./types";

export const AREA_TAG: Record<Area, string> = {
  coil: "CO",
  discrete: "DI",
  input: "IR",
  holding: "HR",
};

// Getters, so that reading a name always gives the current language.
export const AREA_NAME: Record<Area, string> = {
  get coil() {
    return t("area.coil");
  },
  get discrete() {
    return t("area.discrete");
  },
  get input() {
    return t("area.input");
  },
  get holding() {
    return t("area.holding");
  },
};

/** The plural names of the areas, for headings and lists. */
export const AREA_NAMES: Record<Area, string> = {
  get coil() {
    return t("areas.coil");
  },
  get discrete() {
    return t("areas.discrete");
  },
  get input() {
    return t("areas.input");
  },
  get holding() {
    return t("areas.holding");
  },
};

export const FUNCTION_NAME: Record<number, string> = {
  get 1() {
    return td("fc.1");
  },
  get 2() {
    return td("fc.2");
  },
  get 3() {
    return td("fc.3");
  },
  get 4() {
    return td("fc.4");
  },
  get 5() {
    return td("fc.5");
  },
  get 6() {
    return td("fc.6");
  },
  get 15() {
    return td("fc.15");
  },
  get 16() {
    return td("fc.16");
  },
};

export function hex(n: number, digits = 4): string {
  return n.toString(16).toUpperCase().padStart(digits, "0");
}

/**
 * How addresses are shown: as sent on the wire (0-based), as most manuals
 * number them (1-based), in hexadecimal (wire address), in Modicon notation
 * (`400101`: the area's digit, then the 1-based address in five digits) or
 * in IEC 61131-3 notation (`%MW100`: the area's prefix, then the 0-based
 * address).
 */
export type AddressFormat = "base0" | "base1" | "hex" | "modicon" | "iec";

export const ADDRESS_FORMATS: { id: AddressFormat; readonly label: string }[] =
  [
    {
      id: "base0",
      get label() {
        return t("addrfmt.base0");
      },
    },
    {
      id: "base1",
      get label() {
        return t("addrfmt.base1");
      },
    },
    {
      id: "hex",
      get label() {
        return t("addrfmt.hex");
      },
    },
    { id: "modicon", label: "Modicon (400001)" },
    { id: "iec", label: "IEC 61131-3 (%MW0)" },
  ];

/** First digit of a Modicon address, by area. */
const MODICON_DIGIT: Record<Area, string> = {
  coil: "0",
  discrete: "1",
  input: "3",
  holding: "4",
};

/** Prefix of an IEC 61131-3 address, by area. */
const IEC_PREFIX: Record<Area, string> = {
  coil: "%Q",
  discrete: "%I",
  input: "%IW",
  holding: "%MW",
};

/** The area a function code reads or writes, or null for other functions. */
export function areaOfFunction(fc: number): Area | null {
  if (fc === 1 || fc === 5 || fc === 15) return "coil";
  if (fc === 2) return "discrete";
  if (fc === 4) return "input";
  if (fc === 3 || fc === 6 || fc === 16) return "holding";
  return null;
}

/**
 * A wire address for display. Modicon and IEC need the area; without one
 * they fall back to 1-based and 0-based. `pad` zero-pads decimal forms to 5
 * digits.
 */
export function formatAddress(
  n: number,
  format: AddressFormat,
  area: Area | null = null,
  pad = false,
): string {
  if (format === "hex") return `${hex(n)}h`;
  if (format === "modicon" && area)
    return MODICON_DIGIT[area] + String(n + 1).padStart(5, "0");
  if (format === "iec" && area) return IEC_PREFIX[area] + n;
  const s = String(format === "base1" || format === "modicon" ? n + 1 : n);
  return pad ? s.padStart(5, "0") : s;
}

/** A typed address: the wire address, and the area its notation names. */
export interface ParsedAddress {
  address: number;
  /** Set by a Modicon or IEC address; null for a plain number. */
  area: Area | null;
}

/**
 * An address typed by a person. IEC 61131-3 addresses (`%MW100`, `%IW0`,
 * `%I3`, `%Q16#1F`) and six-digit Modicon addresses above 65535 (`400101`)
 * are read in any format, and name their area. With the Modicon format, five
 * and six digits starting with 0, 1, 3 or 4 are Modicon too (`40101`).
 * Anything else is a plain number (see `parseInteger`), returned as typed.
 * Null when the text is not an address.
 */
export function parseAddress(
  text: string,
  format: AddressFormat,
): ParsedAddress | null {
  const t = text.trim();
  const iec = /^%(IW|MW|I|Q)X?\s*(.+)$/i.exec(t);
  if (iec) {
    const digits = iec[2].trim();
    const radix = /^16#([0-9a-f]+)$/i.exec(digits);
    const n = radix ? parseInt(radix[1], 16) : parseInteger(digits);
    if (n === null || n < 0 || n > 0xffff) return null;
    const area = (Object.keys(IEC_PREFIX) as Area[]).find(
      (a) => IEC_PREFIX[a] === `%${iec[1].toUpperCase()}`,
    )!;
    return { address: n, area };
  }
  const modicon =
    /^\d{5,6}$/.test(t) &&
    (format === "modicon" || (t.length === 6 && Number(t) > 0xffff));
  if (modicon) {
    const area = (Object.keys(MODICON_DIGIT) as Area[]).find(
      (a) => MODICON_DIGIT[a] === t[0],
    );
    const n = Number(t.slice(1));
    if (area && n >= 1 && n <= 0x10000) return { address: n - 1, area };
  }
  const n = parseInteger(t);
  return n === null ? null : { address: n, area: null };
}

/**
 * Integer typed by a person: decimal, `0x1F` or `1Fh` (the suffix form is
 * common in device manuals). Returns null if it is not a whole number.
 */
export function parseInteger(text: string): number | null {
  const t = text.trim();
  let m = /^([+-]?)0x([0-9a-f]+)$/i.exec(t) ?? /^([+-]?)([0-9a-f]+)h$/i.exec(t);
  if (m) return (m[1] === "-" ? -1 : 1) * parseInt(m[2], 16);
  m = /^[+-]?\d+$/.exec(t);
  return m ? parseInt(t, 10) : null;
}

export function parseBool(text: string): boolean | null {
  const t = text.trim().toLowerCase();
  if (["1", "true", "on", "yes"].includes(t)) return true;
  if (["0", "false", "off", "no"].includes(t)) return false;
  return null;
}

/** Turns typed text into a value for a register, or explains why not. */
export function parseForRegister(
  reg: Register,
  text: string,
): { value: Value } | { error: string } {
  if (reg.type === "bool") {
    const b = parseBool(text);
    return b === null ? { error: t("fmt.bool") } : { value: b };
  }
  if (reg.type === "string") return { value: text };
  const label = reg.enum.find(
    ([, l]) => l.toLowerCase() === text.trim().toLowerCase(),
  );
  // Labels name raw values; the backend expects engineering units.
  if (label) return { value: label[0] * reg.scale + reg.offset };
  const asInt = parseInteger(text);
  if (asInt !== null) return { value: asInt };
  const n = Number(text.trim().replace(",", "."));
  if (text.trim() !== "" && Number.isFinite(n)) return { value: n };
  return { error: t("fmt.number") };
}

/** Why a numeric write value is outside the register's declared limits. */
export function limitError(reg: Register, value: Value): string | null {
  if (typeof value !== "number") return null;
  const unit = reg.unit ? ` ${reg.unit}` : "";
  if (reg.write_min !== null && value < reg.write_min)
    return `Below the minimum of ${reg.write_min}${unit}.`;
  if (reg.write_max !== null && value > reg.write_max)
    return `Above the maximum of ${reg.write_max}${unit}.`;
  return null;
}

/** The accepted range as short text, or null when the register has none. */
export function limitHint(reg: Register): string | null {
  const { write_min: lo, write_max: hi } = reg;
  if (lo !== null && hi !== null) return `${lo} – ${hi}`;
  if (lo !== null) return `≥ ${lo}`;
  if (hi !== null) return `≤ ${hi}`;
  return null;
}

/** The raw value and its label, when the register's enum has a label for
 * this value. Labels match the raw value, before scale and offset. */
export function enumValue(
  reg: Register,
  value: Value | null,
): { raw: number; label: string } | null {
  if (value === null || value === undefined || typeof value === "string")
    return null;
  const raw =
    typeof value === "boolean"
      ? Number(value)
      : Math.round((value - reg.offset) / reg.scale);
  const label = reg.enum.find(([v]) => v === raw);
  return label ? { raw, label: label[1] } : null;
}

/** Tooltip of a value: the full text, plus the real value behind an enum label. */
export function valueTitle(reg: Register, value: Value | null): string {
  const labelled = enumValue(reg, value);
  if (labelled) return `${labelled.label}\nValue: ${labelled.raw}`;
  return formatValue(reg, value);
}

export function formatValue(reg: Register, value: Value | null): string {
  if (value === null || value === undefined) return "—";
  const labelled = enumValue(reg, value);
  if (labelled) return labelled.label;
  if (typeof value === "boolean") return value ? "ON" : "OFF";
  if (typeof value === "string") return value;
  if (Number.isInteger(value)) return value.toString();
  // Enough digits for the scale, without float noise.
  const decimals =
    reg.scale !== 1 && reg.scale > 0
      ? Math.min(6, Math.max(0, Math.ceil(-Math.log10(reg.scale))))
      : 3;
  return value.toFixed(decimals).replace(/\.?0+$/, "") || "0";
}

export function formatRaw(raw: RawData | null): string {
  if (!raw) return "";
  if (raw.kind === "bits")
    return raw.values.map((b) => (b ? "1" : "0")).join("");
  return raw.values.map((w) => hex(w)).join(" ");
}

export function clock(ms: number): string {
  const d = new Date(ms);
  const p = (n: number, w = 2) => n.toString().padStart(w, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}.${p(d.getMilliseconds(), 3)}`;
}

/** A length of time, e.g. "5 min 0 s" or "1 h 2 min". */
export function duration(ms: number): string {
  const s = Math.round(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h ? `${h} h ${m} min` : m ? `${m} min ${s % 60} s` : `${s} s`;
}

/** Whether values of an area can be written now, and how to say so. */
export function areaAccess(
  area: Area,
  readOnlyMode: boolean,
): { writable: boolean; text: string } {
  const name = AREA_NAMES[area];
  if (area === "input" || area === "discrete")
    return { writable: false, text: t("access.readonly", { name }) };
  if (readOnlyMode)
    return { writable: false, text: t("access.readonlymode", { name }) };
  return { writable: true, text: t("access.writable", { name }) };
}
