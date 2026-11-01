<!--
  The bus monitor: listens to a serial line where a master already talks to
  its devices, without sending anything, and lists each request with its
  answer, decoded.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { api } from "#lib/api.ts";
  import { busMonitor as mon } from "#lib/bus-monitor.svelte.ts";
  import {
    keeps,
    monitorCsv,
    valuesText,
    whereText,
  } from "#lib/bus-monitor.ts";
  import { clock } from "#lib/format.ts";
  import { functionName } from "#lib/frames.ts";
  import { contextMenu, copy } from "#lib/contextmenu.svelte.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import type { MonitorEvent } from "#lib/types.ts";
  import ExportCsv from "../ExportCsv.svelte";
  import Select from "../Select.svelte";

  /** The newest shown; the export has them all. */
  const SHOWN = 500;
  const baudrates = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
  let ports = $state<string[]>([]);

  async function refreshPorts() {
    try {
      ports = await api.listSerialPorts();
      if (!ports.includes(mon.serialPort)) mon.serialPort = ports[0] ?? "";
    } catch {
      ports = [];
    }
  }

  $effect(() => {
    if (!mon.listening) refreshPorts();
  });

  const portOptions = $derived(
    (mon.serialPort && !ports.includes(mon.serialPort)
      ? [mon.serialPort, ...ports]
      : ports
    ).map((p) => ({ value: p, label: p })),
  );

  const kept = $derived(mon.events.filter((e) => keeps(mon.filter, e)));
  const shown = $derived(kept.slice(-SHOWN).reverse());
  const counts = $derived({
    total: mon.events.length,
    problems: mon.events.filter(
      (e) => e.outcome !== "ok" && e.outcome !== "broadcast",
    ).length,
  });
  /** Function codes heard, for the filter. */
  const functions = $derived(
    [...new Set(mon.events.map((e) => e.function).filter((f) => f !== null))]
      .sort((a, b) => a - b)
      .map((f) => ({ value: String(f), label: functionName(f) })),
  );

  function result(e: MonitorEvent): string {
    switch (e.outcome) {
      case "ok":
        return "OK";
      case "exception":
        return `${t("monitor.exception")} 0x${(e.exception ?? 0).toString(16).toUpperCase().padStart(2, "0")}`;
      case "no_answer":
        return t("monitor.no_answer");
      case "broadcast":
        return t("monitor.broadcast");
      case "bad_check":
        return t("monitor.bad_check");
      default:
        return t("monitor.unknown");
    }
  }

  function line(e: MonitorEvent): string {
    return [
      clock(e.time_ms),
      e.unit ?? "",
      e.function !== null ? functionName(e.function) : "",
      whereText(e),
      valuesText(e, Infinity),
      result(e),
      e.ms ?? "",
      e.request,
      e.response ?? "",
    ].join("\t");
  }
</script>

<header class="tool-head">
  <h1>{t("tools.monitor")}</h1>
  <p>{t("monitor.intro")}</p>
</header>

<div class="row">
  <label class="field">
    <span>{t("conn.serialPort")}</span>
    <Select
      bind:value={mon.serialPort}
      disabled={mon.listening}
      label={t("conn.serialPort")}
      placeholder={t("conn.noPorts")}
      options={portOptions}
    />
  </label>
  <label class="field">
    <span>{t("monitor.framing")}</span>
    <Select
      bind:value={mon.framing}
      disabled={mon.listening}
      label={t("monitor.framing")}
      options={[
        { value: "rtu", label: "RTU" },
        { value: "ascii", label: "ASCII" },
      ]}
    />
  </label>
  <label class="field">
    <span>{t("conn.baud")}</span>
    <Select
      bind:value={mon.baudrate}
      disabled={mon.listening}
      label={t("conn.baud")}
      options={baudrates.map((b) => ({ value: b, label: String(b) }))}
    />
  </label>
  <label class="field">
    <span>{t("conn.parity")}</span>
    <Select
      bind:value={mon.parity}
      disabled={mon.listening}
      label={t("conn.parity")}
      options={[
        { value: "N", label: t("conn.parity.none") },
        { value: "E", label: t("conn.parity.even") },
        { value: "O", label: t("conn.parity.odd") },
      ]}
    />
  </label>
  <label class="field">
    <span>{t("conn.stopBits")}</span>
    <Select
      bind:value={mon.stopBits}
      disabled={mon.listening}
      label={t("conn.stopBits")}
      options={[
        { value: 1, label: t("conn.stop1") },
        { value: 2, label: t("conn.stop2") },
      ]}
    />
  </label>
  {#if mon.listening}
    <button class="btn danger" onclick={() => mon.stop()}
      >{t("test.stop")}</button
    >
    <span class="live"><span class="dot"></span>{t("monitor.listening")}</span>
  {:else}
    <button class="btn accent" onclick={() => mon.start()}
      >{t("test.start")}</button
    >
  {/if}
</div>
{#if mon.error}<p class="bad" role="alert">{mon.error}</p>{/if}

<div class="row">
  <label class="field">
    <span>{t("computedform.unit")}</span>
    <input
      class="input num"
      type="number"
      min="0"
      max="247"
      placeholder={t("monitor.all")}
      value={mon.filter.unit ?? ""}
      onchange={(e) => {
        const v = e.currentTarget.value.trim();
        mon.filter.unit = v === "" ? null : +v;
      }}
    />
  </label>
  <label class="field">
    <span>{t("logview.function")}</span>
    <Select
      value={mon.filter.fn === null ? "" : String(mon.filter.fn)}
      label={t("logview.function")}
      options={[{ value: "", label: t("monitor.all") }, ...functions]}
      onchange={(v: string) => (mon.filter.fn = v === "" ? null : +v)}
    />
  </label>
  <label class="check switch-row">
    <input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={mon.filter.problems}
    />
    {t("monitor.problems_only")}
  </label>
  <span class="spacer"></span>
  <span class="dim"
    >{t("monitor.counts", {
      total: counts.total,
      problems: counts.problems,
    })}</span
  >
  <ExportCsv
    name="bus-monitor.csv"
    disabled={kept.length === 0}
    make={() => monitorCsv(kept, settings.csvSeparator)}
  />
</div>

{#if shown.length}
  <div class="log">
    <table class="data">
      <thead>
        <tr>
          <th>{t("logview.time")}</th>
          <th class="num">{t("computedform.unit")}</th>
          <th>{t("step.request")}</th>
          <th>{t("req.address")}</th>
          <th>{t("monitor.values")}</th>
          <th>{t("logview.result")}</th>
          <th class="num">ms</th>
        </tr>
      </thead>
      <tbody>
        {#each shown as e, i (shown.length - i + counts.total)}
          <tr
            title={`${e.request}${e.response ? `\n${e.response}` : ""}`}
            oncontextmenu={(ev) =>
              contextMenu.show(ev, [
                { label: t("logview.copy_line"), action: () => copy(line(e)) },
                {
                  label: t("logview.copy_all"),
                  action: () => copy(kept.map(line).join("\n")),
                },
                null,
                {
                  label: t("monitor.clear"),
                  danger: true,
                  action: () => mon.clear(),
                },
              ])}
          >
            <td class="mono dim">{clock(e.time_ms)}</td>
            <td class="num mono">{e.unit ?? ""}</td>
            <td>{e.function !== null ? functionName(e.function) : ""}</td>
            <td class="mono">{whereText(e)}</td>
            <td class="mono values">{valuesText(e)}</td>
            <td
              class:bad={e.outcome !== "ok" && e.outcome !== "broadcast"}
              class:ok={e.outcome === "ok"}>{result(e)}</td
            >
            <td class="num mono dim">{e.ms ?? ""}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  <p class="empty">
    {mon.listening ? t("monitor.waiting") : t("monitor.empty")}
  </p>
{/if}

<style>
  p {
    margin: 0;
  }

  .switch-row {
    height: 30px;
  }

  .spacer {
    flex: 1;
  }

  .live {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    font-weight: 600;
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

  .num {
    width: 70px;
  }

  .values {
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .bad {
    color: var(--bad);
  }

  .ok {
    color: var(--ok, var(--text-2));
  }

  .log {
    max-height: 480px;
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .log :global(thead th) {
    position: sticky;
    top: 0;
    background: var(--card);
  }

  .empty {
    color: var(--text-3);
    font-size: 13px;
  }
</style>
