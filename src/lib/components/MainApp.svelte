<script lang="ts">
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { listen } from "@tauri-apps/api/event";
  import { untrack } from "svelte";
  import { api } from "#lib/api.ts";
  import ConnectionStep from "#lib/components/ConnectionStep.svelte";
  import RequestStep from "#lib/components/RequestStep.svelte";
  import TestStep from "#lib/components/TestStep.svelte";
  import UpdateBanner from "#lib/components/UpdateBanner.svelte";
  import DemoBanner from "#lib/components/DemoBanner.svelte";
  import AssistantBar from "#lib/components/AssistantBar.svelte";
  import { assistant } from "#lib/gui-control.svelte.ts";
  import LibraryView from "#lib/components/LibraryView.svelte";
  import SettingsView from "#lib/components/SettingsView.svelte";
  import ToolsView from "#lib/components/ToolsView.svelte";
  import Icon from "#lib/components/Icon.svelte";
  import WindowControls from "#lib/components/WindowControls.svelte";
  import Toaster from "#lib/components/Toaster.svelte";
  import CommandMenu from "#lib/components/CommandMenu.svelte";
  import { slidePill, viewIn } from "#lib/motion.ts";
  import { bench, type Page } from "#lib/state.svelte.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { netScanner } from "#lib/netscan.svelte.ts";
  import { sim } from "#lib/simulator.svelte.ts";
  import { gateway } from "#lib/gateway.svelte.ts";
  import { SETUP_EXTENSION } from "#lib/sim-setup.ts";
  import {
    diagnosticWhere,
    type AddressProgress,
    type LinkEvent,
    type NetHit,
    type NetProgress,
    type PollCycle,
    type SerialProgress,
    type TrafficEntry,
  } from "#lib/types.ts";

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
      listen<LinkEvent>("link", (e) => bench.onLink(e.payload)),
      listen<number>("scan-progress", (e) => bench.onScanProgress(e.payload)),
      listen<SerialProgress>("serial-scan-progress", (e) =>
        bench.onSerialProgress(e.payload),
      ),
      listen<AddressProgress>("address-scan-progress", (e) =>
        bench.onAddressProgress(e.payload),
      ),
      listen<NetHit>("net-scan-hit", (e) => netScanner.onHit(e.payload)),
      listen<NetProgress>("net-scan-progress", (e) =>
        netScanner.onProgress(e.payload),
      ),
      listen<string>("open-file", (e) => openFile(e.payload)),
      listen("library-changed", () => bench.refreshLibrary().catch(() => {})),
    ];
    return () => stops.forEach((p) => p.then((stop) => stop()));
  });

  $effect(() => sim.connect());
  $effect(() => gateway.connect());

  // An assistant driving this window (off unless turned on in Settings).
  $effect(() => assistant.listen());
  $effect(() => {
    if (settings.assistantControl) untrack(() => assistant.apply());
  });

  // The backend enforces read-only mode; keep it in step with the setting.
  $effect(() => {
    api.setReadOnly(settings.readOnly).catch(() => {});
  });
  $effect(() => {
    api.setSingleWriteFc16(settings.singleWriteFc16).catch(() => {});
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
      if (path) openFile(path);
    });
  });

  const pages = $derived([
    { id: "main", label: "nav.main" },
    { id: "tools", label: "nav.tools" },
    { id: "library", label: "nav.library" },
    { id: "settings", label: "nav.settings" },
  ] as { id: Page; label: Key }[]);

  /** macOS draws its own window buttons at the top left of the title bar. */
  const mac = navigator.userAgent.includes("Mac OS X");

  let commandOpen = $state(false);

  /** A file given to Modbench (double-click): a simulator setup opens in
   * the simulator, anything else as a map. */
  function openFile(path: string) {
    if (!path.toLowerCase().endsWith(`.${SETUP_EXTENSION}`))
      return bench.openMap(path);
    bench.page = "tools";
    bench.tool = "simulator";
    sim.openSetupFile(path);
  }

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }
</script>

<div class="app">
  <!-- The title bar: drag to move, double-click to maximize -->
  <header class="topbar" class:mac data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region
      ><img src="/favicon.png" alt="" width="20" height="20" />Modbench</span
    >
    <nav aria-label={t("nav.pages")} use:slidePill>
      {#each pages as p (p.id)}
        <button
          class:on={bench.page === p.id}
          aria-current={bench.page === p.id ? "page" : undefined}
          onclick={() => (bench.page = p.id)}
          ><Icon name={p.id} />{t(p.label)}</button
        >
      {/each}
    </nav>
    <div class="drag" data-tauri-drag-region></div>
    <button
      class="cmd"
      class:mac
      title={t("command.open")}
      onclick={() => (commandOpen = true)}
      ><Icon name="addresses" size={14} />{t("command.search")}<kbd
        >{mac ? "⌘K" : "Ctrl K"}</kbd
      ></button
    >
    {#if !mac}<WindowControls />{/if}
  </header>
  <UpdateBanner />
  <AssistantBar />
  {#if bench.page === "tools"}
    <section class="page" in:viewIn><ToolsView /></section>
  {:else if bench.page === "library"}
    <section class="page" in:viewIn><LibraryView /></section>
  {:else if bench.page === "settings"}
    <section class="page" in:viewIn><SettingsView /></section>
  {:else}
    <div class="main" in:viewIn>
      <DemoBanner />
      <ConnectionStep />
      <RequestStep />
      {#if bench.mapError}
        <section class="map-error" role="alert">
          <header>
            <strong
              >{t("map.error", { file: fileName(bench.mapError.path) })}</strong
            >
            {t("map.error.fix")}
            <button class="btn" onclick={() => (bench.mapError = null)}
              >{t("action.dismiss")}</button
            >
          </header>
          <ul>
            {#each bench.mapError.diagnostics as d, i (i)}
              <li class={d.severity}>
                <span class="where">{diagnosticWhere(d)}</span>
                <span>{d.message}</span>
                <span class="code">{d.code}</span>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
      <TestStep />
    </div>
  {/if}
</div>

<Toaster />
<CommandMenu bind:open={commandOpen} />

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

  /* Room for the traffic lights, which sit over the title bar */
  .topbar.mac {
    padding-left: 94px;
  }

  .topbar nav {
    display: flex;
    gap: 4px;
  }

  .drag {
    flex: 1;
    align-self: stretch;
  }

  /* Opens the command menu; the shortcut is shown so it gets learnt */
  .cmd {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 28px;
    margin-right: 8px;
    padding: 0 6px 0 10px;
    border: 0;
    border-radius: 8px;
    background: rgb(var(--fg-rgb) / 0.05);
    color: var(--text-3);
    font: inherit;
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      color 0.14s var(--ease);
  }

  .cmd.mac {
    margin-right: 12px;
  }

  .cmd:hover {
    color: var(--text);
    background: rgb(var(--fg-rgb) / 0.09);
  }

  .cmd kbd {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 5px;
    background: var(--card);
    box-shadow: var(--e1);
    font-family: var(--ui);
    font-size: 11px;
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
    display: inline-flex;
    align-items: center;
    gap: 7px;
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

  /* The selected page: the sliding pill (lib/motion.ts) draws its card */
  .topbar nav button.on {
    color: var(--text);
    background: none;
  }

  .topbar nav :global(.pill) {
    border-radius: 8px;
  }

  .main {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
    /* A short window scrolls rather than squeezing the values away */
    overflow-y: auto;
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
