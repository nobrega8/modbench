// Zoom and pan on the time axis of the charts: the maths, on time ranges in
// ms. A range stays inside the bounds (the recorded data), never gets
// shorter than MIN_SPAN, and zooming keeps the time under the pointer where
// it is.

export interface Range {
  from: number;
  to: number;
}

/** The shortest range a chart can zoom into. */
export const MIN_SPAN = 500;

const span = (r: Range) => r.to - r.from;

/** The range moved into the bounds, and cut to them if it is longer. */
export function clampRange(r: Range, bounds: Range): Range {
  const room = Math.max(span(bounds), MIN_SPAN);
  const length = Math.min(Math.max(span(r), MIN_SPAN), room);
  let from = r.from;
  if (from < bounds.from) from = bounds.from;
  if (from + length > bounds.from + room) from = bounds.from + room - length;
  return { from, to: from + length };
}

/**
 * The range after zooming by `factor` around the time `at`: below 1 zooms in,
 * above 1 zooms out. The time `at` stays at the same place on the axis.
 */
export function zoomRange(
  r: Range,
  at: number,
  factor: number,
  bounds: Range,
): Range {
  const length = span(r) * factor;
  const where = span(r) === 0 ? 0.5 : (at - r.from) / span(r);
  const from = at - where * length;
  return clampRange({ from, to: from + length }, bounds);
}

/** The range moved by `dt` ms (negative: earlier). */
export function panRange(r: Range, dt: number, bounds: Range): Range {
  return clampRange({ from: r.from + dt, to: r.to + dt }, bounds);
}

/** The range between two times picked on the axis, or null when too short. */
export function selectionRange(
  a: number,
  b: number,
  bounds: Range,
): Range | null {
  const from = Math.min(a, b);
  const to = Math.max(a, b);
  if (to - from < MIN_SPAN) return null;
  return clampRange({ from, to }, bounds);
}

/** The range between the markers, with a little room on both sides. */
export function markersRange(
  a: number,
  b: number,
  bounds: Range,
): Range | null {
  const from = Math.min(a, b);
  const to = Math.max(a, b);
  if (to - from <= 0) return null;
  const pad = (to - from) * 0.05;
  return clampRange({ from: from - pad, to: to + pad }, bounds);
}

/** How much one notch of the mouse wheel zooms: a bit over 10 % a notch. */
export function wheelFactor(deltaY: number): number {
  return Math.exp(Math.max(-300, Math.min(300, deltaY)) * 0.0012);
}

/** The time axis range the data covers, or null when there is none. */
export function boundsOf(times: number[]): Range | null {
  if (times.length === 0) return null;
  let from = Infinity;
  let to = -Infinity;
  for (const t of times) {
    if (t < from) from = t;
    if (t > to) to = t;
  }
  return { from, to: Math.max(to, from + MIN_SPAN) };
}
