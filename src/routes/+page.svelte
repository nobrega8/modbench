<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { api } from "$lib/api";
  import ConnectionStep from "$lib/components/ConnectionStep.svelte";
  import RequestStep from "$lib/components/RequestStep.svelte";
  import TestStep from "$lib/components/TestStep.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import LibraryView from "$lib/components/LibraryView.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import { bench, type Page } from "$lib/state.svelte";
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
      listen<string>("open-file", (e) => bench.openMap(e.payload)),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });

  $effect(() => {
    bench.refreshLibrary();
    api.startupFile().then((path) => {
      if (path) bench.openMap(path);
    });
  });

  const pages = $derived([
    { id: "main", label: "Main" },
    {
      id: "library",
      label: `Library${bench.library.length ? ` (${bench.library.length})` : ""}`,
    },
    { id: "settings", label: "Settings" },
  ] as { id: Page; label: string }[]);

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }
</script>

<div class="app">
  <nav class="topbar" aria-label="Pages">
    <span class="brand"
      ><img src="/favicon.png" alt="" width="20" height="20" />Modbench</span
    >
    {#each pages as p (p.id)}
      <button
        class:on={bench.page === p.id}
        aria-current={bench.page === p.id ? "page" : undefined}
        onclick={() => (bench.page = p.id)}>{p.label}</button
      >
    {/each}
  </nav>
  <UpdateBanner />
  {#if bench.page === "library"}
    <section class="page"><LibraryView /></section>
  {:else if bench.page === "settings"}
    <section class="page"><SettingsView /></section>
  {:else}
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
  {/if}
</div>

<style>
  .topbar {
    display: flex;
    align-items: stretch;
    gap: 2px;
    margin: -8px -8px 0;
    padding: 0 12px;
    height: 40px;
    background: var(--card);
    border-bottom: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-right: 18px;
    font-weight: 600;
  }

  .topbar button {
    position: relative;
    border: 0;
    background: none;
    padding: 0 14px;
    font-weight: 500;
    color: var(--text-2);
    cursor: pointer;
  }

  .topbar button:hover {
    color: var(--text);
  }

  .topbar button.on {
    color: var(--text);
    font-weight: 600;
  }

  .topbar button.on::after {
    content: "";
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: 0;
    height: 3px;
    border-radius: 2px 2px 0 0;
    background: var(--accent);
  }

  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 16px;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 8px;
  }

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
