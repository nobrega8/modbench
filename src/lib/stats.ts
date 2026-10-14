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

/** Milliseconds as short text: one decimal below 100 ms, whole above. */
export function ms(value: number): string {
  return value < 100 ? value.toFixed(1) : value.toFixed(0);
}
