<!--
  The Chart tab: a menu to add a line, the time span, and one chart per
  unit. MapChart and ManualChart fill it with map registers or with
  addresses of a manual read.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import Select from "./Select.svelte";
  import { MAX_SERIES } from "#lib/trend.ts";
  import TrendChart, { type Line } from "./TrendChart.svelte";
  import ChartExport from "./ChartExport.svelte";
  import MarkerTable from "./MarkerTable.svelte";
  import { Markers } from "#lib/markers.svelte.ts";
  import { boundsOf, markersRange, type Range } from "#lib/zoom.ts";
  import Icon from "./Icon.svelte";

  let {
    groups,
    options,
    addLabel,
    count,
    empty,
    note,
    onadd,
    onremove,
    onclear,
  }: {
    /** Lines by unit: one chart each. */
    groups: { unit: string; lines: Line[] }[];
    /** What the add menu offers. */
    options: { value: number; label: string }[];
    addLabel: string;
    /** Lines on the chart now. */
    count: number;
    /** Shown while there is nothing on the chart. */
    empty: string;
    note?: string;
    onadd: (value: number) => void;
    onremove: (index: number) => void;
    onclear: () => void;
  } = $props();

  const windows = $derived([
    { ms: 60_000, label: t("chartview.last_minute") },
    { ms: 5 * 60_000, label: t("chartview.last_5_minutes") },
    { ms: 15 * 60_000, label: t("chartview.last_15_minutes") },
    { ms: 60 * 60_000, label: t("chartview.last_hour") },
    { ms: 0, label: t("chartview.everything") },
  ]);
  let windowMs = $state(5 * 60_000);
  /** The add menu; it goes back to its prompt after a pick. */
  let pick = $state(-1);
  /** Markers A and B of this view's charts. */
  const markers = new Markers();
  let charts: HTMLDivElement | undefined = $state();

  const full = $derived(count >= MAX_SERIES);

  /** The span picked by zooming or panning, or null: the time span above. */
  let zoom = $state.raw<Range | null>(null);

  /** The time span every chart shows, ending at the newest sample. */
  const baseSpan = $derived.by(() => {
    let first = Infinity;
    let last = -Infinity;
    for (const g of groups) {
      for (const { samples: s } of g.lines) {
        if (!s.length) continue;
        first = Math.min(first, s[0].t);
        last = Math.max(last, s[s.length - 1].t);
      }
    }
    if (!Number.isFinite(last)) return null;
    const from = windowMs ? last - windowMs : first;
    return { from, to: Math.max(last, from + 1000) };
  });

  /** What the charts show: the zoomed span, or the chosen time span. */
  const span = $derived(baseSpan && zoom ? zoom : baseSpan);

  /** What the data covers: how far the charts can zoom and pan. */
  const bounds = $derived(
    boundsOf(
      groups.flatMap((g) =>
        g.lines.flatMap(({ samples: s }) =>
          s.length ? [s[0].t, s[s.length - 1].t] : [],
        ),
      ),
    ),
  );

  function zoomToMarkers() {
    if (markers.a === null || markers.b === null || !bounds) return;
    const r = markersRange(markers.a, markers.b, bounds);
    if (r) zoom = r;
  }

  function add(value: number) {
    onadd(value);
    pick = -1;
  }
</script>

<div class="toolbar">
  <Select
    bind:value={pick}
    width="280px"
    label={addLabel}
    placeholder={full ? `Up to ${MAX_SERIES} lines` : `${addLabel}…`}
    disabled={full || !options.length}
    {options}
    onchange={add}
  />
  <Select
    bind:value={windowMs}
    label={t("chartview.time_span")}
    options={windows.map((w) => ({ value: w.ms, label: w.label }))}
    onchange={() => (zoom = null)}
  />
  <button
    class="btn icon"
    title={`${t("chartview.zoom_reset")} (${t("chartview.zoom_help")})`}
    aria-label={t("chartview.zoom_reset")}
    disabled={!zoom}
    onclick={() => (zoom = null)}><Icon name="zoomreset" /></button
  >
  <button
    class="btn icon"
    title={t("chartview.zoom_markers")}
    aria-label={t("chartview.zoom_markers")}
    disabled={markers.a === null || markers.b === null || !bounds}
    onclick={zoomToMarkers}><Icon name="zoomin" /></button
  >
  <span class="spacer"></span>
  {#if note}<span class="dim">{note}</span>{/if}
  <ChartExport
    {charts}
    {groups}
    from={span?.from ?? 0}
    to={span?.to ?? 0}
    name="chart"
    disabled={!span || !count}
  />
  <button
    class="btn"
    disabled={!span}
    onclick={() => {
      zoom = null;
      onclear();
    }}>{t("chartview.clear")}</button
  >
</div>

{#if !count}
  <p class="empty">{empty}</p>
{:else if !span}
  <p class="empty">
    {bench.running
      ? t("chartview.waiting_for_the_first")
      : t("chartview.press_start_to_record")}
  </p>
{:else}
  <div class="charts" bind:this={charts}>
    {#each groups as g (g.unit)}
      <TrendChart
        unit={g.unit}
        lines={g.lines}
        from={span.from}
        to={span.to}
        {onremove}
        {markers}
        {bounds}
        onzoom={(r) => (zoom = r)}
      />
    {/each}
  </div>
  <MarkerTable {markers} lines={groups.flatMap((g) => g.lines)} />
{/if}

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding-bottom: 10px;
  }

  .spacer {
    flex: 1;
  }

  .dim {
    color: var(--text-3);
    font-size: 12px;
  }

  .empty {
    color: var(--text-2);
    margin: 8px 0;
  }

  .charts {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 18px;
    /* Two charts can overflow. A scrollbar that comes and goes changes the
       width, the plots redraw a frame late and briefly overflow sideways, and
       the two scrollbars chase each other: the charts shake. Keep the gutter
       and never scroll sideways. */
    overflow: hidden auto;
    scrollbar-gutter: stable;
  }
</style>
