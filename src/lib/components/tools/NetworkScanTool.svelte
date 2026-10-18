<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
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
  <h1>{t("tools.network")}</h1>
  <p>{t("networkscantool.modbus_tcp_devices_on")}</p>
</header>

<div class="row">
  <label class="field targets">
    <span>{t("addressformat.addresses")}</span>
    <input
      class="input num"
      bind:value={scan.targets}
      disabled={scan.running}
      spellcheck="false"
      placeholder="192.168.1.0/24"
      title={t("networkscantool.a_subnet_192_168")}
    />
  </label>
  <label class="field">
    <span>{t("conn.port")}</span>
    <IntInput
      value={scan.port}
      min={1}
      max={65535}
      width="80px"
      label={t("networkscantool.tcp_port")}
      disabled={scan.running}
      onchange={(n) => (scan.port = n)}
    />
  </label>
  <label class="field">
    <span>{t("networkscantool.unit_id")}</span>
    <IntInput
      value={scan.unit}
      min={0}
      max={255}
      width="72px"
      label={t("networkscantool.unit_id")}
      disabled={scan.running}
      onchange={(n) => (scan.unit = n)}
    />
  </label>
  <div class="field">
    <span>{t("networkscantool.mode")}</span>
    <div
      class="segmented"
      role="tablist"
      aria-label={t("networkscantool.scan_mode")}
      use:slidePill
    >
      <button
        role="tab"
        aria-selected={scan.mode === "fast"}
        class:on={scan.mode === "fast"}
        disabled={scan.running}
        title={t("networkscantool.a_short_wait_and")}
        onclick={() => (scan.mode = "fast")}>{t("networkscantool.fast")}</button
      >
      <button
        role="tab"
        aria-selected={scan.mode === "thorough"}
        class:on={scan.mode === "thorough"}
        disabled={scan.running}
        title={t("networkscantool.a_longer_wait_and")}
        onclick={() => (scan.mode = "thorough")}
        >{t("networkscantool.thorough")}</button
      >
    </div>
  </div>
  <label class="field" title={t("networkscantool.how_long_to_wait")}>
    <span>{t("scanpanel.wait_ms")}</span>
    <IntInput
      value={scan.timeoutMs}
      min={20}
      max={10000}
      width="80px"
      label={t("networkscantool.wait_per_address")}
      disabled={scan.running}
      onchange={(n) => (scan.timeoutMs = n)}
    />
  </label>
  <InterfaceSelect disabled={scan.running} />
  <label class="check" title={t("networkscantool.read_vendor_and_product")}>
    <input
      type="checkbox"
      bind:checked={scan.identify}
      disabled={scan.running}
    />
    {t("networkscantool.identify_fc43")}
  </label>
  {#if scan.running}
    <button class="btn" onclick={() => scan.stop()}>{t("test.stop")}</button>
  {:else}
    <button
      class="btn accent"
      disabled={!scan.targets.trim()}
      onclick={() => scan.start()}>{t("scanpanel.scan")}</button
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
          <th>{t("req.address")}</th>
          <th>{t("libraryview.status")}</th>
          <th class="num">{t("logview.time")}</th>
          <th>{t("deviceidtool.answer")}</th>
          <th>{t("networkscantool.identity")}</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each scan.hits as h (h.ip)}
          <tr>
            <td class="mono">{h.ip}:{h.port}</td>
            <td
              >{#if h.status === "modbus"}<span class="ok">Modbus</span
                >{:else}<span class="dim">{t("networkscantool.port_open")}</span
                >{/if}</td
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
                  onclick={() => use(h)}>{t("mappicker.open")}</button
                >
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if !scan.running}
    <p class="empty">
      No address opened port {scan.last?.port ?? scan.port}.{scan.last
        ?.retries === 0
        ? " Try the thorough mode."
        : " Try a longer wait."}
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
