<script module lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  // Kept while other tools are shown.
  let input = $state("4148 0000");
  let decimal = $state(false);
</script>

<script lang="ts">
  import { ORDERS, parseWords, wideRows, wordRows } from "#lib/decoder.ts";
  import { slidePill } from "#lib/motion.ts";
  import { bench } from "#lib/state.svelte.ts";

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
  <h1>{t("tools.decoder")}</h1>
  <p>{t("datadecodertool.registers_as_every_type")}</p>
</header>

<div class="row">
  <label class="field grow">
    <span>{t("libraryview.registers")}</span>
    <input
      class="input num"
      bind:value={input}
      spellcheck="false"
      placeholder={decimal ? "16712 0" : "4148 0000"}
      aria-invalid={!words}
    />
  </label>
  <div class="field">
    <span>{t("datadecodertool.typed_as")}</span>
    <div
      class="segmented"
      role="radiogroup"
      aria-label={t("datadecodertool.typed_as")}
      use:slidePill
    >
      <button
        role="radio"
        aria-checked={!decimal}
        class:on={!decimal}
        onclick={() => (decimal = false)}>{t("registergrid.hex")}</button
      >
      <button
        role="radio"
        aria-checked={decimal}
        class:on={decimal}
        onclick={() => (decimal = true)}>{t("datadecodertool.decimal")}</button
      >
    </div>
  </div>
  <button
    class="btn"
    disabled={bench.read?.data.kind !== "registers"}
    title={t("datadecodertool.use_the_registers_last")}
    onclick={fromRead}>{t("datadecodertool.use_last_read")}</button
  >
</div>

{#if !words}
  <p class="bad">
    {decimal
      ? "Registers 0–65535 (or −32768–32767), separated by spaces."
      : "Hex, four digits per register, e.g. 4148 0000."}
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
    {t("datadecodertool.abcd_is_standard_cdab")}
  </p>

  <table class="data">
    <thead>
      <tr>
        <th>#</th>
        <th>{t("registergrid.hex")}</th>
        <th class="num">{t("datadecodertool.unsigned")}</th>
        <th class="num">{t("datadecodertool.signed")}</th>
        <th class="num">{t("datadecodertool.bytes_swapped")}</th>
        <th>{t("datadecodertool.binary")}</th>
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
