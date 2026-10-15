// Simulator generators: values that change on their own. The backend runs
// them; this turns them into an editable form and into short descriptions.

import type {
  Area,
  Generator,
  GeneratorKind,
  GeneratorTarget,
  Register,
} from "./types";

export type Kind = GeneratorKind["kind"];

export const KINDS: { id: Kind; label: string }[] = [
  { id: "sine", label: "Sine" },
  { id: "increment", label: "Increment" },
  { id: "random", label: "Random" },
  { id: "copy", label: "Copy" },
];

export const AREA_LABELS: Record<Area, string> = {
  holding: "Holding",
  input: "Input",
  coil: "Coil",
  discrete: "Discrete",
};

/** A target as the form edits it: both ways are kept while switching. */
export interface TargetDraft {
  mode: "register" | "address";
  register: string;
  area: Area;
  address: number;
}

/** Every field of every kind, so switching kind keeps what was typed. */
export interface Draft {
  to: TargetDraft;
  kind: Kind;
  every_ms: number;
  step: number;
  min: number;
  max: number;
  period_s: number;
  amplitude: number;
  offset: number;
  from: TargetDraft;
}

function targetDraft(t: GeneratorTarget | null, registers: Register[]) {
  const d: TargetDraft = {
    mode: registers.length ? "register" : "address",
    register: registers[0]?.name ?? "",
    area: "holding",
    address: 0,
  };
  if (t && "register" in t)
    return { ...d, mode: "register" as const, register: t.register };
  if (t)
    return { ...d, mode: "address" as const, area: t.area, address: t.address };
  return d;
}

/** A new generator's form: a sine on the first map register, if any. */
export function newDraft(registers: Register[]): Draft {
  return {
    to: targetDraft(null, registers),
    kind: "sine",
    every_ms: 1000,
    step: 1,
    min: 0,
    max: 100,
    period_s: 10,
    amplitude: 10,
    offset: 50,
    from: { ...targetDraft(null, []), area: "coil" },
  };
}

/** The form for an existing generator. */
export function toDraft(g: Generator, registers: Register[]): Draft {
  const d = newDraft(registers);
  d.to = targetDraft(g, registers);
  d.kind = g.kind;
  d.every_ms = g.every_ms;
  switch (g.kind) {
    case "increment":
      Object.assign(d, { step: g.step, min: g.min, max: g.max });
      break;
    case "sine":
      Object.assign(d, {
        period_s: g.period_s,
        amplitude: g.amplitude,
        offset: g.offset,
      });
      break;
    case "random":
      Object.assign(d, { min: g.min, max: g.max });
      break;
    case "copy":
      d.from = targetDraft(g.from, registers);
      break;
  }
  return d;
}

function target(t: TargetDraft): GeneratorTarget {
  return t.mode === "register"
    ? { register: t.register }
    : { area: t.area, address: t.address };
}

/** The generator a form describes. */
export function fromDraft(d: Draft): Generator {
  const kind: GeneratorKind =
    d.kind === "increment"
      ? { kind: "increment", step: d.step, min: d.min, max: d.max }
      : d.kind === "sine"
        ? {
            kind: "sine",
            period_s: d.period_s,
            amplitude: d.amplitude,
            offset: d.offset,
          }
        : d.kind === "random"
          ? { kind: "random", min: d.min, max: d.max }
          : { kind: "copy", from: target(d.from) };
  return { ...target(d.to), ...kind, every_ms: d.every_ms };
}

/** Where a generator writes, without its kind. */
export function targetOf(g: GeneratorTarget): GeneratorTarget {
  return "register" in g
    ? { register: g.register }
    : { area: g.area, address: g.address };
}

/** The addresses a target takes, when they can be told. */
/**
 * The map register a target writes as a whole: the one it names, or the one
 * that starts at its address (the backend does the same, so a uint32 does
 * not get its value in the high word only).
 */
export function registerOf(
  t: GeneratorTarget,
  registers: Register[],
): Register | undefined {
  if ("register" in t) {
    const key = t.register.trim().toLowerCase();
    return registers.find((r) => r.name.trim().toLowerCase() === key);
  }
  if (t.area === "coil" || t.area === "discrete") return undefined;
  return registers.find(
    (r) =>
      r.area === t.area &&
      r.address === t.address &&
      r.bit === null &&
      r.type !== "bool" &&
      r.type !== "string",
  );
}

export function span(
  t: GeneratorTarget,
  registers: Register[],
): { area: Area; address: number; count: number } | null {
  const r = registerOf(t, registers);
  if (!r) {
    return "register" in t
      ? null
      : { area: t.area, address: t.address, count: 1 };
  }
  const words: Record<string, number> = {
    int32: 2,
    uint32: 2,
    float32: 2,
    int64: 4,
    uint64: 4,
    float64: 4,
  };
  return { area: r.area, address: r.address, count: words[r.type] ?? 1 };
}

const num = (n: number) => String(Number(n.toPrecision(6)));

/** One line about what a generator does, e.g. "230 ± 5 V, 10 s period". */
export function describe(
  g: Generator,
  name: (t: GeneratorTarget) => string,
  unit = "",
): string {
  const u = unit ? ` ${unit}` : "";
  switch (g.kind) {
    case "increment":
      return `${g.step < 0 ? "−" : "+"}${num(Math.abs(g.step))}${u} each time, ${num(g.min)} to ${num(g.max)}`;
    case "sine":
      return `${num(g.offset)} ± ${num(g.amplitude)}${u}, ${num(g.period_s)} s period`;
    case "random":
      return `${num(g.min)} to ${num(g.max)}${u}`;
    case "copy":
      return `Follows ${name(g.from)}`;
  }
}

/** A register's reference value in engineering units, when it is a number. */
export function referenceValue(r: Register): number | null {
  const text = r.reference?.trim();
  if (!text) return null;
  const raw = /^0x[0-9a-f]+$/i.test(text) ? parseInt(text, 16) : Number(text);
  return Number.isFinite(raw) ? raw * r.scale + r.offset : null;
}

/**
 * A new generator for one register, shaped by what the map says about it:
 * a bit toggles, an enum steps through its values, a number swings around
 * its reference within its write limits.
 */
export function draftFor(r: Register, registers: Register[]): Draft {
  const d = newDraft(registers);
  d.to = { ...d.to, mode: "register", register: r.name };
  const isBit = r.type === "bool" || r.area === "coil" || r.area === "discrete";
  if (isBit) {
    return { ...d, kind: "increment", step: 1, min: 0, max: 1, every_ms: 2000 };
  }
  if (r.enum.length) {
    const keys = r.enum.map(([k]) => k * r.scale + r.offset);
    return {
      ...d,
      kind: "increment",
      step: r.scale,
      min: Math.min(...keys),
      max: Math.max(...keys),
      every_ms: 2000,
    };
  }
  const lo = r.write_min;
  const hi = r.write_max;
  const centre =
    referenceValue(r) ?? (lo !== null && hi !== null ? (lo + hi) / 2 : 0);
  const swing =
    lo !== null && hi !== null ? (hi - lo) / 4 : Math.abs(centre) * 0.05 || 10;
  const round = (n: number) => Number(n.toPrecision(4));
  return {
    ...d,
    kind: "sine",
    offset: round(centre),
    amplitude: round(swing),
    min: round(lo ?? centre - 2 * swing),
    max: round(hi ?? centre + 2 * swing),
  };
}
