<!--
  The map builder in a window of its own, next to the main window and the
  manual it is built from.
-->
<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "#lib/i18n.svelte.ts";
  import { builder } from "#lib/builder.svelte.ts";
  import MapBuilder from "./MapBuilder.svelte";
  import WindowControls from "./WindowControls.svelte";

  /** macOS draws its own window buttons at the top left of the title bar. */
  const mac = navigator.userAgent.includes("Mac OS X");

  onMount(() => {
    void builder.check();
  });
</script>

<div class="app">
  <header class="topbar" class:mac data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region
      ><img src="/favicon.png" alt="" width="20" height="20" />{t(
        "builder.title",
      )}</span
    >
    <div class="drag" data-tauri-drag-region></div>
    {#if !mac}<WindowControls />{/if}
  </header>
  <section class="page"><MapBuilder /></section>
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
