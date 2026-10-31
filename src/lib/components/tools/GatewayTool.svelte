<!--
  The gateway: clients connect here (Modbus TCP or UDP, RTU over TCP or UDP,
  or a serial port) and each request goes on to the device of step 1, its
  answer back. Every request is listed as it passes.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { api } from "#lib/api.ts";
  import { clock } from "#lib/format.ts";
  import { functionName } from "#lib/frames.ts";
  import { contextMenu, copy } from "#lib/contextmenu.svelte.ts";
  import { gateway } from "#lib/gateway.svelte.ts";
  import { whereText, type SimKind } from "#lib/simulator.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { GatewayEvent } from "#lib/types.ts";
  import IntInput from "../IntInput.svelte";
  import Select from "../Select.svelte";

  const kinds: { value: SimKind; label: string }[] = $derived([
    { value: "tcp", label: "Modbus TCP" },
    { value: "rtuovertcp", label: "RTU over TCP" },
    { value: "udp", label: "Modbus UDP" },
    { value: "rtuoverudp", label: "RTU over UDP" },
    { value: "rtu", label: t("simulatortool.serial_rtu") },
    { value: "ascii", label: t("simulatortool.serial_ascii") },
  ]);
  const baudrates = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
  const serial = $derived(gateway.kind === "rtu" || gateway.kind === "ascii");
  const stopped = $derived(gateway.running === null);
  let ports = $state<string[]>([]);

  async function refreshPorts() {
    try {
      ports = await api.listSerialPorts();
      if (!ports.includes(gateway.serialPort))
        gateway.serialPort = ports[0] ?? "";
    } catch {
      ports = [];
    }
  }

  $effect(() => {
    if (serial && stopped) refreshPorts();
  });

  const portOptions = $derived(
    (gateway.serialPort && !ports.includes(gateway.serialPort)
      ? [gateway.serialPort, ...ports]
      : ports
    ).map((p) => ({ value: p, label: p })),
  );

  /** The device requests go to: step 1 of Main. */
  const upstream = $derived(whereText(bench.config().transport));

  function where(e: GatewayEvent): string {
    if (e.address === null) return "";
    return e.count !== null && e.count > 1
      ? `${e.address} ×${e.count}`
      : String(e.address);
  }

  function unitText(e: GatewayEvent): string {
    return e.unit === e.target ? String(e.unit) : `${e.unit} → ${e.target}`;
  }

  function result(e: GatewayEvent): string {
    if (e.exception === null) return "OK";
    const code = `0x${e.exception.toString(16).toUpperCase().padStart(2, "0")}`;
    return e.error ? `${code} · ${e.error}` : code;
  }

  function line(e: GatewayEvent): string {
    return [
      clock(e.time_ms),
      e.peer,
      unitText(e),
      functionName(e.function),
      where(e),
      result(e),
      `${e.ms} ms`,
    ].join("\t");
  }
</script>

<header class="tool-head">
  <h1>{t("tools.gateway")}</h1>
  <p>{t("gateway.intro", { where: upstream })}</p>
</header>

<div class="row">
  <label class="field">
    <span>{t("conn.interface")}</span>
    <Select
      bind:value={gateway.kind}
      options={kinds}
      label={t("gateway.how_clients_reach")}
      disabled={!stopped}
    />
  </label>
  {#if serial}
    <label class="field">
      <span>{t("conn.serialPort")}</span>
      <Select
        bind:value={gateway.serialPort}
        disabled={!stopped}
        label={t("conn.serialPort")}
        placeholder={t("conn.noPorts")}
        options={portOptions}
      />
    </label>
    <label class="field">
      <span>{t("conn.baud")}</span>
      <Select
        bind:value={gateway.baudrate}
        disabled={!stopped}
        label={t("conn.baud")}
        options={baudrates.map((b) => ({ value: b, label: String(b) }))}
      />
    </label>
    <label class="field">
      <span>{t("conn.parity")}</span>
      <Select
        bind:value={gateway.parity}
        disabled={!stopped}
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
        bind:value={gateway.stopBits}
        disabled={!stopped}
        label={t("conn.stopBits")}
        options={[
          { value: 1, label: t("conn.stop1") },
          { value: 2, label: t("conn.stop2") },
        ]}
      />
    </label>
  {:else}
    <label class="field">
      <span>{t("conn.port")}</span>
      <IntInput
        value={gateway.port}
        min={1}
        max={65535}
        width="80px"
        label={t("conn.port")}
        disabled={!stopped}
        onchange={(n) => (gateway.port = n)}
      />
    </label>
    <label
      class="check switch-row"
      title={t("simulatortool.let_other_computers_on")}
    >
      <input
        type="checkbox"
        class="switch"
        role="switch"
        bind:checked={gateway.isPublic}
        disabled={!stopped}
      />
      {t("simulatortool.other_computers")}
    </label>
  {/if}
</div>

<div class="row">
  <label class="field" title={t("gateway.unit_hint")}>
    <span>{t("gateway.unit")}</span>
    <input
      class="input num"
      type="number"
      min="0"
      max="247"
      placeholder={t("gateway.unit_same")}
      value={gateway.unit ?? ""}
      disabled={!stopped}
      onchange={(e) => {
        const v = e.currentTarget.value.trim();
        gateway.unit = v === "" ? null : Math.min(247, Math.max(0, +v));
      }}
    />
  </label>
  <label class="check switch-row" title={t("gateway.read_only_hint")}>
    <input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={gateway.readOnly}
      disabled={!stopped}
    />
    {t("gateway.read_only")}
  </label>
  {#if stopped}
    <button class="btn accent" onclick={() => gateway.start()}
      >{t("test.start")}</button
    >
  {:else}
    <button class="btn danger" onclick={() => gateway.stop()}
      >{t("test.stop")}</button
    >
  {/if}
  <span class="status" aria-live="polite">
    {#if gateway.running}
      <span class="live"
        ><span class="dot"></span>{t("simulatortool.listening_on_where2", {
          where: whereText(gateway.running),
        })}</span
      >
    {:else}
      <span class="dim">{t("test.stopped")}</span>
    {/if}
  </span>
</div>
{#if gateway.error}<p class="bad" role="alert">{gateway.error}</p>{/if}

<section>
  <h2 class="key">
    {t("gateway.requests")}
    <span class="dim"
      >{t("gateway.counts", {
        total: gateway.total,
        failed: gateway.failed,
      })}</span
    >
  </h2>
  {#if gateway.events.length}
    <div class="log">
      <table class="data">
        <thead>
          <tr>
            <th>{t("logview.time")}</th>
            <th>{t("framedecodertool.client")}</th>
            <th class="num">{t("computedform.unit")}</th>
            <th>{t("step.request")}</th>
            <th>{t("req.address")}</th>
            <th>{t("logview.result")}</th>
            <th class="num">ms</th>
          </tr>
        </thead>
        <tbody>
          {#each gateway.events as e, i (gateway.total - i)}
            <tr
              oncontextmenu={(ev) =>
                contextMenu.show(ev, [
                  {
                    label: t("logview.copy_line"),
                    action: () => copy(line(e)),
                  },
                  {
                    label: t("logview.copy_all"),
                    action: () =>
                      copy([...gateway.events].reverse().map(line).join("\n")),
                  },
                  null,
                  {
                    label: t("gateway.clear"),
                    danger: true,
                    action: () => gateway.clear(),
                  },
                ])}
            >
              <td class="mono dim">{clock(e.time_ms)}</td>
              <td class="mono">{e.peer}</td>
              <td class="num mono">{unitText(e)}</td>
              <td>{functionName(e.function)}</td>
              <td class="mono">{where(e)}</td>
              <td
                class:bad={e.exception !== null}
                class:ok={e.exception === null}>{result(e)}</td
              >
              <td class="num mono dim">{e.ms}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <p class="empty">{t("gateway.empty")}</p>
  {/if}
</section>

<style>
  p {
    margin: 0;
  }

  .switch-row {
    height: 30px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 30px;
  }

  .live {
    display: inline-flex;
    align-items: center;
    gap: 7px;
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

  .bad {
    color: var(--bad);
  }

  .ok {
    color: var(--ok, var(--text-2));
  }

  .log {
    max-height: 420px;
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
