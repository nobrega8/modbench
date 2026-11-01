import { describe, expect, it } from "vitest";
import {
  newStep,
  parseSequence,
  stepProblem,
  stepText,
  writes,
} from "./sequence";
import type { DeviceMap } from "./types";

const map = {
  meta: {},
  registers: [
    { name: "Setpoint", access: "RW", scale_factor: null },
    { name: "Frequency", access: "R", scale_factor: null },
  ],
} as unknown as DeviceMap;

describe("test sequences", () => {
  it("describes steps as the report does", () => {
    expect(stepText(newStep("write", "Setpoint"))).toBe("write Setpoint = 0");
    expect(stepText(newStep("wait", ""))).toBe("wait 1000 ms");
    expect(
      stepText({
        do: "expect",
        register: "Frequency",
        min: 49,
        max: 51,
        within_ms: 0,
      }),
    ).toBe("expect Frequency in 49..51");
    expect(
      stepText({
        do: "expect",
        register: "Setpoint",
        equals: 42,
        tolerance: 0.5,
        within_ms: 500,
      }),
    ).toBe("expect Setpoint = 42 ± 0.5 within 500 ms");
  });

  it("finds steps that cannot run on the map", () => {
    expect(stepProblem(newStep("write", "Setpoint"), map)).toBeNull();
    expect(stepProblem(newStep("write", "Frequency"), map)).toMatch(
      /read-only/,
    );
    expect(stepProblem(newStep("read", "Nope"), map)).toMatch(/not in the map/);
    expect(stepProblem(newStep("wait", ""), map)).toBeNull();
  });

  it("reads files and refuses unknown steps", () => {
    const seq = parseSequence(
      '{"name":"t","steps":[{"do":"write","register":"Setpoint","value":1}]}',
    );
    expect(writes(seq)).toBe(true);
    expect(() => parseSequence('{"steps":[{"do":"jump"}]}')).toThrow(
      /unknown step/,
    );
    expect(() => parseSequence("{}")).toThrow(/no steps/);
  });
});
