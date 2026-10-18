<script module lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import type { Recording } from "#lib/playback.ts";

  // Kept while other tools are shown.
  let recording = $state.raw<Recording | null>(null);
  let fileName = $state("");
  /** Column indexes on the chart. */
  let shown = $state.raw<number[]>([]);
  let head = $state(0);
  let speed = $state(1);
  let windowMs = $state(5 * 60_000);
</script>

<script lang="ts">
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { parseRecording, upTo } from "#lib/playback.ts";
  import { clock, duration } from "#lib/format.ts";
  import { MAX_SERIES } from "#lib/trend.ts";
  import type { Register } from "#lib/types.ts";
  import Select from "../Select.svelte";
  import TrendChart, { type Line } from "../TrendChart.svelte";
  import ChartExport from "../ChartExport.svelte";
  import MarkerTable from "../MarkerTable.svelte";
  import { Markers } from "#lib/markers.svelte.ts";

  const speeds = [1, 2, 4, 8, 16, 60];
  const windows = $derived([
    { ms: 60_000, label: t("trendplayertool.1_minute") },
    { ms: 5 * 60_000, label: t("trendplayertool.5_minutes") },
    { ms: 15 * 60_000, label: t("trendplayertool.15_minutes") },
    { ms: 60 * 60_000, label: t("trendplayertool.1_hour") },
    { ms: 0, label: t("chartview.everything") },
  ]);

  let playing = $state(false);
  let problem = $state<string | null>(null);
  /** Markers A and B of this view's charts. */
  const markers = new Markers();
  let charts: HTMLDivElement | undefined = $state();

  /** Registers made up from the column headers, for the chart. */
  const registers = $derived(
    (recording?.columns ?? []).map(
      (c) =>
        ({
          name: c.name,
          unit: c.unit,
          description: "",
        }) as Register,
    ),
  );

  const groups = $derived.by(() => {
    const rec = recording;
    if (!rec) return [];
    const out: { unit: string; lines: Line[] }[] = [];
    shown.forEach((index, k) => {
      const c = rec.columns[index];
      let g = out.find((x) => x.unit === c.unit);
      if (!g) out.push((g = { unit: c.unit, lines: [] }));
      g.lines.push({
        index,
        slot: k + 1,
        reg: registers[index],
        samples: upTo(c.samples, head),
      });
    });
    return out;
  });

  const span = $derived.by(() => {
    if (!recording) return { from: 0, to: 1 };
    const from = windowMs
      ? Math.max(recording.start, head - windowMs)
      : recording.start;
    const to = windowMs ? Math.max(head, from + 1000) : recording.end;
    return { from, to: Math.max(to, from + 1000) };
  });

  async function load() {
    problem = null;
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "CSV recording", extensions: ["csv"] }],
    });
    if (typeof path !== "string") return;
    try {
      const rec = parseRecording(await api.readText(path));
      if ("error" in rec) {
        problem = rec.error;
        return;
      }
      playing = false;
      recording = rec;
      fileName = path.split(/[\\/]/).pop() ?? path;
      // Start with the columns that have numbers, up to the chart's limit.
      shown = rec.columns
        .map((c, i) => ({ c, i }))
        .filter(({ c }) => c.samples.some((s) => Number.isFinite(s.v)))
        .slice(0, MAX_SERIES)
        .map(({ i }) => i);
      head = rec.end;
    } catch (e) {
      problem = errorText(e);
    }
  }

  function toggle(index: number) {
    shown = shown.includes(index)
      ? shown.filter((i) => i !== index)
      : shown.length < MAX_SERIES
        ? [...shown, index]
        : shown;
  }

  function play() {
    if (!recording) return;
    if (head >= recording.end) head = recording.start;
    playing = true;
  }

  // Moves the play head while playing, at the chosen speed.
  $effect(() => {
    if (!playing) return;
    let last = performance.now();
    let frame = requestAnimationFrame(function step(now) {
      const rec = recording;
      if (!rec) return;
      head = Math.min(rec.end, head + (now - last) * speed);
      last = now;
      if (head >= rec.end) {
        playing = false;
        return;
      }
      frame = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(frame);
  });
</script>

<header class="tool-head">
  <h1>{t("tools.player")}</h1>
  <p>{t("trendplayertool.plays_back_a_recording")}</p>
</header>

<div class="row">
  <button class="btn accent" onclick={load}
    >{t("trendplayertool.open_recording")}</button
  >
  {#if recording}
    <span class="dim"
      >{fileName} · {t("trendplayertool.rows_count", { count: recording.rows })} ·
      {duration(recording.end - recording.start)}</span
    >
  {/if}
</div>

{#if problem}<p class="bad" role="alert">{problem}</p>{/if}

{#if recording}
  <div
    class="columns"
    role="group"
    aria-label={t("trendplayertool.columns_on_the_chart")}
  >
    {#each recording.columns as c, i (i)}
      <label class="check">
        <input
          type="checkbox"
          checked={shown.includes(i)}
          disabled={!shown.includes(i) && shown.length >= MAX_SERIES}
          onchange={() => toggle(i)}
        />
        {c.name}{#if c.unit}<span class="dim">{c.unit}</span>{/if}
      </label>
    {/each}
  </div>

  <div class="transport">
    {#if playing}
      <button class="btn" onclick={() => (playing = false)}
        >{t("trendplayertool.pause")}</button
      >
    {:else}
      <button class="btn" onclick={play}>{t("trendplayertool.play")}</button>
    {/if}
    <input
      type="range"
      class="scrub"
      min={recording.start}
      max={recording.end}
      step="any"
      bind:value={head}
      aria-label={t("trendplayertool.play_head")}
    />
    <span class="mono time">{clock(head)}</span>
    <Select
      bind:value={speed}
      label={t("trendplayertool.speed")}
      options={speeds.map((s) => ({ value: s, label: `${s}×` }))}
    />
    <Select
      bind:value={windowMs}
      label={t("chartview.time_span")}
      options={windows.map((w) => ({ value: w.ms, label: w.label }))}
    />
    <ChartExport
      {charts}
      {groups}
      from={span.from}
      to={span.to}
      name={fileName.replace(/\.csv$/i, "")}
      disabled={!shown.length}
    />
  </div>

  {#if !shown.length}
    <p class="empty">{t("trendplayertool.choose_columns_to_plot")}</p>
  {:else}
    <div class="charts" bind:this={charts}>
      {#each groups as g (g.unit)}
        <TrendChart
          unit={g.unit}
          lines={g.lines}
          from={span.from}
          to={span.to}
          onremove={toggle}
          {markers}
        />
      {/each}
    </div>
    <MarkerTable {markers} lines={groups.flatMap((g) => g.lines)} />
  {/if}
{:else if !problem}
  <p class="empty">{t("trendplayertool.text_columns_cannot_be")}</p>
{/if}

<style>
  p {
    margin: 0;
  }

  .columns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
  }

  .columns .dim {
    margin-left: 4px;
    font-size: 12px;
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .scrub {
    flex: 1;
    accent-color: rgb(var(--fg-rgb));
  }

  .time {
    font-size: 12px;
    color: var(--text-2);
  }

  .charts {
    flex: 1;
    min-height: 320px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
</style>
