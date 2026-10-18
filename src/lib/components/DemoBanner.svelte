<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { demo } from "#lib/demo.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
</script>

{#if demo.active}
  <section class="banner live" role="status">
    <div class="text">
      <strong>{t("demobanner.demo_simulated_device")}</strong>
      <span
        >No hardware is connected: Modbench's own simulator plays a made-up
        meter on this computer (127.0.0.1:{bench.tcpPort}). Try:</span
      >
      <ol>
        <li>
          <button class="link" onclick={() => (bench.tab = "map")}
            >{t("tab.map")}</button
          >: the values by name, moving.
        </li>
        <li>
          <button class="link" onclick={() => (bench.tab = "chart")}
            >{t("tab.chart")}</button
          >: Voltage L1 over time; right-click a value to add more.
        </li>
        <li>
          <button class="link" onclick={() => (bench.tab = "raw")}
            >{t("tab.raw")}</button
          >: the frames on the wire, byte by byte.
        </li>
      </ol>
    </div>
    <button class="btn accent" onclick={() => demo.exit()}
      >{t("demobanner.leave_demo")}</button
    >
  </section>
{:else if (demo.offered && !demo.dismissed) || demo.error}
  <section class="banner" aria-label={t("demobanner.demo")}>
    <div class="text">
      {#if demo.error}
        <strong class="bad">{t("demobanner.the_demo_could_not")}</strong>
        <span>{demo.error}</span>
      {:else}
        <strong>{t("demobanner.new_to_modbench")}</strong>
        <span>{t("demobanner.try_it_with_a")}</span>
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
      }}>{t("demobanner.not_now")}</button
    >
    {#if !demo.error}
      <button class="btn" onclick={() => demo.dismiss(true)}
        >{t("demobanner.don_t_ask_again")}</button
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
