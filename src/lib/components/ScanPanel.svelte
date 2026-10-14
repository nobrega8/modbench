<script lang="ts">
  import { slidePill } from "$lib/motion";
  import { bench } from "$lib/state.svelte";
  import IntInput from "./IntInput.svelte";
  import SerialScan from "./SerialScan.svelte";

  let { onclose }: { onclose: () => void } = $props();

  let from = $state(1);
  let to = $state(247);
  let timeout = $state(300);
  let mode = $state<"units" | "serial">("units");
  const serial = $derived(bench.transport === "rtu" && mode === "serial");

  const busy = $derived(bench.scanning);
  const done = $derived(bench.scanHits !== null && !bench.scanning);
  const progress = $derived(
    bench.scanUnit === null || to < from
      ? 0
      : Math.min(1, Math.max(0, (bench.scanUnit - from) / (to - from + 1))),
  );

  function use(unit: number) {
    bench.unitId = unit;
  }
</script>

<div class="panel">
  <div class="head">
    {#if bench.transport === "rtu"}
      <div
        class="segmented"
        role="tablist"
        aria-label="What to find"
        use:slidePill
      >
        <button
          role="tab"
          aria-selected={!serial}
          class:on={!serial}
          disabled={busy}
          onclick={() => (mode = "units")}>Unit IDs</button
        >
        <button
          role="tab"
          aria-selected={serial}
          class:on={serial}
          disabled={busy}
          onclick={() => (mode = "serial")}>Serial settings</button
        >
      </div>
    {:else}
      <strong>Find unit IDs</strong>
    {/if}
    <span class="dim"
      >{serial
        ? `Tries each baud rate, parity and stop bits on ${bench.serialPort || "the serial port"} and asks for one register.`
        : "Asks each unit ID for one holding register."} Reads only; nothing is written.</span
    >
    <button class="btn" onclick={onclose} disabled={busy}>Close</button>
  </div>

  {#if serial}
    <SerialScan />
  {:else}
    <div class="row">
      <label class="field">
        <span>From</span>
        <IntInput
          value={from}
          min={0}
          max={255}
          width="72px"
          label="First unit ID"
          disabled={busy}
          onchange={(n) => (from = n)}
        />
      </label>
      <label class="field">
        <span>To</span>
        <IntInput
          value={to}
          min={0}
          max={255}
          width="72px"
          label="Last unit ID"
          disabled={busy}
          onchange={(n) => (to = n)}
        />
      </label>
      <label class="field" title="How long to wait for each unit ID">
        <span>Wait (ms)</span>
        <IntInput
          value={timeout}
          min={20}
          max={5000}
          width="80px"
          label="Wait per unit ID"
          disabled={busy}
          onchange={(n) => (timeout = n)}
        />
      </label>
      {#if busy}
        <button class="btn" onclick={() => bench.cancelScan()}>Stop</button>
        <span class="dim mono">Unit {bench.scanUnit}…</span>
      {:else}
        <button
          class="btn accent"
          disabled={bench.running || from > to}
          onclick={() => bench.scan(from, to, timeout)}>Scan</button
        >
      {/if}
    </div>

    {#if busy}
      <div
        class="bar"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(progress * 100)}
      >
        <div style:width="{progress * 100}%"></div>
      </div>
    {/if}

    {#if bench.scanError}
      <p class="bad">{bench.scanError}</p>
    {:else if done}
      {#if bench.scanHits?.length}
        <ul class="hits">
          {#each bench.scanHits as hit (hit.unit)}
            <li>
              <span class="mono unit">Unit {hit.unit}</span>
              <span class="dim">{hit.response}</span>
              <button
                class="btn"
                disabled={bench.unitId === hit.unit}
                onclick={() => use(hit.unit)}
                >{bench.unitId === hit.unit ? "In use" : "Use"}</button
              >
            </li>
          {/each}
        </ul>
      {:else}
        <p class="dim">
          No unit ID answered. Check the cable, baud rate and parity, or raise
          the wait.
        </p>
      {/if}
    {/if}
  {/if}
</div>

<style>
  .panel {
    margin-top: 12px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .head .dim {
    flex: 1;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent, currentColor);
  }

  .hits {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .hits li {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .unit {
    width: 80px;
    font-weight: 600;
  }

  .mono {
    font-family: var(--mono);
    font-size: 13px;
  }

  .dim {
    color: var(--text-3);
  }

  .bad {
    color: var(--bad);
    margin: 0;
  }
</style>
