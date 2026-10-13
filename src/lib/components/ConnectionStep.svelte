<script lang="ts">
  import { api } from "$lib/api";
  import { bench } from "$lib/state.svelte";
  import StepCard from "./StepCard.svelte";

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
  <div class="tabs" role="tablist" aria-label="Connection type">
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

  <div class="row">
    {#if serial}
      <label class="field port">
        <span>Port</span>
        <span class="with-btn">
          <select class="input" bind:value={bench.serialPort} disabled={locked}>
            {#if ports.length === 0}<option value="">No ports found</option
              >{/if}
            {#each ports as p (p)}<option value={p}>{p}</option>{/each}
          </select>
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
      <label class="field">
        <span>Baud rate</span>
        <select class="input" bind:value={bench.baudrate} disabled={locked}>
          {#each baudrates as b (b)}<option value={b}>{b}</option>{/each}
        </select>
      </label>
      <label class="field">
        <span>Data bits</span>
        <select class="input" bind:value={bench.dataBits} disabled={locked}>
          <option value={8}>8 bits</option>
          <option value={7}>7 bits</option>
        </select>
      </label>
      <label class="field">
        <span>Parity</span>
        <select class="input" bind:value={bench.parity} disabled={locked}>
          <option value="N">None</option>
          <option value="E">Even</option>
          <option value="O">Odd</option>
        </select>
      </label>
      <label class="field">
        <span>Stop bits</span>
        <select class="input" bind:value={bench.stopBits} disabled={locked}>
          <option value={1}>1 stop bit</option>
          <option value={2}>2 stop bits</option>
        </select>
      </label>
    {:else}
      <label class="field">
        <span>Protocol</span>
        <select class="input" bind:value={bench.transport} disabled={locked}>
          <option value="tcp">Modbus TCP</option>
          <option value="rtuovertcp">Modbus RTU over TCP</option>
        </select>
      </label>
      <label class="field host">
        <span>IP address</span>
        <input
          class="input num"
          bind:value={bench.host}
          disabled={locked}
          spellcheck="false"
        />
      </label>
      <label class="field">
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
  .tabs {
    display: flex;
    gap: 2px;
    margin-bottom: 10px;
    border-bottom: 1px solid var(--border);
  }

  .tabs button {
    position: relative;
    border: 0;
    background: none;
    padding: 4px 12px 7px;
    font-weight: 500;
    color: var(--text-2);
    cursor: pointer;
  }

  .tabs button.on {
    color: var(--text);
    font-weight: 600;
  }

  .tabs button.on::after {
    content: "";
    position: absolute;
    left: 10px;
    right: 10px;
    bottom: -1px;
    height: 3px;
    border-radius: 2px;
    background: var(--accent);
  }

  .tabs button:disabled:not(.on) {
    opacity: 0.5;
    cursor: default;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .row select {
    min-width: 110px;
  }

  .port select {
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
