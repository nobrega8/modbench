<!--
  Values the simulator changes on its own: a list of generators, and a form
  to add or edit one. They run while the simulator does.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { formatAddress } from "#lib/format.ts";
  import {
    AREA_LABELS,
    KINDS,
    describe,
    draftFor,
    fromDraft,
    newDraft,
    registerOf as findRegister,
    toDraft,
    type Draft,
    type TargetDraft,
  } from "#lib/generators.ts";
  import { fieldIn, slidePill } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { sim } from "#lib/simulator.svelte.ts";
  import type { Area, GeneratorTarget, Register } from "#lib/types.ts";
  import { tick } from "svelte";
  import AddressInput from "../AddressInput.svelte";
  import IntInput from "../IntInput.svelte";
  import Select from "../Select.svelte";
  import RegisterInfo from "./RegisterInfo.svelte";

  /** The generator being edited: its index, "new", or null for none. */
  let editing = $state<number | "new" | null>(null);
  let draft = $state<Draft>(newDraft([]));
  let editor: HTMLDivElement | undefined = $state();

  const areas = (Object.keys(AREA_LABELS) as Area[]).map((a) => ({
    value: a,
    label: AREA_LABELS[a],
  }));
  const registerOptions = $derived(
    sim.registers.map((r) => ({
      value: r.name,
      label: r.unit ? `${r.name} (${r.unit})` : r.name,
    })),
  );

  const registerOf = (t: GeneratorTarget) => findRegister(t, sim.registers);

  function name(t: GeneratorTarget) {
    if ("register" in t) return t.register;
    const where = `${AREA_LABELS[t.area]} ${formatAddress(t.address, settings.addressFormat, t.area)}`;
    const r = registerOf(t);
    return r ? `${where} · ${r.name}` : where;
  }

  function draftTarget(t: TargetDraft): GeneratorTarget {
    return t.mode === "register"
      ? { register: t.register }
      : { area: t.area, address: t.address };
  }

  /** The map register the form writes to as a whole, if any. */
  const drafted = $derived(registerOf(draftTarget(draft.to)));
  const unit = $derived(drafted?.unit ?? "");

  function add() {
    draft = newDraft(sim.registers);
    editing = "new";
  }

  function edit(i: number) {
    draft = toDraft(sim.generators[i], sim.registers);
    editing = i;
  }

  /** Opens the form for a register: its generator if it has one (`index`),
   * else a new one shaped by what the map says about it. */
  export async function generateFor(r: Register, index: number | null) {
    if (index !== null) edit(index);
    else {
      draft = draftFor(r, sim.registers);
      editing = "new";
    }
    await tick();
    editor?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }

  function save() {
    const g = fromDraft($state.snapshot(draft));
    const list = [...sim.generators];
    if (editing === "new") list.push(g);
    else if (editing !== null) list[editing] = g;
    sim.setGenerators(list);
    editing = null;
  }

  function remove(i: number) {
    sim.setGenerators(sim.generators.filter((_, j) => j !== i));
    if (editing === i) editing = null;
    else if (typeof editing === "number" && editing > i) editing--;
  }
</script>

{#snippet target(draft: TargetDraft, label: string)}
  <div class="target">
    <div
      class="segmented small"
      role="radiogroup"
      aria-label={t("simgenerators.label_register_or_address", { label })}
      use:slidePill
    >
      <button
        role="radio"
        aria-checked={draft.mode === "register"}
        class:on={draft.mode === "register"}
        disabled={!sim.registers.length}
        title={sim.registers.length
          ? undefined
          : t("simgenerators.load_a_map_first")}
        onclick={() => (draft.mode = "register")}
        >{t("simgenerators.map_register")}</button
      >
      <button
        role="radio"
        aria-checked={draft.mode === "address"}
        class:on={draft.mode === "address"}
        onclick={() => (draft.mode = "address")}>{t("req.address")}</button
      >
    </div>
    {#if draft.mode === "register"}
      <Select
        bind:value={draft.register}
        options={registerOptions}
        {label}
        width="240px"
      />
    {:else}
      <Select
        bind:value={draft.area}
        options={areas}
        label="{label} area"
        width="120px"
      />
      <AddressInput
        value={draft.address}
        width="90px"
        label="{label} address"
        onchange={(n, a) => {
          draft.address = n;
          if (a) draft.area = a;
        }}
      />
    {/if}
  </div>
{/snippet}

<section class="generators">
  <div class="row head">
    <h2 class="key">{t("simgenerators.generators")}</h2>
    <span class="dim">{t("simgenerators.values_that_change_on")}</span>
    <span class="spacer"></span>
    <button class="btn" disabled={editing === "new"} onclick={add}
      >{t("simgenerators.add_generator")}</button
    >
  </div>

  {#if sim.generators.length}
    <table class="data">
      <thead>
        <tr>
          <th>{t("maptable.value")}</th><th>{t("simgenerators.kind")}</th><th
            >{t("simgenerators.changes")}</th
          ><th class="num">{t("simgenerators.every")}</th><th></th>
        </tr>
      </thead>
      <tbody>
        {#each sim.generators as g, i (i)}
          {@const problem = sim.problems[i]}
          <tr class:editing={editing === i}>
            <td class="name">{name(g)}</td>
            <td>{KINDS.find((k) => k.id === g.kind)?.label}</td>
            <td>
              {#if problem}
                <span class="bad" title={problem}>{problem}</span>
              {:else}
                {describe(g, name, registerOf(g)?.unit)}
              {/if}
            </td>
            <td class="num mono">{g.every_ms} ms</td>
            <td class="act">
              <button class="btn ghost small" onclick={() => edit(i)}
                >{t("libraryview.edit")}</button
              >
              <button class="btn ghost small" onclick={() => remove(i)}
                >{t("maptable.remove")}</button
              >
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if editing === null}
    <p class="empty">{t("simgenerators.none_yet_add_one")}</p>
  {/if}

  {#if editing !== null}
    <div class="editor" bind:this={editor} in:fieldIn>
      <div class="field">
        <span>{t("simgenerators.value_to_change")}</span>
        {@render target(draft.to, "Value")}
      </div>
      {#if drafted}
        <div class="about">
          <RegisterInfo register={drafted} />
        </div>
      {/if}

      <div class="row">
        <div class="field">
          <span>{t("simgenerators.kind")}</span>
          <div
            class="segmented"
            role="radiogroup"
            aria-label={t("simgenerators.kind")}
            use:slidePill
          >
            {#each KINDS as k (k.id)}
              <button
                role="radio"
                aria-checked={draft.kind === k.id}
                class:on={draft.kind === k.id}
                onclick={() => (draft.kind = k.id)}>{k.label}</button
              >
            {/each}
          </div>
        </div>
        <label class="field">
          <span>{t("simgenerators.every_ms")}</span>
          <IntInput
            value={draft.every_ms}
            min={50}
            max={3_600_000}
            width="100px"
            label={t("simgenerators.every_in_milliseconds")}
            onchange={(n) => (draft.every_ms = n)}
          />
        </label>
      </div>

      {#key draft.kind}
        <div class="row params" in:fieldIn>
          {#if draft.kind === "sine"}
            <label class="field">
              <span>{t("simgenerators.centre")}{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.offset}
              />
            </label>
            <label class="field">
              <span>{t("simgenerators.swing")}{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.amplitude}
              />
            </label>
            <label class="field">
              <span>{t("simgenerators.period_s")}</span>
              <input
                class="input num"
                type="number"
                step="any"
                min="0"
                bind:value={draft.period_s}
              />
            </label>
          {:else if draft.kind === "increment"}
            <label class="field">
              <span>{t("simgenerators.step")}{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.step}
              />
            </label>
            <label class="field">
              <span>{t("scanpanel.from")}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.min}
              />
            </label>
            <label class="field">
              <span>{t("simgenerators.to_then_wraps")}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.max}
              />
            </label>
          {:else if draft.kind === "random"}
            <label class="field">
              <span>{t("simgenerators.lowest")}{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.min}
              />
            </label>
            <label class="field">
              <span>{t("simgenerators.highest")}{unit ? ` (${unit})` : ""}</span
              >
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.max}
              />
            </label>
          {:else}
            <div class="field">
              <span>{t("simgenerators.copy_from")}</span>
              {@render target(draft.from, "Copy from")}
            </div>
          {/if}
        </div>
      {/key}

      <p class="dim note">
        {drafted
          ? t("simgenerators.in_name_s_own", {
              name: drafted.name,
              type: drafted.type,
            })
          : t("simgenerators.raw_a_bit_is")}
      </p>

      <div class="row actions">
        <button class="btn accent" onclick={save}
          >{editing === "new"
            ? t("computedform.add")
            : t("simgenerators.save")}</button
        >
        <button class="btn" onclick={() => (editing = null)}
          >{t("computedform.cancel")}</button
        >
      </div>
    </div>
  {/if}
</section>

<style>
  p {
    margin: 0;
  }

  .spacer {
    flex: 1;
  }

  .generators {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .head {
    align-items: center;
  }

  .head h2 {
    margin: 0;
  }

  .name {
    font-weight: 600;
  }

  .act {
    text-align: right;
    white-space: nowrap;
  }

  tr.editing td {
    background: var(--accent-soft);
  }

  .small {
    height: 28px;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px;
    border-radius: 10px;
    background: var(--subtle);
  }

  .about {
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--card);
  }

  .target {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .params input {
    width: 120px;
  }

  .note {
    font-size: 12px;
  }

  .empty {
    color: var(--text-2);
  }
</style>
