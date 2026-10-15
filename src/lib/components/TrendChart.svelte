<script lang="ts">
  import { clock } from "#lib/format.ts";
  import {
    lastLabel,
    nearest,
    niceTicks,
    valueLabel as label,
    yRange,
    type Sample,
  } from "#lib/trend.ts";
  import type { Register } from "#lib/types.ts";

  export interface Line {
    index: number;
    slot: number;
    /** What the line shows: a map register, or a name for an address. */
    reg: Pick<Register, "name" | "description" | "unit">;
    samples: Sample[];
  }

  let {
    unit,
    lines,
    from,
    to,
    onremove,
  }: {
    unit: string;
    lines: Line[];
    /** Time span shown, in ms since epoch. */
    from: number;
    to: number;
    onremove: (index: number) => void;
  } = $props();

  const M = { top: 10, right: 14, bottom: 24, left: 58 };
  let width = $state(0);
  let height = $state(0);
  let hoverX = $state<number | null>(null);
  /** Unique per chart: units such as "%" cannot go in an id. */
  const clipId = `trend-clip-${Math.random().toString(36).slice(2)}`;

  const plotW = $derived(Math.max(10, width - M.left - M.right));
  const plotH = $derived(Math.max(10, height - M.top - M.bottom));

  /** Samples inside the time span, per line. */
  const visible = $derived(
    lines.map((l) => {
      const s = l.samples;
      let i = 0;
      while (i < s.length && s[i].t < from) i++;
      // Keep one point before the span so the line enters from the edge.
      return s.slice(Math.max(0, i - 1));
    }),
  );

  const range = $derived(yRange(visible.flat().map((s) => s.v)));
  const yTicks = $derived(range ? niceTicks(range[0], range[1]) : []);
  const span = $derived(Math.max(1, to - from));

  const x = (t: number) => M.left + ((t - from) / span) * plotW;
  const y = (v: number) =>
    range
      ? M.top + plotH - ((v - range[0]) / (range[1] - range[0] || 1)) * plotH
      : M.top + plotH / 2;

  const STEPS = [
    1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 14400,
  ].map((s) => s * 1000);
  const xTicks = $derived.by(() => {
    const most = Math.max(2, Math.floor(plotW / 90));
    const step = STEPS.find((s) => span / s <= most) ?? STEPS.at(-1)!;
    const out: number[] = [];
    for (let t = Math.ceil(from / step) * step; t <= to; t += step) out.push(t);
    return out;
  });

  /**
   * SVG path of one line. Gaps (NaN) break it. When there are more points
   * than pixels, each pixel column keeps its first, lowest, highest and last
   * point, so spikes stay visible.
   */
  function path(samples: Sample[]): string {
    const cols = new Map<number, Sample[]>();
    const dense = samples.length > plotW * 2;
    let d = "";
    let pen = false;
    const emit = (s: Sample) => {
      if (!Number.isFinite(s.v)) {
        pen = false;
        return;
      }
      d += `${pen ? "L" : "M"}${x(s.t).toFixed(1)},${y(s.v).toFixed(1)}`;
      pen = true;
    };
    if (!dense) {
      samples.forEach(emit);
      return d;
    }
    for (const s of samples) {
      const c = Math.round(x(s.t));
      (cols.get(c) ?? cols.set(c, []).get(c)!).push(s);
    }
    for (const col of cols.values()) {
      const finite = col.filter((s) => Number.isFinite(s.v));
      if (finite.length < col.length) emit({ t: col[0].t, v: NaN });
      if (!finite.length) continue;
      const lo = finite.reduce((a, b) => (b.v < a.v ? b : a));
      const hi = finite.reduce((a, b) => (b.v > a.v ? b : a));
      const keep = [finite[0], lo, hi, finite.at(-1)!].sort(
        (a, b) => a.t - b.t,
      );
      keep.forEach(emit);
    }
    return d;
  }

  const hover = $derived.by(() => {
    if (hoverX === null) return null;
    const t = from + ((hoverX - M.left) / plotW) * span;
    if (t < from || t > to) return null;
    const points = lines.map((l, k) => {
      const s = nearest(visible[k], t);
      return { line: l, sample: s && s.t >= from ? s : null };
    });
    const first = points.find((p) => p.sample)?.sample;
    if (!first) return null;
    return { t: first.t, points };
  });

  function onmove(e: PointerEvent) {
    const box = (e.currentTarget as SVGElement).getBoundingClientRect();
    hoverX = e.clientX - box.left;
  }
</script>

<div class="chart">
  <div class="head">
    <span class="unit">{unit || "No unit"}</span>
    <ul class="legend">
      {#each lines as l (l.index)}
        <li>
          <span class="swatch" style:background="var(--series-{l.slot})"></span>
          <span class="name" title={l.reg.description || undefined}
            >{l.reg.name}</span
          >
          <span class="last mono">{lastLabel(l.samples)}</span>
          <button
            class="remove"
            title="Remove {l.reg.name} from the chart"
            aria-label="Remove {l.reg.name} from the chart"
            onclick={() => onremove(l.index)}>×</button
          >
        </li>
      {/each}
    </ul>
  </div>

  <div class="plot" bind:clientWidth={width} bind:clientHeight={height}>
    {#if width > 0 && height > 0}
      <svg
        data-trend-plot
        {width}
        {height}
        role="img"
        aria-label="{lines.map((l) => l.reg.name).join(', ')} over time"
        onpointermove={onmove}
        onpointerleave={() => (hoverX = null)}
      >
        {#each yTicks as v (v)}
          <line
            class="grid"
            x1={M.left}
            x2={M.left + plotW}
            y1={y(v)}
            y2={y(v)}
          />
          <text
            class="tick"
            x={M.left - 8}
            y={y(v)}
            text-anchor="end"
            dy="0.32em">{label(v)}</text
          >
        {/each}
        {#each xTicks as t (t)}
          <text
            class="tick"
            x={x(t)}
            y={M.top + plotH + 16}
            text-anchor="middle">{clock(t).slice(0, 8)}</text
          >
        {/each}
        <line
          class="axis"
          x1={M.left}
          x2={M.left + plotW}
          y1={M.top + plotH}
          y2={M.top + plotH}
        />

        <clipPath id={clipId}>
          <rect x={M.left} y={M.top - 2} width={plotW} height={plotH + 4} />
        </clipPath>
        <g clip-path="url(#{clipId})">
          {#each lines as l, k (l.index)}
            <path
              class="line"
              d={path(visible[k])}
              stroke="var(--series-{l.slot})"
            />
          {/each}
        </g>

        {#if hover}
          <line
            class="cross"
            x1={x(hover.t)}
            x2={x(hover.t)}
            y1={M.top}
            y2={M.top + plotH}
          />
          {#each hover.points as p (p.line.index)}
            {#if p.sample && Number.isFinite(p.sample.v)}
              <circle
                class="dot"
                cx={x(p.sample.t)}
                cy={y(p.sample.v)}
                r="4"
                fill="var(--series-{p.line.slot})"
              />
            {/if}
          {/each}
        {/if}
      </svg>
      {#if hover}
        <div
          class="tip"
          class:left={x(hover.t) > width * 0.6}
          style:left="{x(hover.t)}px"
        >
          <span class="time mono">{clock(hover.t)}</span>
          {#each hover.points as p (p.line.index)}
            <span class="row">
              <span
                class="swatch"
                style:background="var(--series-{p.line.slot})"
              ></span>
              <span class="name">{p.line.reg.name}</span>
              <span class="mono"
                >{p.sample ? label(p.sample.v) : "—"}{p.line.reg.unit
                  ? ` ${p.line.reg.unit}`
                  : ""}</span
              >
            </span>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  .chart {
    flex: 1;
    min-height: 170px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .head {
    display: flex;
    align-items: baseline;
    gap: 14px;
    flex-wrap: wrap;
  }

  .unit {
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    min-width: 46px;
  }

  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
  }

  .legend li {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .swatch {
    flex: none;
    width: 12px;
    height: 3px;
    border-radius: 2px;
  }

  .name {
    color: var(--text);
  }

  .last {
    color: var(--text-2);
  }

  .mono {
    font-family: var(--mono);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .remove {
    padding: 0 4px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--text-3);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
  }

  .remove:hover {
    color: var(--text);
    background: var(--field);
  }

  .plot {
    position: relative;
    flex: 1;
    min-height: 0;
  }

  svg {
    position: absolute;
    inset: 0;
    display: block;
    touch-action: none;
  }

  .grid {
    stroke: var(--border);
    stroke-width: 1;
  }

  .axis {
    stroke: var(--border-strong);
    stroke-width: 1;
  }

  .tick {
    fill: var(--text-3);
    font-family: var(--mono);
    font-size: 11px;
  }

  .line {
    fill: none;
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
  }

  .cross {
    stroke: var(--text-3);
    stroke-width: 1;
    stroke-dasharray: 3 3;
  }

  /* A ring in the surface colour keeps overlapping dots apart */
  .dot {
    stroke: var(--card);
    stroke-width: 2;
  }

  .tip {
    position: absolute;
    top: 6px;
    transform: translateX(12px);
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 8px 10px;
    border-radius: 9px;
    background: var(--card);
    box-shadow: var(--e3);
    font-size: 12px;
    pointer-events: none;
    white-space: nowrap;
    z-index: 2;
  }

  .tip.left {
    transform: translateX(calc(-100% - 12px));
  }

  .tip .time {
    color: var(--text-3);
  }

  .tip .row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .tip .row .mono {
    margin-left: auto;
    padding-left: 12px;
    color: var(--text);
  }
</style>
