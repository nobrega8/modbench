<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { api } from "$lib/api";
  import ConnectionStep from "$lib/components/ConnectionStep.svelte";
  import RequestStep from "$lib/components/RequestStep.svelte";
  import TestStep from "$lib/components/TestStep.svelte";
  import { bench } from "$lib/state.svelte";
  import type { PollCycle, TrafficEntry } from "$lib/types";

  interface RawChunk {
    time_ms: number;
    direction: "out" | "in";
    bytes: number[];
  }

  $effect(() => {
    const stops = [
      listen<TrafficEntry>("traffic", (e) => bench.onTraffic(e.payload)),
      listen<RawChunk>("raw", (e) =>
        bench.onRaw(e.payload.time_ms, e.payload.direction, e.payload.bytes),
      ),
      listen<PollCycle>("poll", (e) => bench.onPoll(e.payload)),
      listen<string>("poll-stopped", (e) => bench.onPollStopped(e.payload)),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });

  $effect(() => {
    api.startupFile().then((path) => {
      if (path) bench.openMap(path);
    });
  });

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }
</script>

<div class="app">
  <ConnectionStep />
  <RequestStep />
  {#if bench.mapError}
    <section class="map-error" role="alert">
      <header>
        <strong
          >{fileName(bench.mapError.path)} has errors and was not opened.</strong
        >
        Fix these lines in the file, then open it again.
        <button class="btn" onclick={() => (bench.mapError = null)}
          >Dismiss</button
        >
      </header>
      <ul>
        {#each bench.mapError.diagnostics as d, i (i)}
          <li class={d.severity}>
            <span class="where"
              >{d.line === null ? "file" : `line ${d.line}`}</span
            >
            <span>{d.message}</span>
            <span class="code">{d.code}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
  <TestStep />
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100vh;
    padding: 8px;
  }

  .map-error {
    border: 1px solid var(--bad);
    border-radius: 8px;
    background: var(--card);
    max-height: 180px;
    overflow: auto;
  }

  .map-error header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    color: var(--text-2);
  }

  .map-error header strong {
    color: var(--bad);
  }

  .map-error header .btn {
    margin-left: auto;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0 12px 8px;
  }

  li {
    display: grid;
    grid-template-columns: 70px 1fr auto;
    gap: 10px;
    padding: 3px 0;
    border-top: 1px solid var(--border);
    font-size: 13px;
  }

  .where,
  .code {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
  }

  li.error .where {
    color: var(--bad);
  }
</style>
