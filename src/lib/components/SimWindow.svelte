<!--
  The simulator in a window of its own, next to the main window. The
  backend holds the simulator's state; both windows follow it.
-->
<script lang="ts">
  import { sim } from "$lib/simulator.svelte";
  import ToolsView from "./ToolsView.svelte";
  import WindowControls from "./WindowControls.svelte";

  /** macOS draws its own window buttons at the top left of the title bar. */
  const mac = navigator.userAgent.includes("Mac OS X");

  $effect(() => sim.connect());
</script>

<div class="app">
  <header class="topbar" class:mac data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region
      ><img src="/favicon.png" alt="" width="20" height="20" />Simulator</span
    >
    {#if sim.running !== null}
      <span class="live" data-tauri-drag-region
        ><span class="dot"></span>Port {sim.running}</span
      >
    {/if}
    <div class="drag" data-tauri-drag-region></div>
    {#if !mac}<WindowControls />{/if}
  </header>
  <section class="page"><ToolsView only="simulator" /></section>
</div>

<style>
  /* The same frame as the main window: a title bar on the grey ground */
  .app {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100vh;
    padding: 10px 12px 12px;
  }

  .topbar {
    display: flex;
    align-items: center;
    gap: 14px;
    flex: none;
    height: 44px;
    margin: -10px -12px 0;
    padding-left: 16px;
    user-select: none;
  }

  /* Room for the traffic lights, which sit over the title bar */
  .topbar.mac {
    padding-left: 94px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    font-weight: 600;
    letter-spacing: -0.011em;
  }

  .brand img {
    border-radius: 5px;
    pointer-events: none;
  }

  .live {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    color: var(--text-2);
  }

  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--live);
  }

  .drag {
    flex: 1;
    align-self: stretch;
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
</style>
