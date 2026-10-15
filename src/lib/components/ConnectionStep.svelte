<script lang="ts">
  import { api } from "#lib/api.ts";
  import { fieldIn, slidePill } from "#lib/motion.ts";
  import { bench } from "#lib/state.svelte.ts";
  import StepCard from "./StepCard.svelte";
  import Select from "./Select.svelte";

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
</script>

<StepCard step={1} title="Connection">
  <div class="row">
    <div class="field">
      <span>Interface</span>
      <div
        class="segmented"
        role="tablist"
        aria-label="Connection type"
        use:slidePill
      >
        <button
          role="tab"
          aria-selected={serial}
          class:on={serial}
          disabled={locked}
          onclick={() => setSerial(true)}>Serial</button
        >
        <button
          role="tab"
          aria-selected={!serial}
          class:on={!serial}
          disabled={locked}
          onclick={() => setSerial(false)}>Ethernet</button
        >
      </div>
    </div>
    {#if serial}
      <label in:fieldIn class="field port">
        <span>Port</span>
        <span class="with-btn">
          <Select
            bind:value={bench.serialPort}
            disabled={locked}
            label="Serial port"
            placeholder="No ports found"
            options={ports.map((p) => ({ value: p, label: p }))}
          />
          <button
            class="btn icon"
            title="Search for serial ports again"
            aria-label="Search for serial ports again"
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
      <label
        in:fieldIn
        class="field"
        title="RTU sends binary frames with a CRC; ASCII sends hex text with an LRC, used by some older devices"
      >
        <span>Framing</span>
        <Select
          bind:value={bench.framing}
          disabled={locked}
          label="Framing"
          options={[
            { value: "rtu", label: "RTU" },
            { value: "ascii", label: "ASCII" },
          ]}
        />
      </label>
      <label in:fieldIn class="field">
        <span>Baud rate</span>
        <Select
          bind:value={bench.baudrate}
          disabled={locked}
          label="Baud rate"
          options={baudrates.map((b) => ({ value: b, label: String(b) }))}
        />
      </label>
      <label in:fieldIn class="field">
        <span>Data bits</span>
        <Select
          bind:value={bench.dataBits}
          disabled={locked}
          label="Data bits"
          options={[
            { value: 8, label: "8 bits" },
            { value: 7, label: "7 bits" },
          ]}
        />
      </label>
      <label in:fieldIn class="field">
        <span>Parity</span>
        <Select
          bind:value={bench.parity}
          disabled={locked}
          label="Parity"
          options={[
            { value: "N", label: "None" },
            { value: "E", label: "Even" },
            { value: "O", label: "Odd" },
          ]}
        />
      </label>
      <label in:fieldIn class="field">
        <span>Stop bits</span>
        <Select
          bind:value={bench.stopBits}
          disabled={locked}
          label="Stop bits"
          options={[
            { value: 1, label: "1 stop bit" },
            { value: 2, label: "2 stop bits" },
          ]}
        />
      </label>
    {:else}
      <label in:fieldIn class="field">
        <span>Protocol</span>
        <Select
          bind:value={bench.transport}
          disabled={locked}
          label="Protocol"
          options={[
            { value: "tcp", label: "Modbus TCP" },
            { value: "rtuovertcp", label: "Modbus RTU over TCP" },
            { value: "udp", label: "Modbus UDP" },
            { value: "rtuoverudp", label: "Modbus RTU over UDP" },
          ]}
        />
      </label>
      <label in:fieldIn class="field host">
        <span>IP address</span>
        <input
          class="input num"
          bind:value={bench.host}
          disabled={locked}
          spellcheck="false"
        />
      </label>
      <label in:fieldIn class="field">
        <span>Port</span>
        <input
          class="input num short"
          type="number"
          min="1"
          max="65535"
          bind:value={bench.tcpPort}
          disabled={locked}
        />
      </label>
    {/if}
    <label class="field" title="Pause between two requests">
      <span>Delay (ms)</span>
      <input
        class="input num short"
        type="number"
        min="0"
        step="10"
        bind:value={bench.delayMs}
        disabled={locked}
      />
    </label>
    <label
      class="field"
      title="Largest number of registers in one read request. Lower it for devices that cannot read 125 at once: longer reads are split into several requests."
    >
      <span>Max registers</span>
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
    <label class="field" title="How long to wait for a response">
      <span>Timeout (ms)</span>
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
</style>
