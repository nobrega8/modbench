<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { untrack } from "svelte";
  import { api } from "$lib/api";
  import ConnectionStep from "$lib/components/ConnectionStep.svelte";
  import RequestStep from "$lib/components/RequestStep.svelte";
  import TestStep from "$lib/components/TestStep.svelte";
  import UpdateBanner from "$lib/components/UpdateBanner.svelte";
  import LibraryView from "$lib/components/LibraryView.svelte";
  import SettingsView from "$lib/components/SettingsView.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";
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
      listen<number>("scan-progress", (e) => bench.onScanProgress(e.payload)),
      listen<string>("open-file", (e) => bench.openMap(e.payload)),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });

  // Choices are kept per device: put back the ones of the device now shown
  // whenever the map, the endpoint or the unit ID change.
  $effect(() => {
    void bench.deviceKey;
    untrack(() => bench.restoreDevice());
  });

  $effect(() => {
    // After a page reload the backend may still be polling a device the UI no
    // longer knows about: start from a clean state.
    api.disconnect().catch(() => {});
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
  <!-- The title bar: drag to move, double-click to maximize -->
  <header class="topbar" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region
      ><img src="/favicon.png" alt="" width="20" height="20" />Modbench</span
    >
    <nav aria-label="Pages">
      {#each pages as p (p.id)}
        <button
          class:on={bench.page === p.id}
          aria-current={bench.page === p.id ? "page" : undefined}
          onclick={() => (bench.page = p.id)}>{p.label}</button
        >
      {/each}
    </nav>
    <div class="drag" data-tauri-drag-region></div>
    <WindowControls />
  </header>
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
  /* Top bar on the grey ground: wordmark + pill navigation */
  .topbar {
    display: flex;
    align-items: center;
    flex: none;
    height: 44px;
    /* reach the window edges so the controls sit in the corner */
    margin: -10px -12px 0;
    padding-left: 16px;
    user-select: none;
  }

  .topbar nav {
    display: flex;
    gap: 4px;
  }

  .drag {
    flex: 1;
    align-self: stretch;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    margin-right: 16px;
    font-weight: 600;
    letter-spacing: -0.011em;
  }

  .brand img {
    border-radius: 5px;
    pointer-events: none;
  }

  .topbar nav button {
    height: 30px;
    padding: 0 14px;
    border: 0;
    border-radius: 8px;
    background: none;
    font-weight: 500;
    color: var(--text-2);
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      color 0.14s var(--ease);
  }

  .topbar nav button:hover {
    color: var(--text);
    background: rgb(var(--fg-rgb) / 0.05);
  }

  .topbar nav button.on {
    color: var(--text);
    background: var(--card);
    box-shadow: var(--e1);
  }

  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 20px 22px;
    background: var(--card);
    border-radius: 14px;
    box-shadow: var(--e2);
  }

  .app {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100vh;
    padding: 10px 12px 12px;
  }

  .map-error {
    border-radius: 14px;
    background: rgb(var(--alert-rgb) / 0.08);
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
