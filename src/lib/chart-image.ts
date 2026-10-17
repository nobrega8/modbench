// Export of the charts on screen as one image (Chart tab, Trend player): an
// SVG with the legend, the unit and the time span of every chart, and a PNG
// drawn from it. Colours are written out, not CSS variables, so the SVG
// looks the same in any program that opens it.

import { clock, duration } from "./format";

export interface Palette {
  background: string;
  text: string;
  text2: string;
  text3: string;
  grid: string;
  axis: string;
  /** Line colours, slot 1 first. */
  series: string[];
  /** Marks of failed reads; the second text colour when missing. */
  bad?: string;
}

export interface LegendItem {
  name: string;
  /** The newest value, as the legend on screen shows it. */
  value: string;
  slot: number;
}

export interface ChartPart {
  unit: string;
  legend: LegendItem[];
  /** The plot's SVG content, as made by `plotMarkup`. */
  plot: string;
  width: number;
  height: number;
}

/** Width of `text` at `size` px, in the UI or the mono font. */
export type Measure = (text: string, size: number, mono: boolean) => number;

// Fonts every system has: an image cannot load the app's own Inter.
export const FONT = 'system-ui, -apple-system, "Segoe UI", sans-serif';
export const MONO =
  'ui-monospace, "SF Mono", "Cascadia Mono", Menlo, Consolas, monospace';

const PAD = 16;
const ROW = 20;
const GAP = 18;

export function escapeXml(text: string): string {
  return text.replace(
    /[<>&"']/g,
    (c) =>
      ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;", "'": "&#39;" })[
        c
      ]!,
  );
}

/** The time span of the charts, for the top of the image. */
export function spanText(from: number, to: number): string {
  const d = new Date(from);
  const p = (n: number) => n.toString().padStart(2, "0");
  const day = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  return `${day} ${clock(from).slice(0, 8)} – ${clock(to).slice(0, 8)} (${duration(to - from)})`;
}

/**
 * Presentation attributes for an element of the plot, by its classes (the
 * styles TrendChart gives them); null for the hover marks, which are left
 * out.
 */
export function classAttrs(
  classes: string[],
  p: Palette,
): Record<string, string> | null {
  const has = (c: string) => classes.includes(c);
  if (has("cross") || has("dot")) return null;
  if (has("grid")) return { stroke: p.grid, "stroke-width": "1" };
  if (has("axis")) return { stroke: p.axis, "stroke-width": "1" };
  if (has("tick"))
    return { fill: p.text3, "font-family": MONO, "font-size": "11" };
  if (has("marker"))
    return { stroke: p.text, "stroke-width": "1.5", "stroke-dasharray": "4 3" };
  if (has("marker-label"))
    return {
      fill: p.text,
      "font-family": FONT,
      "font-size": "11",
      "font-weight": "600",
    };
  if (has("mark"))
    return {
      fill: "none",
      stroke: has("failed") ? (p.bad ?? p.text2) : p.text3,
      "stroke-width": "1.5",
      "stroke-linecap": "round",
    };
  if (has("line"))
    return {
      fill: "none",
      "stroke-width": "2",
      "stroke-linejoin": "round",
      "stroke-linecap": "round",
    };
  return {};
}

/** A `var(--series-N)` colour as the palette has it; anything else as is. */
export function seriesColor(value: string, p: Palette): string {
  const m = /^var\(--series-(\d+)\)$/.exec(value.trim());
  return m ? (p.series[Number(m[1]) - 1] ?? value) : value;
}

/** The content of a plot on screen, with its colours written out. */
export function plotMarkup(svg: SVGSVGElement, p: Palette): string {
  const copy = svg.cloneNode(true) as SVGSVGElement;
  for (const el of Array.from(copy.querySelectorAll("*"))) {
    const attrs = classAttrs(Array.from(el.classList), p);
    if (!attrs) {
      el.remove();
      continue;
    }
    for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
    el.removeAttribute("class");
    for (const k of ["stroke", "fill"]) {
      const v = el.getAttribute(k);
      if (v) el.setAttribute(k, seriesColor(v, p));
    }
  }
  return copy.innerHTML;
}

/** One image of all the charts, one under the other, each with its legend. */
export function composeSvg(
  charts: ChartPart[],
  title: string,
  p: Palette,
  measure: Measure,
): { svg: string; width: number; height: number } {
  const width = PAD * 2 + Math.max(0, ...charts.map((c) => c.width));
  const right = width - PAD;
  const out: string[] = [];
  const text = (
    x: number,
    y: number,
    s: string,
    size: number,
    fill: string,
    mono = false,
  ) =>
    out.push(
      `<text x="${x}" y="${y}" font-family="${escapeXml(mono ? MONO : FONT)}" font-size="${size}" fill="${fill}">${escapeXml(s)}</text>`,
    );

  let y = PAD;
  text(PAD, y + 13, title, 12, p.text2);
  y += ROW + 8;

  for (const c of charts) {
    // Legend: the unit, then each line, wrapped to the image's width.
    const unit = (c.unit || "No unit").toUpperCase();
    text(PAD, y + 14, unit, 11, p.text3);
    let x = PAD + Math.max(46, measure(unit, 11, false)) + 14;
    for (const item of c.legend) {
      const nameW = measure(item.name, 13, false);
      const valueW = measure(item.value, 12, true);
      const w = 12 + 6 + nameW + 6 + valueW;
      if (x + w > right && x > PAD) {
        x = PAD;
        y += ROW;
      }
      const color = p.series[item.slot - 1] ?? p.text;
      out.push(
        `<rect x="${x}" y="${y + 9}" width="12" height="3" rx="1.5" fill="${color}"/>`,
      );
      text(x + 18, y + 14, item.name, 13, p.text);
      text(x + 24 + nameW, y + 14, item.value, 12, p.text2, true);
      x += w + 16;
    }
    y += ROW + 6;

    out.push(
      `<svg x="${PAD}" y="${y}" width="${c.width}" height="${c.height}" overflow="hidden">${c.plot}</svg>`,
    );
    y += c.height + GAP;
  }
  const height = Math.ceil(y - GAP + PAD);

  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">` +
    `<rect width="100%" height="100%" fill="${p.background}"/>` +
    out.join("") +
    `</svg>`;
  return { svg, width, height };
}
