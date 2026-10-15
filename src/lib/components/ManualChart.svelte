<!--
  The Chart tab of manual reads: addresses of step 2's read, each read the
  way the Read registers grid shows it (signed, 32-bit float in a byte
  order…), plotted while the loop runs.
-->
<script lang="ts">
  import { formatAddress } from "#lib/format.ts";
  import {
    chartType,
    seriesName,
    valueIn,
    width,
    type ManualSeries,
  } from "#lib/manual-chart.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { MAX_SERIES } from "#lib/trend.ts";
  import ChartView from "./ChartView.svelte";
  import type { Line } from "./TrendChart.svelte";

  const read = $derived(bench.read);
  const type = $derived(read ? chartType(bench.gridFormat, read.area) : null);

  function name(s: ManualSeries): string {
    return seriesName(
      s,
      formatAddress(s.address, settings.addressFormat, s.area),
    );
  }

  /** Addresses of the read that are not charted yet, as the grid shows
   * them: one per register, or one per pair for 32-bit types. */
  const candidates = $derived.by((): ManualSeries[] => {
    if (!read || !type) return [];
    const step = width(type);
    const out: ManualSeries[] = [];
    for (let i = 0; i + step <= read.data.values.length; i += step) {
      const s: ManualSeries = {
        area: read.area,
        address: read.address + i,
        type,
        order: bench.gridOrder,
        slot: 0,
      };
      if (!bench.manualSeriesOf(s)) out.push(s);
    }
    return out;
  });

  const options = $derived(
    candidates.map((s, i) => {
      const v = read ? valueIn(read, s) : null;
      return { value: i, label: v === null ? name(s) : `${name(s)} = ${v}` };
    }),
  );

  const groups = $derived([
    {
      unit: "",
      lines: bench.manualSeries.map((s): Line => ({
        index: s.slot,
        slot: s.slot,
        reg: { name: name(s), description: "", unit: "" },
        samples: bench.manualHistory.get(s.slot) ?? [],
      })),
    },
  ]);
</script>

<ChartView
  {groups}
  {options}
  addLabel="Add an address to the chart"
  count={bench.manualSeries.length}
  empty={read
    ? `Add up to ${MAX_SERIES} addresses of the read, here or with a right-click on the Read registers grid. They are read as the grid shows them (Show as, Byte order), and recorded while the loop runs.`
    : "Read some registers first: choose a read command in step 2 and press Start."}
  note={type === null && read
    ? "ASCII cannot be charted: choose another Show as in the grid"
    : undefined}
  onadd={(i) => {
    const s = candidates[i];
    if (s) bench.addManualToChart(s);
  }}
  onremove={(slot) => bench.removeManualFromChart(slot)}
  onclear={() => bench.clearManualChart()}
/>
