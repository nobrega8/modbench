// Which values changed recently, so the screen can highlight them for a
// while (useful to find what moves on an unknown device).

import type { PointResult, RawData } from "./types";

/** When each key last changed (ms since epoch). */
export type ChangeTimes = Map<string, number>;

/** Word or bit `i` of a read, as a comparable value. */
function at(data: RawData, i: number): number | boolean | undefined {
  return data.values[i];
}

/**
 * Marks every address offset whose value differs between two reads of the
 * same block. Keys are `g:<offset>`. Returns `times` itself when nothing
 * changed.
 */
export function markRead(
  times: ChangeTimes,
  prev: RawData | null,
  next: RawData,
  now: number,
): ChangeTimes {
  if (!prev || prev.kind !== next.kind) return times;
  let out = times;
  for (let i = 0; i < next.values.length; i++) {
    if (i < prev.values.length && at(prev, i) !== at(next, i)) {
      if (out === times) out = new Map(times);
      out.set(`g:${i}`, now);
    }
  }
  return out;
}

function sameRaw(a: RawData | null, b: RawData | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.kind === b.kind &&
    a.values.length === b.values.length &&
    a.values.every((v, i) => v === b.values[i])
  );
}

/**
 * Marks map registers whose raw value differs from the last successful
 * read. Keys are `m:<index>`. A failed read is not a change.
 */
export function markPoll(
  times: ChangeTimes,
  prev: Map<number, PointResult>,
  results: PointResult[],
  now: number,
): ChangeTimes {
  let out = times;
  for (const r of results) {
    const before = prev.get(r.index);
    if (!before?.raw || !r.raw || r.error) continue;
    if (!sameRaw(before.raw, r.raw)) {
      if (out === times) out = new Map(times);
      out.set(`m:${r.index}`, now);
    }
  }
  return out;
}

/** Whether `key` changed less than `holdMs` before `now`. */
export function isFresh(
  times: ChangeTimes,
  key: string,
  now: number,
  holdMs: number,
): boolean {
  const t = times.get(key);
  return t !== undefined && now - t < holdMs;
}

/** Drops entries older than `holdMs`; returns `times` if none expired. */
export function prune(
  times: ChangeTimes,
  now: number,
  holdMs: number,
): ChangeTimes {
  let out = times;
  for (const [k, t] of times) {
    if (now - t >= holdMs) {
      if (out === times) out = new Map(times);
      out.delete(k);
    }
  }
  return out;
}
