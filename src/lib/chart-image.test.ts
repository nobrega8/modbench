import { describe, expect, it } from "vitest";
import {
  classAttrs,
  composeSvg,
  escapeXml,
  seriesColor,
  spanText,
  type ChartPart,
  type Palette,
} from "./chart-image";
import { duration } from "./format";
import { lastLabel, valueLabel } from "./trend";

const palette: Palette = {
  background: "rgb(255 255 255)",
  text: "#111",
  text2: "#222",
  text3: "#333",
  grid: "#eee",
  axis: "#ddd",
  series: ["#1565c0", "#13743a"],
};

/** Every character 7 px wide. */
const measure = (text: string) => text.length * 7;

function chart(unit: string, names: string[], width = 400): ChartPart {
  return {
    unit,
    legend: names.map((name, i) => ({ name, value: "1.5", slot: i + 1 })),
    plot: `<path d="M0,0L10,10" stroke="#1565c0"/>`,
    width,
    height: 200,
  };
}

describe("chart image", () => {
  it("escapes text for XML", () => {
    expect(escapeXml(`a<b & "c" 'd'>`)).toBe(
      "a&lt;b &amp; &quot;c&quot; &#39;d&#39;&gt;",
    );
  });

  it("writes the time span with its date and length", () => {
    const from = new Date(2026, 9, 15, 14, 2, 3).getTime();
    expect(spanText(from, from + 5 * 60_000)).toBe(
      "2026-10-15 14:02:03 – 14:07:03 (5 min 0 s)",
    );
  });

  it("styles the plot by class and leaves out the hover marks", () => {
    expect(classAttrs(["grid", "svelte-x"], palette)).toEqual({
      stroke: "#eee",
      "stroke-width": "1",
    });
    expect(classAttrs(["tick"], palette)?.fill).toBe("#333");
    expect(classAttrs(["line"], palette)?.fill).toBe("none");
    expect(classAttrs(["cross"], palette)).toBeNull();
    expect(classAttrs(["dot"], palette)).toBeNull();
    expect(classAttrs([], palette)).toEqual({});
  });

  it("writes out the series colours", () => {
    expect(seriesColor("var(--series-2)", palette)).toBe("#13743a");
    expect(seriesColor("var(--series-9)", palette)).toBe("var(--series-9)");
    expect(seriesColor("#abc", palette)).toBe("#abc");
  });

  it("puts every chart under the title, each with its legend", () => {
    const { svg, width, height } = composeSvg(
      [chart("V", ["U1", "U2"]), chart("", ["Mode"])],
      "span",
      palette,
      measure,
    );
    expect(width).toBe(432);
    // Padding, title, two legends and plots, the gap between, padding.
    expect(height).toBe(16 + 28 + 2 * (26 + 200) + 18 + 16);
    expect(svg).toContain(`width="432" height="${height}"`);
    expect(svg).toContain(`fill="rgb(255 255 255)"`);
    expect(svg).toContain(">span</text>");
    expect(svg).toContain(">V</text>");
    expect(svg).toContain(">NO UNIT</text>");
    expect(svg).toContain(">Mode</text>");
    expect(svg).toContain(`fill="#13743a"`);
    expect(svg.match(/<path /g)).toHaveLength(2);
    expect(svg).not.toContain("var(");
  });

  it("wraps a long legend onto more rows", () => {
    const names = Array.from({ length: 8 }, (_, i) => `Register ${i + 1}`);
    const one = composeSvg([chart("V", ["A"], 300)], "", palette, measure);
    const many = composeSvg([chart("V", names, 300)], "", palette, measure);
    expect(many.height - one.height).toBeGreaterThanOrEqual(3 * 20);
  });

  it("escapes names in the legend", () => {
    const { svg } = composeSvg([chart("°C", ["T<1>"])], "", palette, measure);
    expect(svg).toContain(">T&lt;1&gt;</text>");
  });
});

describe("labels", () => {
  it("formats a length of time", () => {
    expect(duration(42_000)).toBe("42 s");
    expect(duration(65_000)).toBe("1 min 5 s");
    expect(duration(3_720_000)).toBe("1 h 2 min");
  });

  it("formats values for the axis and the legend", () => {
    expect(valueLabel(1.23456789)).toBe("1.23457");
    expect(valueLabel(2e7)).toBe("2.00e+7");
    expect(valueLabel(NaN)).toBe("—");
    expect(lastLabel([])).toBe("—");
    expect(lastLabel([{ t: 0, v: 3 }])).toBe("3");
  });
});
