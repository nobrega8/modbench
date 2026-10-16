<script module lang="ts">
  import type {
    Area,
    DeviceIdentity,
    FunctionCheck,
    ReadLimit,
    Support,
  } from "#lib/types.ts";

  // Kept while other tools are shown.
  let result = $state.raw<DeviceIdentity | null>(null);
  let checks = $state.raw<FunctionCheck[] | null>(null);
  let address = $state(0);
  let writes = $state(false);
  let limitArea = $state<Area>("holding");
  let limit = $state.raw<{
    area: Area;
    address: number;
    read: ReadLimit;
  } | null>(null);
  let level = $state(1);
</script>

<script lang="ts">
  import { errorText } from "#lib/api.ts";
  import { AREA_NAME, formatAddress, hex } from "#lib/format.ts";
  import { slidePill } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import AddressInput from "../AddressInput.svelte";
  import Select from "../Select.svelte";

  const levels = [
    { id: 1, label: "Basic" },
    { id: 2, label: "Regular" },
    { id: 3, label: "Extended" },
  ];

  let busy = $state(false);
  let error = $state<string | null>(null);
  let checking = $state(false);
  let checkError = $state<string | null>(null);
  let measuring = $state(false);
  let limitError = $state<string | null>(null);
  const areas: Area[] = ["holding", "input", "coil", "discrete"];

  const answers: Record<Support, string> = {
    supported: "Supported",
    not_supported: "Not supported",
    refused: "Supported, refused this request",
    no_answer: "No answer",
    skipped: "Not tried",
  };
  // Functions the standard defines for serial lines only
  const serialOnly = new Set([7, 8, 11, 17]);

  const cannotAsk = $derived(
    busy ||
      checking ||
      measuring ||
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
      const found = await bench.checkFunctions(
        address,
        writes && !settings.readOnly,
      );
      if (found) checks = found;
    } catch (e) {
      checks = null;
      checkError = errorText(e);
    } finally {
      checking = false;
    }
  }

  async function measure() {
    measuring = true;
    limitError = null;
    try {
      limit = {
        area: limitArea,
        address,
        read: await bench.largestRead(limitArea, address),
      };
    } catch (e) {
      limit = null;
      limitError = errorText(e);
    } finally {
      measuring = false;
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
      Sends one request per function at an address and shows how the device
      answers: "illegal function" means it does not have the function; another
      exception means it has it but refused this request, for example because it
      has no such address. Reads only, unless write functions are allowed.
    </p>
  </header>
  <div class="row">
    <label class="field">
      <span>Address</span>
      <AddressInput
        value={address}
        width="90px"
        label="Address to try"
        disabled={cannotAsk}
        onchange={(n) => (address = n)}
      />
    </label>
    <label
      class="check"
      title={settings.readOnly
        ? "Read-only mode is on (Settings › Connection)"
        : "Writes back the value just read at the address"}
    >
      <input
        type="checkbox"
        bind:checked={writes}
        disabled={cannotAsk || settings.readOnly}
      />
      Also try write functions{#if settings.readOnly}<span class="dim"
          >{" · off in read-only mode"}</span
        >{/if}
    </label>
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

  <header class="tool-head">
    <h2>Largest read</h2>
    <p>
      Finds how many registers or bits the device returns in one read from the
      address above, by halving: up to 125 registers or 2000 bits. When one more
      fails with "illegal data value", that is the device's limit: set it as Max
      registers on the Main page. "Illegal data address" means the area has a
      gap there instead.
    </p>
  </header>
  <div class="row">
    <label class="field">
      <span>Area</span>
      <Select
        bind:value={limitArea}
        label="Area"
        disabled={cannotAsk}
        options={areas.map((a) => ({ value: a, label: AREA_NAME[a] + "s" }))}
      />
    </label>
    <button class="btn" disabled={cannotAsk} onclick={measure}
      >{measuring ? "Reading…" : "Find largest read"}</button
    >
  </div>
  {#if limitError}
    <p class="bad" role="alert">{limitError}</p>
  {:else if limit}
    <p>
      From {AREA_NAME[limit.area].toLowerCase()}
      {formatAddress(limit.address, settings.addressFormat, limit.area)}:
      <strong>{limit.read.largest}</strong>
      {limit.area === "coil" || limit.area === "discrete"
        ? "bits"
        : "registers"}
      in one read{#if limit.read.limit}<span class="dim"
          >{`. One more: ${limit.read.limit}`}</span
        >{:else}, the most the protocol allows{/if}.
    </p>
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

  .answer.not_supported,
  .answer.skipped {
    color: var(--text-2);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
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
