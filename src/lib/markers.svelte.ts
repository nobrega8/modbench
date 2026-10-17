// Markers A and B on the time axis of the charts, and the figures between
// them, per line.

import { nearest, type Sample } from "./trend";

export interface MarkerStats {
  /** Value at each marker (the nearest sample), NaN when there is none. */
  a: number;
  b: number;
  /** b − a. */
  diff: number;
  min: number;
  max: number;
  /** Mean of the good samples between the markers. */
  avg: number;
  /** Good samples between the markers. */
  count: number;
}

/** The value of the sample nearest to `t`, or NaN. */
function valueAt(samples: Sample[], t: number): number {
  return nearest(samples, t)?.v ?? NaN;
}

/** Figures of one line between two times (in either order). */
export function markerStats(
  samples: Sample[],
  a: number,
  b: number,
): MarkerStats {
  const lo = Math.min(a, b);
  const hi = Math.max(a, b);
  let min = Infinity;
  let max = -Infinity;
  let sum = 0;
  let count = 0;
  for (const s of samples) {
    if (s.t < lo) continue;
    if (s.t > hi) break;
    if (!Number.isFinite(s.v)) continue;
    min = Math.min(min, s.v);
    max = Math.max(max, s.v);
    sum += s.v;
    count++;
  }
  const va = valueAt(samples, a);
  const vb = valueAt(samples, b);
  return {
    a: va,
    b: vb,
    diff: vb - va,
    min: count ? min : NaN,
    max: count ? max : NaN,
    avg: count ? sum / count : NaN,
    count,
  };
}

/** Markers A and B of one chart view. */
export class Markers {
  a = $state<number | null>(null);
  b = $state<number | null>(null);

  /** Places A, then B; after that, a click moves the nearer marker. */
  place(t: number) {
    if (this.a === null) this.a = t;
    else if (this.b === null) this.b = t;
    else if (Math.abs(t - this.a) <= Math.abs(t - this.b)) this.a = t;
    else this.b = t;
  }

  move(which: "a" | "b", t: number) {
    this[which] = t;
  }

  clear() {
    this.a = null;
    this.b = null;
  }

  get any(): boolean {
    return this.a !== null || this.b !== null;
  }
}
