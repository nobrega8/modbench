// Response time statistics for the Test step.

import type { TrafficEntry } from "./types";

export interface ResponseTimes {
  /** Responses measured so far. */
  count: number;
  last: number;
  min: number;
  max: number;
  /** Sum of every measured duration, for the average. */
  total: number;
}

export const NO_TIMES: ResponseTimes = {
  count: 0,
  last: 0,
  min: 0,
  max: 0,
  total: 0,
};

/**
 * Whether a request's duration says how fast the device answers: a normal
 * response or an exception. A timeout only measures the timeout setting, and
 * an I/O error measures nothing about the device.
 */
export function answered(entry: TrafficEntry): boolean {
  return entry.error === null || entry.error.startsWith("device exception");
}

/** Adds one request to the statistics, when the device answered it. */
export function addTime(
  times: ResponseTimes,
  entry: TrafficEntry,
): ResponseTimes {
  if (!answered(entry)) return times;
  const ms = entry.duration_ms;
  const first = times.count === 0;
  return {
    count: times.count + 1,
    last: ms,
    min: first ? ms : Math.min(times.min, ms),
    max: first ? ms : Math.max(times.max, ms),
    total: times.total + ms,
  };
}

export function average(times: ResponseTimes): number {
  return times.count ? times.total / times.count : 0;
}

/** Answers needed before a timeout is suggested. */
export const SUGGEST_AFTER = 20;
/** A suggested timeout is never shorter than this. */
export const TIMEOUT_FLOOR_MS = 100;
/** Step of the Timeout field in step 1; suggestions round up to it. */
const TIMEOUT_STEP_MS = 50;

/**
 * A timeout that leaves room for slow answers without waiting long for a
 * lost one: 4 × the slowest answer measured, rounded up to the step of the
 * Timeout field, never below the floor. Null until enough answers came.
 */
export function suggestedTimeout(times: ResponseTimes): number | null {
  if (times.count < SUGGEST_AFTER) return null;
  const ms = Math.ceil((times.max * 4) / TIMEOUT_STEP_MS) * TIMEOUT_STEP_MS;
  return Math.max(TIMEOUT_FLOOR_MS, ms);
}

/** Milliseconds as short text: one decimal below 100 ms, whole above. */
export function ms(value: number): string {
  return value < 100 ? value.toFixed(1) : value.toFixed(0);
}

/** Why a request failed, as a short name; "OK" when it did not. */
export function cause(entry: TrafficEntry): string {
  const e = entry.error;
  if (e === null) return "OK";
  if (e.startsWith("timed out")) return "Timeout (no answer)";
  const ex = /^device exception 0x([0-9A-F]{2}) \((.+)\)$/.exec(e);
  if (ex) return `Exception ${parseInt(ex[1], 16)}: ${ex[2]}`;
  if (e.startsWith("protocol error")) return "Bad frame";
  if (e.startsWith("I/O error") || e.startsWith("serial port error"))
    return "Connection error";
  if (e.startsWith("read-only mode")) return "Blocked by read-only mode";
  return "Other error";
}

/** Requests of one function code: how many, how many failed, how fast. */
export interface FunctionStats {
  count: number;
  failed: number;
  times: ResponseTimes;
}

/** Every request since the last reset, by cause and by function code. */
export interface Breakdown {
  total: number;
  causes: Map<string, number>;
  functions: Map<number, FunctionStats>;
}

export const NO_BREAKDOWN: Breakdown = {
  total: 0,
  causes: new Map(),
  functions: new Map(),
};

/** Adds one request; returns a new value so a change is seen. */
export function addEntry(b: Breakdown, entry: TrafficEntry): Breakdown {
  const causes = new Map(b.causes);
  const c = cause(entry);
  causes.set(c, (causes.get(c) ?? 0) + 1);
  const functions = new Map(b.functions);
  const f = functions.get(entry.function) ?? {
    count: 0,
    failed: 0,
    times: NO_TIMES,
  };
  functions.set(entry.function, {
    count: f.count + 1,
    failed: f.failed + (entry.error === null ? 0 : 1),
    times: addTime(f.times, entry),
  });
  return { total: b.total + 1, causes, functions };
}

/** Response times of every function code together (last is not kept). */
export function allTimes(b: Breakdown): ResponseTimes {
  let all = NO_TIMES;
  for (const { times: t } of b.functions.values()) {
    if (!t.count) continue;
    const first = all.count === 0;
    all = {
      count: all.count + t.count,
      last: 0,
      min: first ? t.min : Math.min(all.min, t.min),
      max: first ? t.max : Math.max(all.max, t.max),
      total: all.total + t.total,
    };
  }
  return all;
}
