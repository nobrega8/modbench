<!--
  The Chart tab: a menu to add a line, the time span, and one chart per
  unit. MapChart and ManualChart fill it with map registers or with
  addresses of a manual read.
-->
<script lang="ts">
  import { bench } from "#lib/state.svelte.ts";
  import Select from "./Select.svelte";
  import { MAX_SERIES } from "#lib/trend.ts";
  import TrendChart, { type Line } from "./TrendChart.svelte";

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

  const windows = [
    { ms: 60_000, label: "Last minute" },
    { ms: 5 * 60_000, label: "Last 5 minutes" },
    { ms: 15 * 60_000, label: "Last 15 minutes" },
    { ms: 60 * 60_000, label: "Last hour" },
    { ms: 0, label: "Everything" },
  ];
  let windowMs = $state(5 * 60_000);
  /** The add menu; it goes back to its prompt after a pick. */
  let pick = $state(-1);

  const full = $derived(count >= MAX_SERIES);

  /** The time span every chart shows, ending at the newest sample. */
  const span = $derived.by(() => {
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
    label="Time span"
    options={windows.map((w) => ({ value: w.ms, label: w.label }))}
  />
  <span class="spacer"></span>
  {#if note}<span class="dim">{note}</span>{/if}
  <button class="btn" disabled={!span} onclick={onclear}>Clear</button>
</div>

{#if !count}
  <p class="empty">{empty}</p>
{:else if !span}
  <p class="empty">
    {bench.running
      ? "Waiting for the first values…"
      : "Press Start to record values."}
  </p>
{:else}
  <div class="charts">
    {#each groups as g (g.unit)}
      <TrendChart
        unit={g.unit}
        lines={g.lines}
        from={span.from}
        to={span.to}
        {onremove}
      />
    {/each}
  </div>
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
