<!-- The Chart tab with a device map: its registers over time. -->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
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
  addLabel={t("mapchart.add_a_register_to")}
  count={bench.chartSeries.length}
  empty={t("mapchart.add_up_to_max", { max: MAX_SERIES })}
  note={t("mapchart.one_chart_per_unit")}
  onadd={(i) => bench.addToChart(i)}
  onremove={(i) => bench.removeFromChart(i)}
  onclear={() => bench.clearChart()}
/>
