import { describe, expect, it } from "vitest";
import { recordHeader, recordRow } from "./csv";
import {
  cellNumber,
  parseCsv,
  parseRecording,
  parseStamp,
  upTo,
} from "./playback";
import type { PointResult, Register } from "./types";

describe("playback", () => {
  it("splits CSV with quotes", () => {
    expect(parseCsv('a,"b,c","d""e"\r\n1,2,3\n', ",")).toEqual([
      ["a", "b,c", 'd"e'],
      ["1", "2", "3"],
    ]);
  });

  it("reads stamps and cells", () => {
    expect(parseStamp("2026-10-14 09:30:05.120")).toBe(
      new Date(2026, 9, 14, 9, 30, 5, 120).getTime(),
    );
    expect(parseStamp("yesterday")).toBeNaN();
    expect(cellNumber("ON")).toBe(1);
    expect(cellNumber("12,5")).toBe(12.5);
    expect(cellNumber("Running")).toBeNaN();
  });

  it("reads back what Record… writes", () => {
    const reg = (name: string, unit: string) =>
      ({
        name,
        unit,
        type: "float32",
        scale: 1,
        offset: 0,
        enum: [],
      }) as unknown as Register;
    const registers = [reg("Voltage", "V"), reg("State", "")];
    const result = (index: number, value: number | boolean): PointResult => ({
      index,
      value,
      raw: null,
      error: null,
      sentinel: null,
      reference: null,
    });
    for (const sep of [",", ";"] as const) {
      const t0 = new Date(2026, 0, 1, 12, 0, 0).getTime();
      const text =
        recordHeader(registers, sep) +
        recordRow(
          t0,
          registers,
          [0, 1],
          new Map([
            [0, result(0, 230.5)],
            [1, result(1, true)],
          ]),
          sep,
        ) +
        recordRow(
          t0 + 1000,
          registers,
          [0, 1],
          new Map([[1, result(1, false)]]),
          sep,
        );
      const rec = parseRecording(text);
      if ("error" in rec) throw new Error(rec.error);
      expect(rec.rows).toBe(2);
      expect(rec.columns.map((c) => [c.name, c.unit])).toEqual([
        ["Voltage", "V"],
        ["State", ""],
      ]);
      expect(rec.columns[0].samples[0]).toEqual({ t: t0, v: 230.5 });
      expect(rec.columns[0].samples[1].v).toBeNaN();
      expect(rec.columns[1].samples.map((s) => s.v)).toEqual([1, 0]);
      expect(rec.end - rec.start).toBe(1000);
    }
    expect(parseRecording("a,b\n1,2\n")).toHaveProperty("error");
  });

  it("cuts samples at the play head", () => {
    const s = [1, 2, 3].map((t) => ({ t, v: t }));
    expect(upTo(s, 2).map((x) => x.t)).toEqual([1, 2]);
    expect(upTo(s, 0)).toEqual([]);
    expect(upTo(s, 9)).toBe(s);
  });
});
