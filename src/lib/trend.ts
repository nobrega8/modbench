// Values over time for the Chart tab: history, scales and ticks.

import type { PointResult, Register, Value } from "./types";

/** Most series on the chart: one per categorical color slot. */
export const MAX_SERIES = 8;
/** Points kept per series; the oldest are dropped first. */
export const MAX_POINTS = 20_000;

/**
 * Why a sample has no value: the read failed, or the device sent no usable
 * value (a sentinel such as "Unimplemented", or nothing to plot). A sample
 * without `q` is a good reading; NaN without `q` is a gap of unknown cause
 * (an older recording).
 */
export type Quality = "failed" | "unavailable";

export interface Sample {
  t: number;
  v: number;
  q?: Quality;
}

/** A sample for one register result: its value, or NaN with the reason. */
export function sampleOf(r: PointResult, t: number): Sample {
  if (r.error) return { t, v: NaN, q: "failed" };
  if (r.sentinel) return { t, v: NaN, q: "unavailable" };
  const v = plotValue(r.value);
  return v === null ? { t, v: NaN, q: "unavailable" } : { t, v };
}

/** The last good sample at or before index `i`, for showing a held value. */
export function lastGood(samples: Sample[], i: number): Sample | null {
  for (let k = Math.min(i, samples.length - 1); k >= 0; k--) {
    if (Number.isFinite(samples[k].v)) return samples[k];
  }
  return null;
}

export interface Series {
  /** Map index of the register. */
  index: number;
  /** Color slot 1-8, kept for as long as the register is on the chart. */
  slot: number;
}

/** Whether a register can be charted: a number or a bit, and readable. */
export function chartable(reg: Register): boolean {
  return reg.type !== "string" && reg.access !== "W";
}

/** A value as a number for the chart, or null when there is none. */
export function plotValue(value: Value | null): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "boolean") return value ? 1 : 0;
  return null;
}

/** The first color slot no series uses. */
export function freeSlot<T extends { slot: number }>(
  series: T[],
): number | null {
  for (let s = 1; s <= MAX_SERIES; s++) {
    if (!series.some((x) => x.slot === s)) return s;
  }
  return null;
}

/**
 * Adds one cycle's results to the history of the charted registers. Failed
 * reads and sentinels leave a gap (NaN, with the reason), so the line breaks
 * there instead of dropping to zero. The
 * sample lists are appended in place (they can be long); the returned map is
 * new, so a change is seen.
 */
export function addCycle(
  history: Map<number, Sample[]>,
  series: Series[],
  results: PointResult[],
  t: number,
): Map<number, Sample[]> {
  const next = new Map(history);
  for (const { index } of series) {
    const r = results.find((x) => x.index === index);
    if (!r) continue;
    const list = next.get(index) ?? [];
    list.push(sampleOf(r, t));
    if (list.length > MAX_POINTS) list.splice(0, list.length - MAX_POINTS);
    next.set(index, list);
  }
  return next;
}

/** Round step sizes (1, 2, 5 × 10^n) and the ticks they give in [lo, hi]. */
export function niceTicks(lo: number, hi: number, count = 5): number[] {
  if (!Number.isFinite(lo) || !Number.isFinite(hi)) return [];
  if (lo === hi) {
    const pad = Math.abs(lo) * 0.1 || 1;
    lo -= pad;
    hi += pad;
  }
  const raw = (hi - lo) / Math.max(1, count);
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step =
    [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
  const ticks: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) {
    ticks.push(Number(v.toPrecision(12)));
  }
  return ticks;
}

/** Y range for the samples in view, padded and snapped to round ticks. */
export function yRange(values: number[]): [number, number] | null {
  const finite = values.filter(Number.isFinite);
  if (!finite.length) return null;
  const lo = Math.min(...finite);
  const hi = Math.max(...finite);
  const ticks = niceTicks(lo, hi);
  if (!ticks.length) return null;
  const step = ticks.length > 1 ? ticks[1] - ticks[0] : 1;
  return [
    Math.min(ticks[0], Math.floor(lo / step) * step),
    Math.max(ticks[ticks.length - 1], Math.ceil(hi / step) * step),
  ];
}

/** Charted registers grouped by unit: one chart per unit, one y-axis each. */
export function groupByUnit(
  series: Series[],
  registers: Register[],
): { unit: string; series: Series[] }[] {
  const groups: { unit: string; series: Series[] }[] = [];
  for (const s of series) {
    const unit = registers[s.index]?.unit ?? "";
    let g = groups.find((x) => x.unit === unit);
    if (!g) groups.push((g = { unit, series: [] }));
    g.series.push(s);
  }
  return groups;
}

/** The sample nearest to time `t` (samples are in time order). */
export function nearest(samples: Sample[], t: number): Sample | null {
  if (!samples.length) return null;
  let lo = 0;
  let hi = samples.length - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (samples[mid].t < t) lo = mid + 1;
    else hi = mid;
  }
  const prev = samples[lo - 1];
  return prev && t - prev.t < samples[lo].t - t ? prev : samples[lo];
}

/** A value on a chart's axis, legend or tip: six digits at most. */
export function valueLabel(v: number): string {
  if (!Number.isFinite(v)) return "—";
  const abs = Math.abs(v);
  if (abs !== 0 && (abs >= 1e6 || abs < 1e-3)) return v.toExponential(2);
  return String(Number(v.toPrecision(6)));
}

/** The newest value of a line, as its legend shows it. */
export function lastLabel(samples: Sample[]): string {
  const last = samples.at(-1);
  return last ? valueLabel(last.v) : "—";
}
