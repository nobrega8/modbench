<script lang="ts">
  import { bench, type Tab } from "$lib/state.svelte";
  import Counter from "./Counter.svelte";
  import LogView from "./LogView.svelte";
  import MapTable from "./MapTable.svelte";
  import MapWriteList from "./MapWriteList.svelte";
  import RawView from "./RawView.svelte";
  import RegisterGrid from "./RegisterGrid.svelte";
  import StepCard from "./StepCard.svelte";
  import WriteGrid from "./WriteGrid.svelte";

  const tabs = $derived([
    ...(bench.source === "map" ? [{ id: "map", label: "Device map" }] : []),
    { id: "registers", label: "Read registers" },
    { id: "write", label: "Write values" },
    { id: "raw", label: "Raw data" },
    {
      id: "log",
      label: `Log${bench.log.length ? ` (${bench.log.length})` : ""}`,
    },
  ] as { id: Tab; label: string }[]);

  const noPort = $derived(bench.transport === "rtu" && bench.serialPort === "");
  const noMap = $derived(bench.source === "map" && bench.map === null);
  const blocked = $derived(
    noPort
      ? "Choose a serial port in step 1, or plug in the adapter and refresh."
      : noMap
        ? "Choose a device map in step 2."
        : null,
  );

  const where = $derived(
    bench.transport === "rtu"
      ? `${bench.serialPort} · ${bench.baudrate} ${bench.dataBits}${bench.parity}${bench.stopBits}`
      : `${bench.host}:${bench.tcpPort}`,
  );
</script>

<StepCard step={3} title="Test" grow>
  <div class="controls">
    <button
      class="run start"
      disabled={bench.running || blocked !== null}
      title={blocked ?? "Start"}
      onclick={() => bench.start()}
    >
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"
        ><path d="M4 2.5v11l9-5.5z" fill="currentColor" /></svg
      >
      Start
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
      Stop
    </button>

    <div class="counters">
      <Counter
        label="Valid responses"
        value={bench.counters.valid}
        tone="ok"
        onreset={() => (bench.counters.valid = 0)}
      />
      <Counter
        label="Error responses"
        value={bench.counters.errors}
        tone="bad"
        onreset={() => (bench.counters.errors = 0)}
      />
      <Counter
        label="Timeouts"
        value={bench.counters.timeouts}
        tone="warn"
        onreset={() => (bench.counters.timeouts = 0)}
      />
    </div>

    <div class="state" aria-live="polite">
      {#if bench.running}
        <span class="live"><span class="dot"></span>Running · {where}</span>
        {#if bench.source === "map" && bench.lastCycle}
          <span class="sub"
            >{bench.lastCycle.requests} requests per cycle · {bench.lastCycle.duration_ms.toFixed(
              0,
            )} ms</span
          >
        {/if}
      {:else if blocked}
        <span class="sub">{blocked}</span>
      {:else}
        <span class="sub">Stopped</span>
      {/if}
      {#if bench.status && !bench.status.ok}
        <span class="bad" title={bench.status.text}>{bench.status.text}</span>
      {/if}
    </div>
  </div>

  <div class="tabs" role="tablist">
    {#each tabs as t (t.id)}
      <button
        role="tab"
        aria-selected={bench.tab === t.id}
        class:on={bench.tab === t.id}
        onclick={() => (bench.tab = t.id)}>{t.label}</button
      >
    {/each}
  </div>

  <div class="panel" role="tabpanel">
    {#if bench.tab === "registers"}
      <RegisterGrid />
    {:else if bench.tab === "map"}
      {#if bench.map}
        {#key bench.map.path}<MapTable open={bench.map} />{/key}
      {:else}
        <p class="empty">
          Open a <code>.mbmap</code> file in step 2 to see the device's registers
          by name, with types, units and scaling.
        </p>
      {/if}
    {:else if bench.tab === "write"}
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
    height: 48px;
    padding: 0 18px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--card);
    font-size: 16px;
    font-weight: 600;
    cursor: pointer;
  }

  .run:hover:not(:disabled) {
    background: var(--subtle);
  }

  .run:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .start svg {
    color: var(--ok);
  }

  .stop svg {
    color: var(--bad);
  }

  .counters {
    display: flex;
    gap: 16px;
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
    gap: 6px;
    font-weight: 600;
    color: var(--ok);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
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

  .bad {
    color: var(--bad);
  }

  .tabs {
    display: flex;
    gap: 2px;
    margin: 12px 0 0;
    border-bottom: 1px solid var(--border);
  }

  .tabs button {
    position: relative;
    border: 0;
    background: none;
    padding: 6px 14px 8px;
    font-weight: 500;
    color: var(--text-2);
    cursor: pointer;
  }

  .tabs button:hover {
    color: var(--text);
  }

  .tabs button.on {
    color: var(--text);
    font-weight: 600;
  }

  .tabs button.on::after {
    content: "";
    position: absolute;
    left: 12px;
    right: 12px;
    bottom: -1px;
    height: 3px;
    border-radius: 2px;
    background: var(--accent);
  }

  .panel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding-top: 12px;
  }

  .empty {
    color: var(--text-2);
  }
</style>
