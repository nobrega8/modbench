<!-- The Chart tab with a device map: its registers over time. -->
<script lang="ts">
  import { bench, type OpenMap } from "#lib/state.svelte.ts";
  import { MAX_SERIES, chartable, groupByUnit } from "#lib/trend.ts";
  import ChartView from "./ChartView.svelte";
  import type { Line } from "./TrendChart.svelte";

  let { open }: { open: OpenMap } = $props();

  const registers = $derived(open.map.registers);

  const options = $derived(
    registers
      .map((reg, index) => ({ reg, index }))
      .filter(
        ({ reg, index }) =>
          chartable(reg) && !bench.chartSeries.some((s) => s.index === index),
      )
      .map(({ reg, index }) => ({
        value: index,
        label: `${reg.name}${reg.unit ? ` (${reg.unit})` : ""}${reg.group ? ` · ${reg.group}` : ""}`,
      })),
  );

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
</script>

<ChartView
  {groups}
  {options}
  addLabel="Add a register to the chart"
  count={bench.chartSeries.length}
  empty="Add up to {MAX_SERIES} registers to plot their values over time. Values are recorded from then on, while the test runs."
  note="One chart per unit"
  onadd={(i) => bench.addToChart(i)}
  onremove={(i) => bench.removeFromChart(i)}
  onclear={() => bench.clearChart()}
/>
