<script module lang="ts">
  // Kept while other tools are shown.
  let input = $state("4148 0000");
  let decimal = $state(false);
</script>

<script lang="ts">
  import { ORDERS, parseWords, wideRows, wordRows } from "$lib/decoder";
  import { slidePill } from "$lib/motion";
  import { bench } from "$lib/state.svelte";

  const words = $derived(parseWords(input, decimal));
  const wide = $derived(words ? wideRows(words) : []);
  const each = $derived(words ? wordRows(words) : []);

  /** Puts the registers last read on the Main page in the box. */
  function fromRead() {
    const r = bench.read;
    if (r?.data.kind !== "registers") return;
    decimal = false;
    input = r.data.values
      .slice(0, 8)
      .map((w) => w.toString(16).toUpperCase().padStart(4, "0"))
      .join(" ");
  }
</script>

<header class="tool-head">
  <h1>Data decoder</h1>
  <p>
    Type or paste registers to see them as every type at once, in all four byte
    orders. Useful to find a value's type and byte order before writing a map.
    Works without a connection.
  </p>
</header>

<div class="row">
  <label class="field grow">
    <span>Registers</span>
    <input
      class="input num"
      bind:value={input}
      spellcheck="false"
      placeholder={decimal ? "16712 0" : "4148 0000"}
      aria-invalid={!words}
    />
  </label>
  <div class="field">
    <span>Typed as</span>
    <div
      class="segmented"
      role="radiogroup"
      aria-label="Typed as"
      use:slidePill
    >
      <button
        role="radio"
        aria-checked={!decimal}
        class:on={!decimal}
        onclick={() => (decimal = false)}>Hex</button
      >
      <button
        role="radio"
        aria-checked={decimal}
        class:on={decimal}
        onclick={() => (decimal = true)}>Decimal</button
      >
    </div>
  </div>
  <button
    class="btn"
    disabled={bench.read?.data.kind !== "registers"}
    title="Use the registers last read on the Main page"
    onclick={fromRead}>Use last read</button
  >
</div>

{#if !words}
  <p class="bad">
    {decimal
      ? "Type whole registers from 0 to 65535 (or −32768 to 32767), separated by spaces."
      : "Type hex with four digits per register, e.g. 4148 0000 or 41 48 00 00."}
  </p>
{:else}
  <table class="data">
    <thead>
      <tr>
        <th></th>
        {#each ORDERS as o (o)}<th class="num">{o}</th>{/each}
      </tr>
    </thead>
    <tbody>
      {#each wide as r (r.label)}
        <tr>
          <td class="label">{r.label}</td>
          {#each r.values as v, i (i)}<td class="num mono">{v}</td>{/each}
        </tr>
      {/each}
    </tbody>
  </table>
  <p class="dim note">
    ABCD is the Modbus standard. CDAB swaps the registers, BADC the bytes in
    each register, DCBA both. 32-bit values use the first two registers, 64-bit
    the first four.
  </p>

  <table class="data">
    <thead>
      <tr>
        <th>#</th>
        <th>Hex</th>
        <th class="num">Unsigned</th>
        <th class="num">Signed</th>
        <th class="num">Bytes swapped</th>
        <th>Binary</th>
        <th>BCD</th>
      </tr>
    </thead>
    <tbody>
      {#each each as w, i (i)}
        <tr>
          <td class="dim">{i}</td>
          <td class="mono">{w.hex}</td>
          <td class="num mono">{w.unsigned}</td>
          <td class="num mono">{w.signed}</td>
          <td class="num mono">{w.swapped}</td>
          <td class="mono">{w.binary}</td>
          <td class="mono">{w.bcd ?? "—"}</td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .grow {
    flex: 1;
    min-width: 240px;
  }

  .grow input {
    width: 100%;
  }

  .label {
    white-space: nowrap;
    color: var(--text-2);
  }

  .note {
    margin: -6px 0 0;
    font-size: 12px;
  }

  p {
    margin: 0;
  }
</style>
