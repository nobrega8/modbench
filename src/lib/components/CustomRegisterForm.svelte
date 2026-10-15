<!--
  Adds a register the open map is missing. It is read from the next cycle
  and kept for this map file on this computer; the file is not changed.
-->
<script lang="ts">
  import {
    customProblem,
    customRegister,
    type CustomFields,
  } from "#lib/custom-registers.ts";
  import { parseInteger } from "#lib/format.ts";
  import { fieldIn } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench, type OpenMap } from "#lib/state.svelte.ts";
  import type { Access, Area, DataType } from "#lib/types.ts";
  import Select from "./Select.svelte";

  let { open, onclose }: { open: OpenMap; onclose: () => void } = $props();

  const AREAS: { value: Area; label: string }[] = [
    { value: "holding", label: "Holding register" },
    { value: "input", label: "Input register" },
    { value: "coil", label: "Coil" },
    { value: "discrete", label: "Discrete input" },
  ];
  const TYPES: DataType[] = [
    "uint16",
    "int16",
    "uint32",
    "int32",
    "float32",
    "uint64",
    "int64",
    "float64",
    "string",
  ];

  let name = $state("");
  let area = $state<Area>("holding");
  let address = $state("");
  let type = $state<DataType>("uint16");
  let length = $state(8);
  let scale = $state(1);
  let offset = $state(0);
  let unit = $state("");
  let access = $state<Access>("R");
  let problem = $state<string | null>(null);

  const bits = $derived(area === "coil" || area === "discrete");
  const writable = $derived(area === "holding" || area === "coil");

  /** The address as typed, in the format the table shows, on the wire. */
  function wire(text: string): number {
    const n = parseInteger(text);
    if (n === null) return NaN;
    return settings.addressFormat === "base1" ? n - 1 : n;
  }

  function add(e: SubmitEvent) {
    e.preventDefault();
    const fields: CustomFields = {
      name,
      area,
      address: wire(address),
      type: bits ? "bool" : type,
      length: type === "string" ? length : null,
      scale: bits ? 1 : Number(scale),
      offset: bits ? 0 : Number(offset),
      unit: bits ? "" : unit,
      access: writable ? access : "R",
    };
    problem = customProblem(fields, open.map.registers);
    if (problem) return;
    bench.addCustomRegister(customRegister(fields));
    onclose();
  }

  function focus(node: HTMLInputElement) {
    node.focus();
  }
</script>

<form class="custom" onsubmit={add} in:fieldIn>
  <div class="head">
    <strong>Add a register</strong>
    <span class="dim"
      >For one the map is missing. It is read from the next cycle and kept for
      this map on this computer; the map file is not changed.</span
    >
  </div>
  <div class="fields">
    <label class="field">
      <span>Name</span>
      <input
        class="input"
        bind:value={name}
        use:focus
        spellcheck="false"
        style:width="180px"
      />
    </label>
    <label class="field">
      <span>Area</span>
      <Select bind:value={area} options={AREAS} label="Area" width="170px" />
    </label>
    <label class="field">
      <span
        >Address{settings.addressFormat === "base1"
          ? " (from 1)"
          : settings.addressFormat === "hex"
            ? " (hex)"
            : ""}</span
      >
      <input
        class="input num"
        bind:value={address}
        spellcheck="false"
        placeholder={settings.addressFormat === "hex" ? "0x0400" : "1024"}
        style:width="100px"
      />
    </label>
    {#if !bits}
      <label class="field">
        <span>Type</span>
        <Select
          bind:value={type}
          options={TYPES.map((t) => ({ value: t, label: t }))}
          label="Type"
          width="110px"
        />
      </label>
      {#if type === "string"}
        <label class="field">
          <span>Length (registers)</span>
          <input
            class="input num"
            type="number"
            min="1"
            bind:value={length}
            style:width="90px"
          />
        </label>
      {:else}
        <label class="field">
          <span>Scale</span>
          <input
            class="input num"
            type="number"
            step="any"
            bind:value={scale}
            style:width="90px"
          />
        </label>
        <label class="field">
          <span>Offset</span>
          <input
            class="input num"
            type="number"
            step="any"
            bind:value={offset}
            style:width="90px"
          />
        </label>
        <label class="field">
          <span>Unit</span>
          <input
            class="input"
            bind:value={unit}
            spellcheck="false"
            style:width="70px"
          />
        </label>
      {/if}
    {/if}
    {#if writable}
      <label class="field">
        <span>Access</span>
        <Select
          bind:value={access}
          options={[
            { value: "R", label: "Read" },
            { value: "RW", label: "Read and write" },
          ]}
          label="Access"
          width="150px"
        />
      </label>
    {/if}
  </div>
  <div class="actions">
    <button class="btn accent">Add</button>
    <button type="button" class="btn" onclick={onclose}>Cancel</button>
    {#if problem}<span class="bad" role="alert">{problem}</span>{/if}
  </div>
</form>

<style>
  .custom {
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
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 10px;
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

  .field > span {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-3);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .bad {
    color: var(--bad);
  }
</style>
