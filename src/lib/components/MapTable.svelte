<script lang="ts">
  import { errorText } from "$lib/api";
  import {
    AREA_TAG,
    AREA_NAME,
    formatRaw,
    formatValue,
    parseForRegister,
  } from "$lib/format";
  import { bench, type OpenMap } from "$lib/state.svelte";
  import type { Register } from "$lib/types";

  let { open }: { open: OpenMap } = $props();

  let search = $state("");
  let group = $state("");
  let editing = $state<number | null>(null);
  let draft = $state("");
  let note = $state<{ index: number; ok: boolean; text: string } | null>(null);
  let busy = $state(false);

  const meta = $derived(open.map.meta);
  const base = $derived(meta.address_base ?? 0);
  const groups = $derived([
    ...new Set(open.map.registers.map((r) => r.group).filter(Boolean)),
  ]);
  const rows = $derived(
    open.map.registers
      .map((reg, index) => ({ reg, index }))
      .filter(({ reg }) => {
        if (group && reg.group !== group) return false;
        const q = search.trim().toLowerCase();
        return (
          !q ||
          reg.name.toLowerCase().includes(q) ||
          reg.description.toLowerCase().includes(q) ||
          String(reg.address).includes(q)
        );
      })
      // Registers that are not read go to the end, in map order.
      .sort(
        (a, b) =>
          Number(bench.skipped.has(a.index)) -
            Number(bench.skipped.has(b.index)) || a.index - b.index,
      ),
  );
  const readCount = $derived(open.map.registers.length - bench.skipped.size);

  function typeLabel(reg: Register): string {
    if (reg.type === "string") return `string[${reg.length}]`;
    if (reg.type === "bool" && reg.bit !== null) return `bit ${reg.bit}`;
    const wide = !["bool", "int16", "uint16"].includes(reg.type);
    const order =
      bench.orderOverride ?? reg.byte_order ?? meta.byte_order ?? "ABCD";
    return wide ? `${reg.type} ${order}` : reg.type;
  }

  function edit(index: number) {
    editing = index;
    note = null;
    const reg = open.map.registers[index];
    const v = bench.mapResults.get(index)?.value;
    draft = v === null || v === undefined ? "" : formatValue(reg, v);
  }

  async function write(index: number, text: string) {
    const reg = open.map.registers[index];
    const parsed = parseForRegister(reg, text);
    if ("error" in parsed) {
      note = { index, ok: false, text: parsed.error };
      return;
    }
    busy = true;
    try {
      await bench.writeMapValue(index, parsed.value);
      note = { index, ok: true, text: `Wrote ${text} to ${reg.name}.` };
      editing = null;
    } catch (e) {
      note = { index, ok: false, text: `Not written: ${errorText(e)}` };
    } finally {
      busy = false;
    }
  }
</script>

<div class="toolbar">
  <div class="title">
    <strong>{meta.vendor} {meta.model}</strong>
    {#if meta.description}<span class="desc">{meta.description}</span>{/if}
  </div>
  <span class="spacer"></span>
  <span class="count">Reading {readCount} of {open.map.registers.length}</span>
  <button
    class="btn small"
    disabled={readCount === open.map.registers.length}
    onclick={() => bench.setAllRead(true)}>Read all</button
  >
  <button
    class="btn small"
    disabled={readCount === 0}
    onclick={() => bench.setAllRead(false)}>Read none</button
  >
  <input
    class="input search"
    type="search"
    placeholder="Search name or address"
    bind:value={search}
    aria-label="Search registers"
  />
  {#if groups.length}
    <select class="input" bind:value={group} aria-label="Group">
      <option value="">All groups</option>
      {#each groups as g (g)}<option value={g}>{g}</option>{/each}
    </select>
  {/if}
</div>

{#if open.warnings.length}
  <p class="warnings">
    The map file has {open.warnings.length} warning{open.warnings.length === 1
      ? ""
      : "s"}:
    {open.warnings
      .map((w) => (w.line ? `line ${w.line}: ${w.message}` : w.message))
      .join("; ")}.
  </p>
{/if}

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th class="read">Read</th>
        <th class="addr">Address</th>
        <th>Name</th>
        <th>Type</th>
        <th class="val">Value</th>
        <th>Unit</th>
        <th>Raw</th>
        <th class="act"></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as { reg, index } (index)}
        {@const r = bench.mapResults.get(index)}
        {@const off = bench.skipped.has(index)}
        <tr class:err={!!r?.error} class:off>
          <td class="read">
            {#if reg.access !== "W"}
              <input
                type="checkbox"
                class="switch"
                role="switch"
                checked={!off}
                aria-label="Read {reg.name}"
                title={off ? "Not read. Switch on to read it." : "Read"}
                onchange={() => bench.toggleRead(index)}
              />
            {/if}
          </td>
          <td
            class="addr mono"
            title="{AREA_NAME[reg.area]} {reg.address}{base
              ? ` (${reg.address + base} in the manual)`
              : ''}"
            ><span class="area">{AREA_TAG[reg.area]}</span>{reg.address}</td
          >
          <td class="name" title={reg.description || undefined}
            >{reg.name}{#if reg.group && !group}<span class="grp"
                >{reg.group}</span
              >{/if}</td
          >
          <td class="type mono">{typeLabel(reg)}</td>
          <td class="val mono">
            {#if reg.access === "W"}<span class="dim">write only</span
              >{:else}{formatValue(reg, r?.value ?? null)}{/if}
          </td>
          <td class="unit">{reg.unit}</td>
          <td class="raw mono" title={r?.error ?? undefined}>
            {#if r?.error}<span class="bad">{r.error}</span>{:else}{formatRaw(
                r?.raw ?? null,
              )}{/if}
          </td>
          <td class="act">
            {#if reg.access !== "R"}
              <button class="btn small" onclick={() => edit(index)}
                >Write…</button
              >
            {/if}
          </td>
        </tr>
        {#if editing === index}
          <tr class="editor">
            <td colspan="2"></td>
            <td colspan="6">
              <form
                onsubmit={(e) => {
                  e.preventDefault();
                  write(index, draft);
                }}
              >
                <span>New value for <strong>{reg.name}</strong></span>
                {#if reg.type === "bool"}
                  {#each [true, false] as on (on)}
                    {@const label =
                      reg.enum.find(([v]) => v === (on ? 1 : 0))?.[1] ??
                      (on ? "ON (1)" : "OFF (0)")}
                    <button
                      type="button"
                      class="btn accent"
                      disabled={busy}
                      onclick={() => write(index, on ? "1" : "0")}
                      >{label}</button
                    >
                  {/each}
                {:else if reg.enum.length}
                  <select class="input" bind:value={draft}>
                    {#each reg.enum as [v, l] (v)}<option value={l}
                        >{l} ({v})</option
                      >{/each}
                  </select>
                  <button class="btn accent" disabled={busy}>Write</button>
                {:else}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="input num"
                    bind:value={draft}
                    autofocus
                    spellcheck="false"
                    aria-label="New value"
                  />
                  {#if reg.unit}<span class="dim">{reg.unit}</span>{/if}
                  <button class="btn accent" disabled={busy}>Write</button>
                {/if}
                <button
                  type="button"
                  class="btn"
                  onclick={() => (editing = null)}>Cancel</button
                >
                {#if note?.index === index && !note.ok}<span class="bad"
                    >{note.text}</span
                  >{/if}
              </form>
            </td>
          </tr>
        {:else if note?.index === index && note.ok}
          <tr class="done"
            ><td colspan="2"></td><td colspan="6">{note.text}</td></tr
          >
        {/if}
      {:else}
        <tr>
          <td colspan="8" class="empty"
            >{open.map.registers.length
              ? "No registers match this search."
              : "This map has no registers."}</td
          >
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 10px;
    flex-wrap: wrap;
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .title strong {
    font-size: 15px;
  }

  .desc,
  .dim {
    color: var(--text-3);
  }

  .spacer {
    flex: 1;
  }

  .search {
    width: 220px;
  }

  .warnings {
    margin: 0 0 8px;
    color: var(--warn);
    font-size: 13px;
  }

  .table-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 4px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--subtle);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    text-align: left;
    padding: 6px 10px;
  }

  td {
    padding: 5px 10px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  tbody tr:hover td {
    background: var(--accent-soft);
  }

  .mono {
    font-family: var(--mono);
    font-size: 13px;
  }

  .addr {
    width: 100px;
  }

  .read {
    width: 52px;
    text-align: center;
  }

  /* Small toggle switch */
  .switch {
    appearance: none;
    position: relative;
    width: 30px;
    height: 16px;
    margin: 0;
    border-radius: 8px;
    background: var(--border-strong);
    cursor: pointer;
    vertical-align: middle;
    transition: background 0.15s;
  }

  .switch::after {
    content: "";
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s;
  }

  .switch:checked {
    background: var(--accent);
  }

  .switch:checked::after {
    transform: translateX(14px);
  }

  /* Registers that are not read: greyed out, at the end of the list */
  tr.off td:not(.read) {
    opacity: 0.4;
  }

  .count {
    color: var(--text-2);
    font-size: 13px;
  }

  .area {
    display: inline-block;
    width: 24px;
    margin-right: 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--accent);
  }

  .name {
    font-weight: 600;
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .grp {
    margin-left: 8px;
    font-weight: 400;
    font-size: 12px;
    color: var(--text-3);
  }

  .type {
    color: var(--text-2);
    font-size: 12px;
  }

  .val {
    text-align: right;
    font-size: 14px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .unit {
    color: var(--text-2);
    width: 60px;
  }

  .raw {
    color: var(--text-3);
    font-size: 12px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .act {
    width: 80px;
    text-align: right;
  }

  .small {
    height: 24px;
    padding: 0 8px;
    font-size: 12px;
  }

  .bad {
    color: var(--bad);
  }

  .editor td {
    background: var(--accent-soft);
  }

  .editor form {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .editor .input {
    width: 160px;
  }

  .done td {
    color: var(--ok);
    font-weight: 600;
  }

  .empty {
    color: var(--text-2);
    padding: 16px 10px;
  }
</style>
