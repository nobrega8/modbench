<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { slidePill, viewIn } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench, type Tab } from "#lib/state.svelte.ts";
  import ManualChart from "./ManualChart.svelte";
  import MapChart from "./MapChart.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import Counter from "./Counter.svelte";
  import LogView from "./LogView.svelte";
  import MapTable from "./MapTable.svelte";
  import MapWriteList from "./MapWriteList.svelte";
  import RawView from "./RawView.svelte";
  import RegisterGrid from "./RegisterGrid.svelte";
  import StepCard from "./StepCard.svelte";
  import TimeStat from "./TimeStat.svelte";
  import WriteGrid from "./WriteGrid.svelte";

  const TAB_ICON: Record<Tab, IconName> = {
    map: "map",
    chart: "chart",
    registers: "registers",
    write: "write",
    raw: "raw",
    log: "log",
  };

  const tabs = $derived([
    ...(bench.source === "map"
      ? [
          { id: "map", label: t("tab.map") },
          {
            id: "chart",
            label: `${t("tab.chart")}${bench.chartSeries.length ? ` (${bench.chartSeries.length})` : ""}`,
          },
        ]
      : // A map reads its own registers: the raw grid is for manual reads.
        [
          { id: "registers", label: t("tab.registers") },
          {
            id: "chart",
            label: `${t("tab.chart")}${bench.manualSeries.length ? ` (${bench.manualSeries.length})` : ""}`,
          },
        ]),
    { id: "write", label: t("tab.write") },
    { id: "raw", label: t("tab.raw") },
    {
      id: "log",
      label: `${t("tab.log")}${bench.log.length ? ` (${bench.log.length})` : ""}`,
    },
  ] as { id: Tab; label: string }[]);

  const noPort = $derived(bench.transport === "rtu" && bench.serialPort === "");
  const noMap = $derived(bench.source === "map" && bench.map === null);
  const blocked = $derived(
    noPort ? t("test.noPort") : noMap ? t("test.noMap") : null,
  );

  const where = $derived(bench.connectionText);
</script>

<StepCard step={3} title={t("step.test")} grow>
  <div class="controls">
    <button
      class="run start"
      disabled={bench.running || blocked !== null}
      title={blocked ?? t("test.start")}
      onclick={() => bench.start()}
    >
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"
        ><path d="M4 2.5v11l9-5.5z" fill="currentColor" /></svg
      >
      {t("test.start")}
    </button>
    <button
      class="run stop"
      disabled={!bench.running}
      onclick={() => bench.stop()}
    >
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
        ><rect
          x="2.5"
          y="2.5"
          width="11"
          height="11"
          rx="1"
          fill="currentColor"
        /></svg
      >
      {t("test.stop")}
    </button>

    <div class="counters">
      <Counter
        label={t("test.valid")}
        value={bench.counters.valid}
        tone="ok"
        onreset={() => (bench.counters.valid = 0)}
      />
      <Counter
        label={t("test.errors")}
        value={bench.counters.errors}
        tone="bad"
        onreset={() => (bench.counters.errors = 0)}
      />
      <Counter
        label={t("test.timeouts")}
        value={bench.counters.timeouts}
        tone="warn"
        onreset={() => (bench.counters.timeouts = 0)}
      />
      <TimeStat
        times={bench.times}
        onreset={() => bench.resetTimes()}
        timeout={bench.timeoutMs}
        locked={bench.running}
        onuse={(ms) => (bench.timeoutMs = ms)}
      />
    </div>

    <div class="state" aria-live="polite">
      {#if settings.readOnly}
        <button
          class="ro"
          title={t("test.readOnly.hint")}
          onclick={() => (bench.page = "settings")}>{t("test.readOnly")}</button
        >
      {/if}
      {#if bench.running && bench.link}
        <span class="live warn"
          ><span class="dot"></span>{t("test.reconnecting", { where })}</span
        >
        <span
          class="sub"
          title={[bench.link.cause, bench.link.error]
            .filter(Boolean)
            .join("\n") || undefined}
          >{t("test.lost", {
            cause: bench.link.cause ? ` (${bench.link.cause})` : "",
            attempt: bench.link.attempt,
            seconds: (bench.link.retry_ms / 1000).toFixed(1),
          })}{bench.link.error
            ? t("test.lastError", { error: bench.link.error })
            : ""}</span
        >
      {:else if bench.running}
        <span class="live"
          ><span class="dot"></span>{t("test.running", { where })}</span
        >
        {#if bench.source === "map" && bench.lastCycle}
          <span class="sub"
            >{t("test.cycle", {
              requests: bench.lastCycle.requests,
              ms: bench.lastCycle.duration_ms.toFixed(0),
            })}</span
          >
        {/if}
      {:else if blocked}
        <span class="sub">{blocked}</span>
      {:else}
        <span class="sub">{t("test.stopped")}</span>
      {/if}
      {#if bench.status && !bench.status.ok}
        <span class="bad" title={bench.status.text}>{bench.status.text}</span>
      {/if}
    </div>
  </div>

  <div class="segmented tabs" role="tablist" use:slidePill>
    {#each tabs as tab (tab.id)}
      <button
        role="tab"
        aria-selected={bench.tab === tab.id}
        class:on={bench.tab === tab.id}
        onclick={() => (bench.tab = tab.id)}
        ><Icon name={TAB_ICON[tab.id]} size={14} />{tab.label}</button
      >
    {/each}
  </div>

  <div class="panel" role="tabpanel">
    {#key bench.tab}<div class="view" in:viewIn>
        {#if bench.tab === "registers"}
          <RegisterGrid />
        {:else if bench.tab === "map"}
          {#if bench.map}
            {#key bench.map.path}<MapTable open={bench.map} />{/key}
          {:else}
            <p class="empty">{t("test.openMap")}</p>
          {/if}
        {:else if bench.tab === "chart"}
          {#if bench.source === "manual"}
            <ManualChart />
          {:else if bench.map}
            {#key bench.map.path}<MapChart open={bench.map} />{/key}
          {:else}
            <p class="empty">{t("test.openMapChart")}</p>
          {/if}
        {:else if bench.tab === "write"}
          {#if settings.readOnly}
            <p class="ro-note" role="status">
              {t("test.readOnlyOn")}
              <button
                class="btn small"
                onclick={() => (bench.page = "settings")}
                >{t("test.settings")}</button
              >
            </p>
          {/if}
          {#if bench.source === "map" && bench.map}
            {#key bench.map.path}<MapWriteList open={bench.map} />{/key}
          {:else}
            <WriteGrid />
          {/if}
        {:else if bench.tab === "raw"}
          <RawView />
        {:else}
          <LogView />
        {/if}
      </div>{/key}
  </div>
</StepCard>

<style>
  .controls {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    flex-wrap: wrap;
  }

  .run {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 20px;
    border: 0;
    border-radius: 10px;
    font-size: 15px;
    font-weight: 600;
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      transform 0.1s var(--ease);
  }

  .run:active:not(:disabled) {
    transform: scale(0.975);
  }

  .run:disabled {
    opacity: 0.35;
    cursor: default;
  }

  /* Start = the ink fill; Stop = a red tint that commits on hover */
  .start {
    background: var(--accent);
    color: var(--on-accent);
  }

  .start:hover:not(:disabled) {
    background: rgb(var(--fg-rgb) / 0.84);
  }

  .stop {
    background: rgb(var(--alert-rgb) / 0.12);
    color: rgb(var(--alert-rgb));
  }

  .stop:hover:not(:disabled) {
    background: rgb(var(--alert-rgb));
    color: rgb(var(--alert-fg-rgb));
  }

  .counters {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 16px;
    margin-left: 14px;
  }

  .state {
    margin-left: auto;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    flex: 0 1 420px;
    min-width: 0;
    text-align: right;
    font-size: 13px;
  }

  /* One line; the full message is in the tooltip */
  .state .bad,
  .state .sub {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .live {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-weight: 600;
    color: var(--text);
  }

  .live.warn {
    color: rgb(var(--alert-rgb));
  }

  .live.warn .dot {
    background: rgb(var(--alert-rgb));
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--live);
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }

  .sub {
    color: var(--text-2);
  }

  .ro {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: none;
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .ro:hover {
    color: var(--text);
  }

  .bad {
    color: var(--bad);
  }

  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .tabs {
    align-self: flex-start;
    margin-top: 16px;
  }

  .panel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding-top: 12px;
  }

  .view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .empty {
    color: var(--text-2);
  }

  .ro-note {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 0 10px;
    padding: 8px 12px;
    border-radius: 9px;
    background: var(--subtle);
    color: var(--text-2);
  }
</style>
