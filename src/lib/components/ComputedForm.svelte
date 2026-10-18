<!--
  Adds a computed value to the open map: a name, a unit and a formula over
  its registers. Kept for this map file on this computer, like the
  registers added by hand; the file is not changed.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { computedRegister } from "#lib/computed.ts";
  import { formulaProblem, plainName } from "#lib/formula.ts";
  import { bench, type OpenMap } from "#lib/state.svelte.ts";

  let { open, onclose }: { open: OpenMap; onclose: () => void } = $props();

  let name = $state("");
  let unit = $state("");
  let formula = $state("");
  let tried = $state(false);

  const names = $derived(open.map.registers.map((r) => r.name));
  const problem = $derived.by(() => {
    if (!name.trim()) return t("computedform.give_the_value_a");
    if (names.some((n) => n.trim().toLowerCase() === name.trim().toLowerCase()))
      return t("computedform.the_map_already_has");
    return formulaProblem(formula, names);
  });
  /** Registers to offer as examples, the way a formula writes them. */
  const sample = $derived(
    open.map.registers
      .filter((r) => r.type !== "string")
      .slice(0, 3)
      .map((r) => plainName(r.name)),
  );

  function add() {
    tried = true;
    if (problem) return;
    bench.addCustomRegister(computedRegister(name, unit, formula));
    onclose();
  }
</script>

<form
  class="computed"
  onsubmit={(e) => {
    e.preventDefault();
    add();
  }}
>
  <div class="head">
    <strong>{t("computedform.add_a_computed_value")}</strong>
    <span class="dim"
      >{t("computedform.worked_out_every_cycle", {
        sample: sample.length ? ` (${sample.join(", ")})` : "",
      })}</span
    >
  </div>
  <div class="fields">
    <label class="field">
      <span>{t("computedform.name")}</span>
      <input class="input" bind:value={name} placeholder="Power L1" />
    </label>
    <label class="field">
      <span>{t("computedform.unit")}</span>
      <input class="input short" bind:value={unit} placeholder="kW" />
    </label>
    <label class="field grow">
      <span>{t("computedform.formula")}</span>
      <input
        class="input mono"
        bind:value={formula}
        spellcheck="false"
        placeholder="Voltage_L1 * Current_L1 / 1000"
      />
    </label>
    <button class="btn accent" type="submit">{t("computedform.add")}</button>
    <button class="btn" type="button" onclick={onclose}
      >{t("computedform.cancel")}</button
    >
  </div>
  {#if problem && (tried || formula.trim())}
    <p class="bad" role="alert">{problem}</p>
  {/if}
</form>

<style>
  .computed {
    white-space: normal;
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 10px;
    padding: 14px;
    border-radius: 10px;
    background: var(--subtle);
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .dim {
    color: var(--text-3);
    font-size: 12px;
  }

  .fields {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 10px 12px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field span {
    font-size: 12px;
    color: var(--text-2);
  }

  .grow {
    flex: 1;
    min-width: 260px;
  }

  .grow input {
    width: 100%;
  }

  .short {
    width: 80px;
  }

  .bad {
    margin: 0;
    color: var(--bad);
  }
</style>
