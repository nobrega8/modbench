<script lang="ts">
  import { demo } from "#lib/demo.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
</script>

{#if demo.active}
  <section class="banner live" role="status">
    <div class="text">
      <strong>Demo · simulated device</strong>
      <span
        >No hardware is connected: Modbench's own simulator plays a made-up
        meter on this computer (127.0.0.1:{bench.tcpPort}). Try:</span
      >
      <ol>
        <li>
          <button class="link" onclick={() => (bench.tab = "map")}
            >Device map</button
          >: the values by name, moving.
        </li>
        <li>
          <button class="link" onclick={() => (bench.tab = "chart")}
            >Chart</button
          >: Voltage L1 over time; right-click a value to add more.
        </li>
        <li>
          <button class="link" onclick={() => (bench.tab = "raw")}
            >Raw data</button
          >: the frames on the wire, byte by byte.
        </li>
      </ol>
    </div>
    <button class="btn accent" onclick={() => demo.exit()}>Leave demo</button>
  </section>
{:else if (demo.offered && !demo.dismissed) || demo.error}
  <section class="banner" aria-label="Demo">
    <div class="text">
      {#if demo.error}
        <strong class="bad">The demo could not start</strong>
        <span>{demo.error}</span>
      {:else}
        <strong>New to Modbench?</strong>
        <span
          >Try it with a simulated device: no hardware or map needed. Your
          settings are left as they are.</span
        >
      {/if}
    </div>
    <button
      class="btn accent"
      disabled={demo.starting}
      onclick={() => demo.start()}
      >{demo.starting ? "Starting…" : "Start demo"}</button
    >
    <button
      class="btn"
      onclick={() => {
        demo.error = null;
        demo.dismiss(false);
      }}>Not now</button
    >
    {#if !demo.error}
      <button class="btn" onclick={() => demo.dismiss(true)}
        >Don't ask again</button
      >
    {/if}
  </section>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border-radius: 14px;
    background: var(--card);
    box-shadow: var(--e2);
    flex-shrink: 0;
  }

  .banner.live {
    box-shadow:
      var(--e2),
      inset 0 0 0 2px var(--accent);
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    color: var(--text-2);
  }

  strong {
    color: var(--text);
  }

  .bad {
    color: var(--bad);
  }

  ol {
    margin: 4px 0 0;
    padding-left: 20px;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-weight: 600;
    text-decoration: underline;
    cursor: pointer;
  }
</style>
