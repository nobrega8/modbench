<script lang="ts">
  import { slidePill } from "#lib/motion.ts";
  import { netScanner as scan } from "#lib/netscan.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { NetHit } from "#lib/types.ts";
  import { networkScanCsv } from "#lib/scan-export.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import ExportCsv from "../ExportCsv.svelte";
  import IntInput from "../IntInput.svelte";
  import InterfaceSelect from "../InterfaceSelect.svelte";

  const progress = $derived(
    scan.progress ? scan.progress.done / Math.max(1, scan.progress.total) : 0,
  );
  const modbus = $derived(scan.hits?.filter((h) => h.status === "modbus"));
  const openOnly = $derived(scan.hits?.filter((h) => h.status === "tcp_only"));

  /** Opens a device found by the scan as the connection of step 1. */
  function use(hit: NetHit) {
    bench.transport = "tcp";
    bench.host = hit.ip;
    bench.tcpPort = hit.port;
    bench.unitId = hit.unit;
    bench.page = "main";
  }
</script>

<header class="tool-head">
  <h1>Network scan</h1>
  <p>
    Looks for Modbus TCP devices on a subnet or a range of addresses. Each
    address that opens the port is asked for one holding register; any answer,
    an exception included, shows a Modbus device. Reads only; nothing is
    written.
  </p>
</header>

<div class="row">
  <label class="field targets">
    <span>Addresses</span>
    <input
      class="input num"
      bind:value={scan.targets}
      disabled={scan.running}
      spellcheck="false"
      placeholder="192.168.1.0/24"
      title="A subnet (192.168.1.0/24), a range (192.168.1.10-50 or 192.168.1.10-192.168.2.20) or addresses separated by commas"
    />
  </label>
  <label class="field">
    <span>Port</span>
    <IntInput
      value={scan.port}
      min={1}
      max={65535}
      width="80px"
      label="TCP port"
      disabled={scan.running}
      onchange={(n) => (scan.port = n)}
    />
  </label>
  <label class="field">
    <span>Unit ID</span>
    <IntInput
      value={scan.unit}
      min={0}
      max={255}
      width="72px"
      label="Unit ID"
      disabled={scan.running}
      onchange={(n) => (scan.unit = n)}
    />
  </label>
  <div class="field">
    <span>Mode</span>
    <div class="segmented" role="tablist" aria-label="Scan mode" use:slidePill>
      <button
        role="tab"
        aria-selected={scan.mode === "fast"}
        class:on={scan.mode === "fast"}
        disabled={scan.running}
        title="A short wait and no retries: a quick first pass"
        onclick={() => (scan.mode = "fast")}>Fast</button
      >
      <button
        role="tab"
        aria-selected={scan.mode === "thorough"}
        class:on={scan.mode === "thorough"}
        disabled={scan.running}
        title="A longer wait and a retry: for slow devices or networks"
        onclick={() => (scan.mode = "thorough")}>Thorough</button
      >
    </div>
  </div>
  <label
    class="field"
    title="How long to wait for the connection, and then for the answer"
  >
    <span>Wait (ms)</span>
    <IntInput
      value={scan.timeoutMs}
      min={20}
      max={10000}
      width="80px"
      label="Wait per address"
      disabled={scan.running}
      onchange={(n) => (scan.timeoutMs = n)}
    />
  </label>
  <InterfaceSelect disabled={scan.running} />
  <label class="check" title="Read vendor and product of the devices found">
    <input
      type="checkbox"
      bind:checked={scan.identify}
      disabled={scan.running}
    />
    Identify (FC43)
  </label>
  {#if scan.running}
    <button class="btn" onclick={() => scan.stop()}>Stop</button>
  {:else}
    <button
      class="btn accent"
      disabled={!scan.targets.trim()}
      onclick={() => scan.start()}>Scan</button
    >
  {/if}
  {#if scan.hits?.length && scan.last}
    {@const last = scan.last}
    <ExportCsv
      name="modbench-network-scan.csv"
      make={() =>
        networkScanCsv(
          scan.hits ?? [],
          last,
          scan.progress,
          settings.csvSeparator,
        )}
    />
  {/if}
</div>

<p class="dim policy">
  {scan.mode === "fast" ? "Fast" : "Thorough"}: waits {scan.timeoutMs} ms for each
  address, {scan.retries === 0
    ? "no retries"
    : `${scan.retries} ${scan.retries === 1 ? "retry" : "retries"} on a timeout`}.
</p>

{#if scan.running}
  <div
    class="bar"
    role="progressbar"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(progress * 100)}
  >
    <div style:width="{progress * 100}%"></div>
  </div>
  {#if scan.progress}
    <p class="dim mono">
      {scan.progress.done} of {scan.progress.total} addresses · {scan.progress
        .found} found so far
    </p>
  {/if}
{/if}

{#if scan.error}
  <p class="bad" role="alert">{scan.error}</p>
{/if}

{#if scan.hits && (scan.hits.length || !scan.running) && !scan.error}
  {#if !scan.running}
    <p>
      {modbus?.length ?? 0} Modbus
      {modbus?.length === 1 ? "device" : "devices"}{openOnly?.length
        ? `, and ${openOnly.length} ${openOnly.length === 1 ? "address" : "addresses"} with the port open but no Modbus answer`
        : ""}{scan.progress && scan.progress.done < scan.progress.total
        ? ` (stopped after ${scan.progress.done} of ${scan.progress.total} addresses)`
        : ""}.
    </p>
  {/if}
  {#if scan.hits.length}
    <table class="data">
      <thead>
        <tr>
          <th>Address</th>
          <th>Status</th>
          <th class="num">Time</th>
          <th>Answer</th>
          <th>Identity</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each scan.hits as h (h.ip)}
          <tr>
            <td class="mono">{h.ip}:{h.port}</td>
            <td
              >{#if h.status === "modbus"}<span class="ok">Modbus</span
                >{:else}<span class="dim">Port open</span>{/if}</td
            >
            <td class="num mono">{h.response_ms.toFixed(0)} ms</td>
            <td class="answer" title={h.response}
              >{h.status === "tcp_only" && h.response.startsWith("timed out")
                ? `no answer within ${scan.last?.timeout_ms ?? scan.timeoutMs} ms`
                : h.response}</td
            >
            <td>{h.identity ?? ""}</td>
            <td class="act">
              {#if h.status === "modbus"}
                <button
                  class="btn small"
                  disabled={bench.running}
                  title={bench.running
                    ? "Stop the test on the Main page first"
                    : `Connect to ${h.ip}:${h.port}, unit ${h.unit}, on the Main page`}
                  onclick={() => use(h)}>Open</button
                >
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if !scan.running}
    <p class="empty">
      No address opened port {scan.last?.port ?? scan.port}. No answer within
      the wait time does not prove there is no device{scan.last?.retries === 0
        ? ": try the thorough mode"
        : ": try a longer wait"}.
    </p>
  {/if}
{/if}

<style>
  p {
    margin: 0;
  }

  .policy {
    margin-top: -6px;
    font-size: 12px;
  }

  .targets input {
    width: 260px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    color: var(--text-2);
  }

  .ok {
    color: var(--ok);
    font-weight: 500;
  }

  .answer {
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
  }

  .act {
    text-align: right;
  }

  .btn.small {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
  }
</style>
