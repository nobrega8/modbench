<script module lang="ts">
  import type { Recording } from "$lib/playback";

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
  import { api, errorText } from "$lib/api";
  import { parseRecording, upTo } from "$lib/playback";
  import { clock } from "$lib/format";
  import { MAX_SERIES } from "$lib/trend";
  import type { Register } from "$lib/types";
  import Select from "../Select.svelte";
  import TrendChart, { type Line } from "../TrendChart.svelte";

  const speeds = [1, 2, 4, 8, 16, 60];
  const windows = [
    { ms: 60_000, label: "1 minute" },
    { ms: 5 * 60_000, label: "5 minutes" },
    { ms: 15 * 60_000, label: "15 minutes" },
    { ms: 60 * 60_000, label: "1 hour" },
    { ms: 0, label: "Everything" },
  ];

  let playing = $state(false);
  let problem = $state<string | null>(null);

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

  function duration(ms: number): string {
    const s = Math.round(ms / 1000);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h ? `${h} h ${m} min` : m ? `${m} min ${s % 60} s` : `${s} s`;
  }
</script>

<header class="tool-head">
  <h1>Trend player</h1>
  <p>
    Plays back a CSV file made with Record… on the Device map tab, so a test can
    be looked at again later: play it faster, or drag the slider to any moment.
  </p>
</header>

<div class="row">
  <button class="btn accent" onclick={load}>Open recording…</button>
  {#if recording}
    <span class="dim"
      >{fileName} · {recording.rows} rows · {duration(
        recording.end - recording.start,
      )}</span
    >
  {/if}
</div>

{#if problem}<p class="bad" role="alert">{problem}</p>{/if}

{#if recording}
  <div class="columns" role="group" aria-label="Columns on the chart">
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
      <button class="btn" onclick={() => (playing = false)}>Pause</button>
    {:else}
      <button class="btn" onclick={play}>Play</button>
    {/if}
    <input
      type="range"
      class="scrub"
      min={recording.start}
      max={recording.end}
      step="any"
      bind:value={head}
      aria-label="Play head"
    />
    <span class="mono time">{clock(head)}</span>
    <Select
      bind:value={speed}
      label="Speed"
      options={speeds.map((s) => ({ value: s, label: `${s}×` }))}
    />
    <Select
      bind:value={windowMs}
      label="Time span"
      options={windows.map((w) => ({ value: w.ms, label: w.label }))}
    />
  </div>

  {#if !shown.length}
    <p class="empty">Choose columns to plot.</p>
  {:else}
    <div class="charts">
      {#each groups as g (g.unit)}
        <TrendChart
          unit={g.unit}
          lines={g.lines}
          from={span.from}
          to={span.to}
          onremove={toggle}
        />
      {/each}
    </div>
  {/if}
{:else if !problem}
  <p class="empty">
    Columns with text, such as state labels, cannot be plotted and are left out.
  </p>
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
