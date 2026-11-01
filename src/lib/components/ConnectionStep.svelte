<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { api } from "#lib/api.ts";
  import { fieldIn, slidePill } from "#lib/motion.ts";
  import { bench } from "#lib/state.svelte.ts";
  import StepCard from "./StepCard.svelte";
  import InterfaceSelect from "./InterfaceSelect.svelte";
  import Select from "./Select.svelte";
  import { open as openFile } from "@tauri-apps/plugin-dialog";

  const baudrates = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
  let ports = $state<string[]>([]);
  const locked = $derived(bench.running);
  const serial = $derived(bench.transport === "rtu");

  async function refreshPorts() {
    try {
      ports = await api.listSerialPorts();
      if (!ports.includes(bench.serialPort)) bench.serialPort = ports[0] ?? "";
    } catch {
      ports = [];
    }
  }

  $effect(() => {
    if (serial) refreshPorts();
  });

  function setSerial(on: boolean) {
    bench.transport = on ? "rtu" : "tcp";
  }

  /** Modbus/TCP Security on or off; the port follows when it was the
   * other's default. */
  function setTls(on: boolean) {
    bench.tls = on;
    if (on && bench.tcpPort === 502) bench.tcpPort = 802;
    else if (!on && bench.tcpPort === 802) bench.tcpPort = 502;
  }

  async function pickPem(field: "tlsCa" | "tlsCert" | "tlsKey") {
    const picked = await openFile({
      multiple: false,
      filters: [
        { name: "PEM", extensions: ["pem", "crt", "cer", "key"] },
        { name: "*", extensions: ["*"] },
      ],
    });
    if (typeof picked === "string") bench[field] = picked;
  }

  const TLS_FILES = $derived([
    {
      field: "tlsCa" as const,
      label: t("conn.tls_ca"),
      hint: t("conn.tls_ca.hint"),
    },
    {
      field: "tlsCert" as const,
      label: t("conn.tls_cert"),
      hint: t("conn.tls_cert.hint"),
    },
    {
      field: "tlsKey" as const,
      label: t("conn.tls_key"),
      hint: t("conn.tls_cert.hint"),
    },
  ]);
</script>

<StepCard step={1} title={t("step.connection")}>
  <div class="row">
    <div class="field">
      <span>{t("conn.interface")}</span>
      <div
        class="segmented"
        role="tablist"
        aria-label={t("conn.type")}
        use:slidePill
      >
        <button
          role="tab"
          aria-selected={serial}
          class:on={serial}
          disabled={locked}
          onclick={() => setSerial(true)}>{t("conn.serial")}</button
        >
        <button
          role="tab"
          aria-selected={!serial}
          class:on={!serial}
          disabled={locked}
          onclick={() => setSerial(false)}>{t("conn.ethernet")}</button
        >
      </div>
    </div>
    {#if serial}
      <label in:fieldIn class="field port">
        <span>{t("conn.port")}</span>
        <span class="with-btn">
          <Select
            bind:value={bench.serialPort}
            disabled={locked}
            label={t("conn.serialPort")}
            placeholder={t("conn.noPorts")}
            options={ports.map((p) => ({ value: p, label: p }))}
          />
          <button
            class="btn icon"
            title={t("conn.rescan")}
            aria-label={t("conn.rescan")}
            disabled={locked}
            onclick={refreshPorts}
          >
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
              ><path
                d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v3h-3"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              /></svg
            >
          </button>
        </span>
      </label>
      <label in:fieldIn class="field" title={t("conn.framing.hint")}>
        <span>{t("conn.framing")}</span>
        <Select
          bind:value={bench.framing}
          disabled={locked}
          label={t("conn.framing")}
          options={[
            { value: "rtu", label: "RTU" },
            { value: "ascii", label: "ASCII" },
          ]}
        />
      </label>
      <label in:fieldIn class="field">
        <span>{t("conn.baud")}</span>
        <Select
          bind:value={bench.baudrate}
          disabled={locked}
          label={t("conn.baud")}
          options={baudrates.map((b) => ({ value: b, label: String(b) }))}
        />
      </label>
      <label in:fieldIn class="field">
        <span>{t("conn.dataBits")}</span>
        <Select
          bind:value={bench.dataBits}
          disabled={locked}
          label={t("conn.dataBits")}
          options={[
            { value: 8, label: t("conn.bits8") },
            { value: 7, label: t("conn.bits7") },
          ]}
        />
      </label>
      <label in:fieldIn class="field">
        <span>{t("conn.parity")}</span>
        <Select
          bind:value={bench.parity}
          disabled={locked}
          label={t("conn.parity")}
          options={[
            { value: "N", label: t("conn.parity.none") },
            { value: "E", label: t("conn.parity.even") },
            { value: "O", label: t("conn.parity.odd") },
          ]}
        />
      </label>
      <label in:fieldIn class="field">
        <span>{t("conn.stopBits")}</span>
        <Select
          bind:value={bench.stopBits}
          disabled={locked}
          label={t("conn.stopBits")}
          options={[
            { value: 1, label: t("conn.stop1") },
            { value: 2, label: t("conn.stop2") },
          ]}
        />
      </label>
    {:else}
      <label in:fieldIn class="field">
        <span>{t("conn.protocol")}</span>
        <Select
          bind:value={bench.transport}
          disabled={locked}
          label={t("conn.protocol")}
          options={[
            { value: "tcp", label: "Modbus TCP" },
            { value: "rtuovertcp", label: "Modbus RTU over TCP" },
            { value: "udp", label: "Modbus UDP" },
            { value: "rtuoverudp", label: "Modbus RTU over UDP" },
          ]}
        />
      </label>
      <label in:fieldIn class="field host">
        <span>{t("conn.ip")}</span>
        <input
          class="input num"
          bind:value={bench.host}
          disabled={locked}
          spellcheck="false"
        />
      </label>
      <label in:fieldIn class="field">
        <span>{t("conn.port")}</span>
        <input
          class="input num short"
          type="number"
          min="1"
          max="65535"
          bind:value={bench.tcpPort}
          disabled={locked}
        />
      </label>
      <InterfaceSelect disabled={locked} />
      {#if bench.transport === "tcp"}
        <label in:fieldIn class="check tls-switch" title={t("conn.tls.hint")}>
          <input
            type="checkbox"
            class="switch"
            role="switch"
            checked={bench.tls}
            disabled={locked}
            onchange={(e) => setTls(e.currentTarget.checked)}
          />
          TLS
        </label>
        {#if bench.tls}
          <div class="tls-files">
            {#each TLS_FILES as f (f.field)}
              <label in:fieldIn class="field file" title={f.hint}>
                <span>{f.label}</span>
                <span class="with-btn">
                  <input
                    class="input mono"
                    bind:value={bench[f.field]}
                    disabled={locked}
                    spellcheck="false"
                    placeholder={f.field === "tlsCa"
                      ? t("conn.tls_any")
                      : t("conn.tls_none")}
                  />
                  <button
                    class="btn icon"
                    title={t("conn.browse")}
                    aria-label={t("conn.browse")}
                    disabled={locked}
                    onclick={() => pickPem(f.field)}>…</button
                  >
                </span>
              </label>
            {/each}
          </div>
        {/if}
      {/if}
    {/if}
    <label class="field" title={t("conn.delay.hint")}>
      <span>{t("conn.delay")}</span>
      <input
        class="input num short"
        type="number"
        min="0"
        step="10"
        bind:value={bench.delayMs}
        disabled={locked}
      />
    </label>
    <label class="field" title={t("conn.maxRegs.hint")}>
      <span>{t("conn.maxRegs")}</span>
      <input
        class="input num short"
        type="number"
        min="1"
        max="125"
        bind:value={bench.maxRegisters}
        onchange={() =>
          (bench.maxRegisters = Math.min(
            125,
            Math.max(1, Math.round(bench.maxRegisters) || 125),
          ))}
        disabled={locked}
      />
    </label>
    <label class="field" title={t("conn.timeout.hint")}>
      <span>{t("conn.timeout")}</span>
      <input
        class="input num short"
        type="number"
        min="50"
        step="50"
        bind:value={bench.timeoutMs}
        disabled={locked}
      />
    </label>
  </div>
</StepCard>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .row :global(.select) {
    min-width: 110px;
  }

  .port :global(.select) {
    min-width: 120px;
  }

  .with-btn {
    display: flex;
    gap: 4px;
  }

  .icon {
    padding: 0;
    width: 30px;
  }

  .host input {
    width: 180px;
  }

  .short {
    width: 96px;
  }

  .tls-switch {
    height: 30px;
    align-self: flex-end;
  }

  .tls-files {
    flex-basis: 100%;
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
  }

  .file .input {
    width: 220px;
  }

  .with-btn {
    display: flex;
    gap: 4px;
  }
</style>
