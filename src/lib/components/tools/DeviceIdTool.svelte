<script module lang="ts">
  import type { DeviceIdentity } from "$lib/types";

  // Kept while other tools are shown.
  let result = $state.raw<DeviceIdentity | null>(null);
  let level = $state(1);
</script>

<script lang="ts">
  import { errorText } from "$lib/api";
  import { hex } from "$lib/format";
  import { slidePill } from "$lib/motion";
  import { bench } from "$lib/state.svelte";

  const levels = [
    { id: 1, label: "Basic" },
    { id: 2, label: "Regular" },
    { id: 3, label: "Extended" },
  ];

  let busy = $state(false);
  let error = $state<string | null>(null);

  const where = $derived(
    bench.transport === "rtu"
      ? `${bench.serialPort || "the serial port"}, unit ${bench.unitId}`
      : `${bench.host}:${bench.tcpPort}, unit ${bench.unitId}`,
  );

  /** What the conformity byte says the device supports. */
  function conformity(c: number): string {
    const kind = ["", "basic", "regular", "extended"][c & 0x7f] ?? "unknown";
    return `${kind} objects, ${c & 0x80 ? "stream and single-object access" : "stream access only"}`;
  }

  async function read() {
    busy = true;
    error = null;
    try {
      result = await bench.readDeviceId(level);
    } catch (e) {
      result = null;
      const text = errorText(e);
      error = text.includes("0x01")
        ? 'The device does not support identification (FC43): it answered "illegal function".'
        : text;
    } finally {
      busy = false;
    }
  }
</script>

<header class="tool-head">
  <h1>Identify the device</h1>
  <p>
    Asks the device for its vendor, product code and revision with Read Device
    Identification (FC43). Uses the connection of step 1 on the Main page:
    {where}. Reads only.
  </p>
</header>

<div class="row">
  <div class="field">
    <span>Objects</span>
    <div class="segmented" role="radiogroup" aria-label="Objects" use:slidePill>
      {#each levels as l (l.id)}
        <button
          role="radio"
          aria-checked={level === l.id}
          class:on={level === l.id}
          onclick={() => (level = l.id)}>{l.label}</button
        >
      {/each}
    </div>
  </div>
  <button
    class="btn accent"
    disabled={busy ||
      bench.scanning ||
      (bench.transport === "rtu" && !bench.serialPort)}
    onclick={read}>{busy ? "Asking…" : "Read identification"}</button
  >
</div>

{#if error}
  <p class="bad" role="alert">{error}</p>
{:else if result}
  {#if result.objects.length}
    <table class="data">
      <thead>
        <tr><th>ID</th><th>Object</th><th>Value</th></tr>
      </thead>
      <tbody>
        {#each result.objects as o (o.id)}
          <tr>
            <td class="mono dim">0x{hex(o.id, 2)}</td>
            <td>{o.name}</td>
            <td class="mono value">{o.value || "—"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty">The device answered without any objects.</p>
  {/if}
  <p class="dim">
    Conformity 0x{hex(result.conformity, 2)}: {conformity(result.conformity)}.
  </p>
{:else}
  <p class="empty">
    Many devices answer with their name and firmware version. Devices that do
    not support it answer "illegal function".
  </p>
{/if}

<style>
  .value {
    word-break: break-all;
  }

  p {
    margin: 0;
  }
</style>
