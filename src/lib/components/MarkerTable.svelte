<!--
  Figures between markers A and B, one row per line: the values at A and
  B, their difference, and the minimum, maximum and average between them.
-->
<script lang="ts">
  import { clock, duration } from "#lib/format.ts";
  import { markerStats, type Markers } from "#lib/markers.svelte.ts";
  import { valueLabel as label } from "#lib/trend.ts";
  import type { Line } from "./TrendChart.svelte";

  let { markers, lines }: { markers: Markers; lines: Line[] } = $props();

  const rows = $derived(
    markers.a !== null && markers.b !== null
      ? lines.map((l) => ({
          line: l,
          s: markerStats(l.samples, markers.a!, markers.b!),
        }))
      : [],
  );
</script>

<div class="markers">
  <div class="head">
    <strong>Markers</strong>
    {#if markers.a === null}
      <span class="dim"
        >Click a chart to place marker A, then B. Drag a marker to move it.</span
      >
    {:else if markers.b === null}
      <span class="dim">A at {clock(markers.a)}. Click again to place B.</span>
    {:else}
      <span class="mono"
        >A {clock(markers.a)} · B {clock(markers.b)} · Δt {duration(
          Math.abs(markers.b - markers.a),
        )}</span
      >
    {/if}
    <span class="spacer"></span>
    <button class="btn" disabled={!markers.any} onclick={() => markers.clear()}
      >Clear markers</button
    >
  </div>
  {#if rows.length}
    <table class="data">
      <thead>
        <tr>
          <th>Line</th>
          <th class="num">At A</th>
          <th class="num">At B</th>
          <th class="num">B − A</th>
          <th class="num">Min</th>
          <th class="num">Max</th>
          <th class="num">Average</th>
          <th class="num">Samples</th>
        </tr>
      </thead>
      <tbody>
        {#each rows as { line, s } (line.slot)}
          {@const unit = line.reg.unit ? ` ${line.reg.unit}` : ""}
          <tr>
            <td
              ><span class="swatch" style:background="var(--series-{line.slot})"
              ></span>{line.reg.name}</td
            >
            <td class="num mono">{label(s.a)}{unit}</td>
            <td class="num mono">{label(s.b)}{unit}</td>
            <td class="num mono"
              >{Number.isFinite(s.diff) && s.diff > 0 ? "+" : ""}{label(
                s.diff,
              )}{unit}</td
            >
            <td class="num mono">{label(s.min)}{unit}</td>
            <td class="num mono">{label(s.max)}{unit}</td>
            <td class="num mono">{label(s.avg)}{unit}</td>
            <td class="num mono">{s.count}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .markers {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    font-size: 12px;
  }

  .spacer {
    flex: 1;
  }

  .dim {
    color: var(--text-3);
  }

  .mono {
    font-family: var(--mono);
  }

  table {
    border-collapse: collapse;
    font-size: 12px;
  }

  th {
    text-align: left;
    padding: 3px 10px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-3);
  }

  td {
    padding: 3px 10px;
    border-top: 1px solid var(--border);
    white-space: nowrap;
  }

  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .swatch {
    display: inline-block;
    width: 10px;
    height: 3px;
    margin-right: 6px;
    vertical-align: middle;
    border-radius: 2px;
  }
</style>
