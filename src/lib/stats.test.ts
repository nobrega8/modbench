import { describe, expect, it } from "vitest";
import { NO_TIMES, addTime, answered, average, ms } from "./stats";
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
