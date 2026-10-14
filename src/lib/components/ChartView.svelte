<script lang="ts">
  import { bench, type OpenMap } from "$lib/state.svelte";
  import { MAX_SERIES, chartable, groupByUnit } from "$lib/trend";
  import TrendChart, { type Line } from "./TrendChart.svelte";

  let { open }: { open: OpenMap } = $props();

  const windows = [
    { ms: 60_000, label: "Last minute" },
    { ms: 5 * 60_000, label: "Last 5 minutes" },
    { ms: 15 * 60_000, label: "Last 15 minutes" },
    { ms: 60 * 60_000, label: "Last hour" },
    { ms: 0, label: "Everything" },
  ];
  let windowMs = $state(5 * 60_000);

  const registers = $derived(open.map.registers);
  const full = $derived(bench.chartSeries.length >= MAX_SERIES);
  const options = $derived(
    registers
      .map((reg, index) => ({ reg, index }))
      .filter(
        ({ reg, index }) =>
          chartable(reg) && !bench.chartSeries.some((s) => s.index === index),
      ),
  );

  /** The time span every chart shows, ending at the newest sample. */
  const span = $derived.by(() => {
    let first = Infinity;
    let last = -Infinity;
    for (const s of bench.history.values()) {
      if (!s.length) continue;
      first = Math.min(first, s[0].t);
      last = Math.max(last, s[s.length - 1].t);
    }
    if (!Number.isFinite(last)) return null;
    const from = windowMs ? last - windowMs : first;
    return { from, to: Math.max(last, from + 1000) };
  });

  const groups = $derived(
    groupByUnit(bench.chartSeries, registers).map((g) => ({
      unit: g.unit,
      lines: g.series.map((s): Line => ({
        ...s,
        reg: registers[s.index],
        samples: bench.history.get(s.index) ?? [],
      })),
    })),
  );

  function add(e: Event & { currentTarget: HTMLSelectElement }) {
    const value = e.currentTarget.value;
    e.currentTarget.value = "";
    if (value !== "") bench.addToChart(Number(value));
  }
</script>

<div class="toolbar">
  <select
    class="input add"
    aria-label="Add a register to the chart"
    disabled={full || !options.length}
    onchange={add}
  >
    <option value=""
      >{full
        ? `Up to ${MAX_SERIES} registers`
        : "Add a register to the chart…"}</option
    >
    {#each options as { reg, index } (index)}
      <option value={index}
        >{reg.name}{reg.unit ? ` (${reg.unit})` : ""}{reg.group
          ? ` · ${reg.group}`
          : ""}</option
      >
    {/each}
  </select>
  <select class="input" bind:value={windowMs} aria-label="Time span">
    {#each windows as w (w.ms)}<option value={w.ms}>{w.label}</option>{/each}
  </select>
  <span class="spacer"></span>
  <span class="dim">One chart per unit</span>
  <button
    class="btn"
    disabled={!bench.history.size}
    onclick={() => bench.clearChart()}>Clear</button
  >
</div>

{#if !bench.chartSeries.length}
  <p class="empty">
    Add up to {MAX_SERIES} registers to plot their values over time. Values are recorded
    from then on, while the test runs.
  </p>
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
        onremove={(i) => bench.removeFromChart(i)}
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

  .add {
    width: 280px;
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
    overflow: auto;
  }
</style>
