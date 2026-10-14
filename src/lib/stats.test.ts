import { describe, expect, it } from "vitest";
import {
  NO_BREAKDOWN,
  NO_TIMES,
  addEntry,
  addTime,
  answered,
  average,
  cause,
  ms,
} from "./stats";
import type { TrafficEntry } from "./types";

function entry(duration_ms: number, error: string | null = null): TrafficEntry {
  return { time_ms: 0, function: 3, address: 0, count: 1, duration_ms, error };
}

describe("response times", () => {
  it("tracks last, min, max and average", () => {
    let t = NO_TIMES;
    for (const d of [12, 4, 20]) t = addTime(t, entry(d));
    expect(t).toMatchObject({ count: 3, last: 20, min: 4, max: 20 });
    expect(average(t)).toBe(12);
  });

  it("counts exceptions but not timeouts or link errors", () => {
    expect(
      answered(entry(5, "device exception 0x02 (illegal data address)")),
    ).toBe(true);
    expect(answered(entry(1000, "timed out after 1000 ms"))).toBe(false);
    expect(answered(entry(1, "I/O error: broken pipe"))).toBe(false);
    const t = addTime(NO_TIMES, entry(1000, "timed out after 1000 ms"));
    expect(t).toBe(NO_TIMES);
    expect(average(t)).toBe(0);
  });

  it("formats milliseconds", () => {
    expect(ms(3.14159)).toBe("3.1");
    expect(ms(250.6)).toBe("251");
  });
});

describe("breakdown", () => {
  const entry = (fn: number, error: string | null, ms = 10): TrafficEntry => ({
    time_ms: 0,
    function: fn,
    address: 0,
    count: 1,
    duration_ms: ms,
    error,
  });

  it("names the cause of each failure", () => {
    expect(cause(entry(3, null))).toBe("OK");
    expect(cause(entry(3, "timed out after 1000 ms"))).toBe(
      "Timeout (no answer)",
    );
    expect(
      cause(entry(3, "device exception 0x02 (illegal data address)")),
    ).toBe("Exception 2: illegal data address");
    expect(cause(entry(3, "protocol error: mismatching headers"))).toBe(
      "Bad frame",
    );
    expect(cause(entry(3, "I/O error: broken pipe"))).toBe("Connection error");
  });

  it("counts requests by cause and by function", () => {
    let b = NO_BREAKDOWN;
    b = addEntry(b, entry(3, null, 10));
    b = addEntry(b, entry(3, "timed out after 1000 ms", 1000));
    b = addEntry(b, entry(16, null, 20));
    expect(b.total).toBe(3);
    expect(b.causes.get("OK")).toBe(2);
    expect(b.causes.get("Timeout (no answer)")).toBe(1);
    const fc3 = b.functions.get(3)!;
    expect(fc3).toMatchObject({ count: 2, failed: 1 });
    // The timeout does not count as a response time.
    expect(fc3.times.count).toBe(1);
    expect(NO_BREAKDOWN.total).toBe(0);
  });
});
