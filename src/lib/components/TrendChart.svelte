<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { clock } from "#lib/format.ts";
  import {
    lastGood,
    lastLabel,
    nearest,
    niceTicks,
    valueLabel as label,
    yRange,
    type Sample,
  } from "#lib/trend.ts";
  import type { Register } from "#lib/types.ts";
  import type { Markers } from "#lib/markers.svelte.ts";
  import {
    panRange,
    selectionRange,
    wheelFactor,
    zoomRange,
    type Range,
  } from "#lib/zoom.ts";

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
    markers,
    bounds,
    onzoom,
  }: {
    unit: string;
    lines: Line[];
    /** Time span shown, in ms since epoch. */
    from: number;
    to: number;
    onremove: (index: number) => void;
    /** Markers A and B, shared by the charts of one view; none: no markers. */
    markers?: Markers;
    /** What the data covers: the limits of zooming and panning. */
    bounds?: Range | null;
    /** Called with the new time span when the chart is zoomed or panned. */
    onzoom?: (range: Range) => void;
  } = $props();

  /** The marker being dragged. */
  let dragging = $state<"a" | "b" | null>(null);

  /** The time at a pointer position, inside the span. */
  function timeAt(e: PointerEvent): number | null {
    const box = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = e.clientX - box.left;
    if (px < M.left || px > M.left + plotW) return null;
    return from + ((px - M.left) / plotW) * span;
  }

  /**
   * A press that is not on a marker: a click places a marker, a drag pans the
   * time axis, and Shift + drag picks a range to zoom into.
   */
  let gesture = $state<{
    startX: number;
    startT: number;
    from: number;
    to: number;
    moved: boolean;
    select: boolean;
  } | null>(null);
  /** The range being picked with Shift + drag, in ms. */
  let selection = $state<{ a: number; b: number } | null>(null);
  /** Pixels a press has to move before it is a drag and not a click. */
  const DRAG_PX = 4;

  /** The time at a pointer position, held inside the plot. */
  function timeClamped(e: PointerEvent): number {
    const box = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = Math.min(Math.max(e.clientX - box.left, M.left), M.left + plotW);
    return from + ((px - M.left) / plotW) * span;
  }

  /** Grabs a marker near the pointer, or starts a click or a drag. */
  function ondown(e: PointerEvent) {
    if (e.button !== 0) return;
    const t = timeAt(e);
    if (t === null) return;
    const box = (e.currentTarget as SVGElement).getBoundingClientRect();
    const px = e.clientX - box.left;
    const near = (m: number | null) => m !== null && Math.abs(x(m) - px) <= 6;
    dragging = markers
      ? near(markers.a)
        ? "a"
        : near(markers.b)
          ? "b"
          : null
      : null;
    if (dragging) {
      (e.currentTarget as SVGElement).setPointerCapture(e.pointerId);
      return;
    }
    if (!markers && !(onzoom && bounds)) return;
    gesture = {
      startX: e.clientX,
      startT: t,
      from,
      to,
      moved: false,
      select: e.shiftKey,
    };
    (e.currentTarget as SVGElement).setPointerCapture(e.pointerId);
  }

  function ondrag(e: PointerEvent) {
    onmove(e);
    if (markers && dragging) {
      const t = timeAt(e);
      if (t !== null) markers.move(dragging, t);
      return;
    }
    if (!gesture) return;
    const dx = e.clientX - gesture.startX;
    if (!gesture.moved && Math.abs(dx) < DRAG_PX) return;
    gesture.moved = true;
    if (gesture.select) {
      selection = { a: gesture.startT, b: timeClamped(e) };
    } else if (onzoom && bounds) {
      const shift = -(dx / plotW) * (gesture.to - gesture.from);
      onzoom(panRange({ from: gesture.from, to: gesture.to }, shift, bounds));
    }
  }

  function onup() {
    const g = gesture;
    gesture = null;
    dragging = null;
    const picked = selection;
    selection = null;
    if (!g) return;
    if (!g.moved) {
      // A click places a marker where it was.
      markers?.place(g.startT);
    } else if (g.select && picked && onzoom && bounds) {
      const r = selectionRange(picked.a, picked.b, bounds);
      if (r) onzoom(r);
    }
  }

  /** The wheel zooms around the pointer. */
  function wheel(node: SVGElement) {
    const handle = (e: WheelEvent) => {
      if (!onzoom || !bounds) return;
      e.preventDefault();
      const px = e.clientX - node.getBoundingClientRect().left;
      const at = from + Math.min(1, Math.max(0, (px - M.left) / plotW)) * span;
      onzoom(zoomRange({ from, to }, at, wheelFactor(e.deltaY), bounds));
    };
    node.addEventListener("wheel", handle, { passive: false });
    return { destroy: () => node.removeEventListener("wheel", handle) };
  }

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

  /**
   * Where reads failed or gave no value, per line: one mark per pixel
   * column, failures first, along the bottom of the plot.
   */
  const marks = $derived(
    visible.map((samples) => {
      const out = new Map<number, "failed" | "unavailable">();
      for (const s of samples) {
        if (!s.q || s.t < from || s.t > to) continue;
        const px = Math.round(x(s.t));
        if (out.get(px) !== "failed") out.set(px, s.q);
      }
      return [...out];
    }),
  );

  /** What a hovered sample says when it has no value. */
  function noValue(samples: Sample[], s: Sample): string {
    const why =
      s.q === "failed"
        ? t("trendchart.read_failed")
        : s.q === "unavailable"
          ? t("trendchart.no_value")
          : "—";
    const held = lastGood(samples, samples.indexOf(s));
    return held
      ? t("trendchart.why_last_value_held", { why, value: label(held.v) })
      : why;
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
    <span class="unit">{unit || t("trendchart.no_unit")}</span>
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
            title={t("trendchart.remove_name_from_the", { name: l.reg.name })}
            aria-label={t("trendchart.remove_name_from_the", {
              name: l.reg.name,
            })}
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
        aria-label={t("trendchart.names_over_time", {
          names: lines.map((l) => l.reg.name).join(", "),
        })}
        onpointermove={ondrag}
        onpointerdown={ondown}
        onpointerup={onup}
        onpointercancel={onup}
        onpointerleave={() => (hoverX = null)}
        use:wheel
        class:markable={!!markers}
        class:zoomable={!!(onzoom && bounds)}
        class:panning={!!gesture?.moved && !gesture.select}
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
        <!-- Failed reads (×) and values the device did not give (|), so a
             gap is never mistaken for a value -->
        {#each lines as l, k (l.index)}
          {#each marks[k] as [px, q] (px)}
            {@const base = M.top + plotH - 3 - k * 2}
            {#if q === "failed"}
              <path
                class="mark failed"
                d="M{px - 2.5},{base - 2.5}L{px + 2.5},{base + 2.5}M{px +
                  2.5},{base - 2.5}L{px - 2.5},{base + 2.5}"
              />
            {:else}
              <path class="mark unavailable" d="M{px},{base - 3}V{base + 3}" />
            {/if}
          {/each}
        {/each}

        {#if selection}
          {@const x0 = x(Math.min(selection.a, selection.b))}
          <rect
            class="selection"
            x={x0}
            y={M.top}
            width={Math.abs(x(selection.b) - x(selection.a))}
            height={plotH}
          />
        {/if}
        {#if markers}
          {#each [["a", markers.a], ["b", markers.b]] as [name, t] (name)}
            {#if t !== null && typeof t === "number" && t >= from && t <= to}
              <line
                class="marker"
                x1={x(t)}
                x2={x(t)}
                y1={M.top}
                y2={M.top + plotH}
              />
              <text class="marker-label" x={x(t) + 4} y={M.top + 10}
                >{String(name).toUpperCase()}</text
              >
            {/if}
          {/each}
        {/if}
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
              {#if p.sample && !Number.isFinite(p.sample.v)}
                <span class="mono none"
                  >{noValue(visible[lines.indexOf(p.line)], p.sample)}</span
                >
              {:else}
                <span class="mono"
                  >{p.sample ? label(p.sample.v) : "—"}{p.line.reg.unit
                    ? ` ${p.line.reg.unit}`
                    : ""}</span
                >
              {/if}
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

  svg.zoomable {
    cursor: grab;
  }

  svg.markable {
    cursor: crosshair;
  }

  svg.panning {
    cursor: grabbing;
  }

  .selection {
    fill: rgb(var(--fg-rgb) / 0.1);
    stroke: var(--text-3);
    stroke-width: 1;
    stroke-dasharray: 4 3;
  }

  .marker {
    stroke: var(--text);
    stroke-width: 1.5;
    stroke-dasharray: 4 3;
  }

  .marker-label {
    fill: var(--text);
    font-size: 11px;
    font-weight: 600;
  }

  .mark {
    fill: none;
    stroke-width: 1.5;
    stroke-linecap: round;
  }

  .mark.failed {
    stroke: var(--bad);
  }

  .mark.unavailable {
    stroke: var(--text-3);
  }

  .tip .none {
    color: var(--text-2);
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
