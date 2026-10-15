import { describe, expect, it } from "vitest";
import {
  addRead,
  chartType,
  covers,
  decodePair,
  sameSeries,
  seriesName,
  valueIn,
  type ManualRead,
  type ManualSeries,
} from "./manual-chart";

const hr = (address: number, values: number[]): ManualRead => ({
  address,
  area: "holding",
  data: { kind: "registers", values },
});

const series = (
  address: number,
  type: ManualSeries["type"],
  slot: number,
  order: ManualSeries["order"] = "ABCD",
): ManualSeries => ({ area: "holding", address, type, order, slot });

describe("manual chart", () => {
  it("plots each grid format as a number type", () => {
    expect(chartType("signed", "holding")).toBe("int16");
    expect(chartType("hex", "input")).toBe("uint16");
    expect(chartType("bits", "holding")).toBe("uint16");
    expect(chartType("float32", "holding")).toBe("float32");
    expect(chartType("ascii", "holding")).toBeNull();
    expect(chartType("float32", "coil")).toBe("bool");
  });

  it("reads 32-bit values in every byte order", () => {
    // 1.5 as a float is 0x3FC0 0000.
    expect(decodePair(0x3fc0, 0, "float32", "ABCD")).toBe(1.5);
    expect(decodePair(0, 0x3fc0, "float32", "CDAB")).toBe(1.5);
    expect(decodePair(0xc03f, 0, "float32", "BADC")).toBe(1.5);
    expect(decodePair(0xffff, 0xfffe, "int32", "ABCD")).toBe(-2);
    expect(decodePair(1, 0, "uint32", "ABCD")).toBe(65536);
  });

  it("takes values from a read that covers the series", () => {
    const read = hr(100, [0xffff, 0x3fc0, 0]);
    expect(valueIn(read, series(100, "int16", 1))).toBe(-1);
    expect(valueIn(read, series(100, "uint16", 1))).toBe(65535);
    expect(valueIn(read, series(101, "float32", 1))).toBe(1.5);
    expect(covers(read, series(102, "float32", 1))).toBe(false);
    expect(covers(read, series(99, "uint16", 1))).toBe(false);
    expect(
      valueIn(
        {
          address: 0,
          area: "coil",
          data: { kind: "bits", values: [false, true] },
        },
        { area: "coil", address: 1, type: "bool", order: "ABCD", slot: 1 },
      ),
    ).toBe(1);
  });

  it("keeps the series still read and drops the others", () => {
    const list = [series(100, "uint16", 1), series(105, "uint16", 2)];
    let state = addRead(list, new Map(), hr(100, [7, 0, 0, 0, 0, 9]), 1);
    expect(state.series).toBe(list);
    expect(state.history.get(2)).toEqual([{ t: 1, v: 9 }]);
    // Step 2 now reads 100-102: address 105 leaves the chart.
    state = addRead(state.series, state.history, hr(100, [8, 0, 0]), 2);
    expect(state.series.map((s) => s.address)).toEqual([100]);
    expect(state.history.get(1)).toEqual([
      { t: 1, v: 7 },
      { t: 2, v: 8 },
    ]);
    expect(state.history.has(2)).toBe(false);
    // A failed read leaves a gap.
    state = addRead(state.series, state.history, null, 3);
    expect(state.history.get(1)?.at(-1)?.v).toBeNaN();
  });

  it("tells series apart and names them", () => {
    expect(
      sameSeries(series(1, "uint16", 1), series(1, "uint16", 2, "DCBA")),
    ).toBe(true);
    expect(
      sameSeries(series(1, "float32", 1), series(1, "float32", 2, "CDAB")),
    ).toBe(false);
    expect(seriesName(series(1, "float32", 1, "CDAB"), "1")).toBe(
      "HR 1 · float32 CDAB",
    );
  });
});
