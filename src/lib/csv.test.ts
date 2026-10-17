import { describe, expect, it } from "vitest";
import { csvRow, logCsv, recordHeader, recordRow, valuesCsv } from "./csv";
import type { PointResult, Register } from "./types";

function reg(name: string, unit = "", scale = 1): Register {
  return {
    name,
    group: "",
    area: "holding",
    address: 7,
    type: "uint16",
    length: null,
    scale,
    scale_factor: null,
    offset: 0,
    unit,
    access: "R",
    write_min: null,
    write_max: null,
    reference: null,
    byte_order: null,
    bit: null,
    bits: null,
    enum: [],
    enum_ref: null,
    description: "",
    extra: [],
  };
}

function ok(index: number, value: number, raw: number): PointResult {
  return {
    index,
    value,
    raw: { kind: "registers", values: [raw] },
    error: null,
    sentinel: null,
    reference: null,
  };
}

describe("csv", () => {
  it("quotes only fields that need it", () => {
    expect(csvRow(["a", 1, "b;c", 'say "hi"'], ";")).toBe(
      'a;1;"b;c";"say ""hi"""',
    );
    expect(csvRow(["b;c", "1,5"], ",")).toBe('b;c,"1,5"');
  });

  it("writes the traffic log with a header", () => {
    const text = logCsv(
      [
        {
          time_ms: 0,
          function: 3,
          address: 10,
          count: 2,
          duration_ms: 4.25,
          error: null,
        },
      ],
      ",",
    );
    const [head, row] = text.trim().split("\n");
    expect(head).toBe("time,function,name,address,count,duration_ms,result");
    expect(row).toMatch(/,3,Read holding registers,10,2,4\.3,OK$/);
  });

  it("writes map values, leaving failed reads empty", () => {
    const regs = [reg("Voltage", "V", 0.1), reg("Status")];
    const results = new Map<number, PointResult>([
      [0, ok(0, 230.5, 2305)],
      [
        1,
        {
          index: 1,
          value: null,
          raw: null,
          error: "timed out after 1000 ms",
          sentinel: null,
          reference: null,
        },
      ],
    ]);
    const lines = valuesCsv(regs, [0, 1], results, ";").trim().split("\n");
    expect(lines[1]).toBe("Voltage;;holding;7;230.5;V;0901;");
    expect(lines[2]).toBe("Status;;holding;7;;;;timed out after 1000 ms");
  });

  it("records one row per cycle in a fixed column order", () => {
    const regs = [reg("A", "V"), reg("B")];
    expect(recordHeader([regs[1], regs[0]], ",")).toBe("time,B,A (V)\n");
    const row = recordRow(
      0,
      regs,
      [1, 0],
      new Map([[0, ok(0, 5, 5)]]),
      ",",
    ).trim();
    expect(row.split(",").slice(1)).toEqual(["", "5"]);
  });
});

describe("quality in CSV files", () => {
  it("records a failed read as #failed, and log notes as notes", () => {
    const reg = { name: "V", type: "uint16", scale: 1, offset: 0 } as Register;
    const failed: PointResult = {
      index: 0,
      reference: null,
      value: null,
      raw: null,
      error: "timed out",
      sentinel: null,
    };
    const row = recordRow(0, [reg], [0], new Map([[0, failed]]), ",");
    expect(row.trim().split(",")[1]).toBe("#failed");
    const log = logCsv(
      [
        {
          time_ms: 0,
          function: 0,
          address: 0,
          count: 0,
          duration_ms: 0,
          error: null,
          note: "V: read failed (timed out)",
        },
      ],
      ",",
    );
    expect(log.trim().split("\n")[1]).toMatch(
      /,,note,,,,V: read failed \(timed out\)$/,
    );
  });
});
