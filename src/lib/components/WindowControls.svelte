<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  const win = getCurrentWindow();
  let maximized = $state(false);

  $effect(() => {
    const sync = () => win.isMaximized().then((m) => (maximized = m));
    sync();
    const stop = win.onResized(sync);
    return () => {
      stop.then((f) => f());
    };
  });
</script>

<div class="controls">
  <button
    aria-label={t("windowcontrols.minimize")}
    title={t("windowcontrols.minimize")}
    onclick={() => win.minimize()}
  >
    <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5h10" /></svg>
  </button>
  <button
    aria-label={maximized
      ? t("windowcontrols.restore")
      : t("windowcontrols.maximize")}
    title={maximized
      ? t("windowcontrols.restore")
      : t("windowcontrols.maximize")}
    onclick={() => win.toggleMaximize()}
  >
    {#if maximized}
      <svg viewBox="0 0 10 10" aria-hidden="true">
        <path
          d="M2.5 2.5V1.5a1 1 0 0 1 1-1h5a1 1 0 0 1 1 1v5a1 1 0 0 1-1 1h-1"
        />
        <rect x="0.5" y="2.5" width="7" height="7" rx="1" />
      </svg>
    {:else}
      <svg viewBox="0 0 10 10" aria-hidden="true">
        <rect x="0.5" y="0.5" width="9" height="9" rx="1" />
      </svg>
    {/if}
  </button>
  <button
    class="close"
    aria-label={t("mapexplorer.close")}
    title={t("mapexplorer.close")}
    onclick={() => win.close()}
  >
    <svg viewBox="0 0 10 10" aria-hidden="true"
      ><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" /></svg
    >
  </button>
</div>

<style>
  .controls {
    display: flex;
    align-self: stretch;
    margin-left: auto;
  }

  button {
    display: grid;
    place-items: center;
    width: 46px;
    height: 100%;
    border: 0;
    border-radius: 0;
    background: none;
    color: var(--text-2);
    cursor: default;
    transition:
      background-color 0.12s var(--ease),
      color 0.12s var(--ease);
  }

  button:hover {
    background: rgb(var(--fg-rgb) / 0.07);
    color: var(--text);
  }

  button:active {
    background: rgb(var(--fg-rgb) / 0.12);
  }

  /* Closing is the one destructive control in the chrome */
  button.close:hover {
    background: #c42b1c;
    color: #fff;
  }

  button.close:active {
    background: #b0281a;
  }

  button:focus-visible {
    border-radius: 0;
    box-shadow: inset var(--ring-focus);
  }

  svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
    shape-rendering: geometricPrecision;
  }

  svg :global(*) {
    vector-effect: non-scaling-stroke;
  }
</style>
