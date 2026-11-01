import { describe, expect, it } from "vitest";
import { keeps, monitorCsv, valuesText, whereText } from "./bus-monitor";
import type { MonitorEvent } from "./types";

function event(over: Partial<MonitorEvent> = {}): MonitorEvent {
  return {
    time_ms: 0,
    outcome: "ok",
    unit: 1,
    function: 3,
    address: 10,
    count: 2,
    registers: [7, 8],
    bits: null,
    exception: null,
    ms: 12,
    request: "01 03 00 0A 00 02 E4 09",
    response: "01 03 04 00 07 00 08 4B F3",
    ...over,
  };
}

describe("bus monitor", () => {
  it("shows values short", () => {
    expect(valuesText(event())).toBe("7 8");
    expect(valuesText(event({ registers: [1, 2, 3] }), 2)).toBe("1 2 …");
    expect(valuesText(event({ registers: null, bits: [true, false] }))).toBe(
      "10",
    );
    expect(whereText(event())).toBe("10 ×2");
    expect(whereText(event({ count: 1 }))).toBe("10");
  });

  it("filters by unit, function and problems", () => {
    const all = { unit: null, fn: null, problems: false };
    expect(keeps(all, event())).toBe(true);
    expect(keeps({ ...all, unit: 2 }, event())).toBe(false);
    expect(keeps({ ...all, fn: 4 }, event())).toBe(false);
    expect(keeps({ ...all, problems: true }, event())).toBe(false);
    expect(
      keeps({ ...all, problems: true }, event({ outcome: "no_answer" })),
    ).toBe(true);
  });

  it("writes every request as a CSV row", () => {
    const csv = monitorCsv([event(), event({ outcome: "bad_check" })], ";");
    const lines = csv.trim().split("\r\n");
    expect(lines).toHaveLength(3);
    expect(lines[0]).toMatch(/^time;unit;function;/);
    expect(lines[1]).toContain(";ok;");
    expect(lines[1]).toContain(";7 8;");
  });
});
