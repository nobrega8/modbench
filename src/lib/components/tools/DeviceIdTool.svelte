<script module lang="ts">
  import type { DeviceIdentity, FunctionCheck, Support } from "#lib/types.ts";

  // Kept while other tools are shown.
  let result = $state.raw<DeviceIdentity | null>(null);
  let checks = $state.raw<FunctionCheck[] | null>(null);
  let level = $state(1);
</script>

<script lang="ts">
  import { errorText } from "#lib/api.ts";
  import { hex } from "#lib/format.ts";
  import { slidePill } from "#lib/motion.ts";
  import { bench } from "#lib/state.svelte.ts";

  const levels = [
    { id: 1, label: "Basic" },
    { id: 2, label: "Regular" },
    { id: 3, label: "Extended" },
  ];

  let busy = $state(false);
  let error = $state<string | null>(null);
  let checking = $state(false);
  let checkError = $state<string | null>(null);

  const answers: Record<Support, string> = {
    supported: "Supported",
    not_supported: "Not supported",
    refused: "Supported, refused this request",
    no_answer: "No answer",
  };
  // Functions the standard defines for serial lines only
  const serialOnly = new Set([7, 8, 11, 17]);

  const cannotAsk = $derived(
    busy ||
      checking ||
      bench.scanning ||
      (bench.transport === "rtu" && !bench.serialPort),
  );

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

  async function check() {
    checking = true;
    checkError = null;
    try {
      checks = await bench.checkFunctions();
    } catch (e) {
      checks = null;
      checkError = errorText(e);
    } finally {
      checking = false;
    }
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
  <button class="btn accent" disabled={cannotAsk} onclick={read}
    >{busy ? "Asking…" : "Read identification"}</button
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

<section class="functions">
  <header class="tool-head">
    <h2>Function codes</h2>
    <p>
      Sends one request per read function and shows how the device answers:
      "illegal function" means it does not have the function; another exception
      means it has it but refused this request (for example address 0). Reads
      only.
    </p>
  </header>
  <div class="row">
    <button class="btn" disabled={cannotAsk} onclick={check}
      >{checking ? "Checking…" : "Check function codes"}</button
    >
  </div>

  {#if checkError}
    <p class="bad" role="alert">{checkError}</p>
  {:else if checks}
    <table class="data">
      <thead>
        <tr><th class="num">FC</th><th>Function</th><th>Answer</th></tr>
      </thead>
      <tbody>
        {#each checks as c (c.function)}
          <tr>
            <td class="num mono">{c.function}</td>
            <td
              >{c.name}{#if serialOnly.has(c.function)}<span class="dim"
                  >{" · serial line only"}</span
                >{/if}</td
            >
            <td class="answer {c.support}"
              >{answers[c.support]}{#if c.detail}<span class="dim"
                  >{` · ${c.detail}`}</span
                >{/if}</td
            >
          </tr>
        {/each}
      </tbody>
    </table>
    {#if bench.transport !== "rtu"}
      <p class="dim">
        Functions for serial lines only often answer "illegal function" over
        TCP, also on devices that have them on their RS-485 port.
      </p>
    {/if}
  {/if}
</section>

<style>
  .functions {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 12px;
  }

  .functions h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .answer.supported,
  .answer.refused {
    color: var(--ok);
  }

  .answer.not_supported {
    color: var(--text-2);
  }

  .answer.no_answer {
    color: var(--bad);
  }

  .value {
    word-break: break-all;
  }

  p {
    margin: 0;
  }
</style>
