<!--
  Values the simulator changes on its own: a list of generators, and a form
  to add or edit one. They run while the simulator does.
-->
<script lang="ts">
  import { formatAddress } from "$lib/format";
  import {
    AREA_LABELS,
    KINDS,
    describe,
    fromDraft,
    newDraft,
    registerOf as findRegister,
    toDraft,
    type Draft,
    type TargetDraft,
  } from "$lib/generators";
  import { fieldIn, slidePill } from "$lib/motion";
  import { settings } from "$lib/settings.svelte";
  import { sim } from "$lib/simulator.svelte";
  import type { Area, GeneratorTarget } from "$lib/types";
  import IntInput from "../IntInput.svelte";
  import Select from "../Select.svelte";

  /** The generator being edited: its index, "new", or null for none. */
  let editing = $state<number | "new" | null>(null);
  let draft = $state<Draft>(newDraft([]));

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
    const where = `${AREA_LABELS[t.area]} ${formatAddress(t.address, settings.addressFormat)}`;
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

{#snippet target(t: TargetDraft, label: string)}
  <div class="target">
    <div
      class="segmented small"
      role="radiogroup"
      aria-label="{label}: register or address"
      use:slidePill
    >
      <button
        role="radio"
        aria-checked={t.mode === "register"}
        class:on={t.mode === "register"}
        disabled={!sim.registers.length}
        title={sim.registers.length ? undefined : "Load a map first"}
        onclick={() => (t.mode = "register")}>Map register</button
      >
      <button
        role="radio"
        aria-checked={t.mode === "address"}
        class:on={t.mode === "address"}
        onclick={() => (t.mode = "address")}>Address</button
      >
    </div>
    {#if t.mode === "register"}
      <Select
        bind:value={t.register}
        options={registerOptions}
        {label}
        width="240px"
      />
    {:else}
      <Select
        bind:value={t.area}
        options={areas}
        label="{label} area"
        width="120px"
      />
      <IntInput
        value={t.address}
        min={0}
        max={65535}
        width="90px"
        label="{label} address"
        onchange={(n) => (t.address = n)}
      />
    {/if}
  </div>
{/snippet}

<section class="generators">
  <div class="row head">
    <h2 class="key">Generators</h2>
    <span class="dim"
      >Values that change on their own while the simulator runs.</span
    >
    <span class="spacer"></span>
    <button class="btn" disabled={editing === "new"} onclick={add}
      >Add generator</button
    >
  </div>

  {#if sim.generators.length}
    <table class="data">
      <thead>
        <tr>
          <th>Value</th><th>Kind</th><th>Changes</th><th class="num">Every</th
          ><th></th>
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
                >Edit</button
              >
              <button class="btn ghost small" onclick={() => remove(i)}
                >Remove</button
              >
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else if editing === null}
    <p class="empty">
      None yet. Add one to make a value move, e.g. a voltage that swings around
      230 V, or a status that follows a coil.
    </p>
  {/if}

  {#if editing !== null}
    <div class="editor" in:fieldIn>
      <div class="field">
        <span>Value to change</span>
        {@render target(draft.to, "Value")}
      </div>

      <div class="row">
        <div class="field">
          <span>Kind</span>
          <div
            class="segmented"
            role="radiogroup"
            aria-label="Kind"
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
          <span>Every (ms)</span>
          <IntInput
            value={draft.every_ms}
            min={50}
            max={3_600_000}
            width="100px"
            label="Every, in milliseconds"
            onchange={(n) => (draft.every_ms = n)}
          />
        </label>
      </div>

      {#key draft.kind}
        <div class="row params" in:fieldIn>
          {#if draft.kind === "sine"}
            <label class="field">
              <span>Centre{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.offset}
              />
            </label>
            <label class="field">
              <span>Swing ±{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.amplitude}
              />
            </label>
            <label class="field">
              <span>Period (s)</span>
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
              <span>Step{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.step}
              />
            </label>
            <label class="field">
              <span>From</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.min}
              />
            </label>
            <label class="field">
              <span>To, then wraps</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.max}
              />
            </label>
          {:else if draft.kind === "random"}
            <label class="field">
              <span>Lowest{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.min}
              />
            </label>
            <label class="field">
              <span>Highest{unit ? ` (${unit})` : ""}</span>
              <input
                class="input num"
                type="number"
                step="any"
                bind:value={draft.max}
              />
            </label>
          {:else}
            <div class="field">
              <span>Copy from</span>
              {@render target(draft.from, "Copy from")}
            </div>
          {/if}
        </div>
      {/key}

      <p class="dim note">
        {drafted
          ? `In ${drafted.name}'s own units: its type (${drafted.type}), scale and offset are applied.`
          : "Raw: a bit is 0 or 1, a register a whole number (negative ones are stored as int16)."}
      </p>

      <div class="row actions">
        <button class="btn accent" onclick={save}
          >{editing === "new" ? "Add" : "Save"}</button
        >
        <button class="btn" onclick={() => (editing = null)}>Cancel</button>
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
