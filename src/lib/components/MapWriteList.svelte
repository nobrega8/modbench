<script lang="ts">
  import { errorText } from "$lib/api";
  import { AREA_TAG, formatValue, parseForRegister } from "$lib/format";
  import { bench, type OpenMap } from "$lib/state.svelte";
  import type { Register } from "$lib/types";

  let { open }: { open: OpenMap } = $props();

  let search = $state("");
  let drafts = $state<Record<number, string>>({});
  let notes = $state<Record<number, { ok: boolean; text: string }>>({});
  let busy = $state<number | null>(null);

  const rows = $derived(
    open.map.registers
      .map((reg, index) => ({ reg, index }))
      .filter(({ reg }) => reg.access !== "R")
      .filter(({ reg }) => {
        const q = search.trim().toLowerCase();
        return (
          !q ||
          reg.name.toLowerCase().includes(q) ||
          reg.group.toLowerCase().includes(q) ||
          String(reg.address).includes(q)
        );
      }),
  );
  const total = $derived(
    open.map.registers.filter((r) => r.access !== "R").length,
  );

  function boolLabel(reg: Register, on: boolean): string {
    return (
      reg.enum.find(([v]) => v === (on ? 1 : 0))?.[1] ?? (on ? "ON" : "OFF")
    );
  }

  async function write(index: number, text: string) {
    const reg = open.map.registers[index];
    const parsed = parseForRegister(reg, text);
    if ("error" in parsed) {
      notes[index] = { ok: false, text: parsed.error };
      return;
    }
    if (!(await bench.confirmWrite(`Write ${text} to ${reg.name}?`))) return;
    busy = index;
    try {
      await bench.writeMapValue(index, parsed.value);
      notes[index] = { ok: true, text: `Wrote ${text}` };
    } catch (e) {
      notes[index] = { ok: false, text: errorText(e) };
    } finally {
      busy = null;
    }
  }
</script>

<div class="toolbar">
  <span class="count"
    >{total} writable register{total === 1 ? "" : "s"} in {open.map.meta
      .model}</span
  >
  <span class="spacer"></span>
  <input
    class="input search"
    type="search"
    placeholder="Search name or address"
    bind:value={search}
    aria-label="Search writable registers"
  />
</div>

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th class="addr">Address</th>
        <th class="name-col">Name</th>
        <th class="cur">Current value</th>
        <th>New value</th>
      </tr>
    </thead>
    <tbody>
      {#each rows as { reg, index } (index)}
        {@const r = bench.mapResults.get(index)}
        {@const note = notes[index]}
        <tr>
          <td class="addr mono"
            ><span class="area">{AREA_TAG[reg.area]}</span>{reg.address}</td
          >
          <td class="name" title={reg.description || undefined}>
            {reg.name}
            {#if reg.group}<span class="grp">{reg.group}</span>{/if}
            {#if reg.description}<div class="desc">{reg.description}</div>{/if}
          </td>
          <td class="cur mono">
            {#if reg.access === "W"}<span class="dim">write only</span
              >{:else if r?.sentinel}<span class="dim">{r.sentinel}</span
              >{:else}{formatValue(reg, r?.value ?? null)}
              {#if r?.value !== null && r?.value !== undefined && reg.unit}<span
                  class="dim">{reg.unit}</span
                >{/if}{/if}
          </td>
          <td>
            <form
              class="edit"
              onsubmit={(e) => {
                e.preventDefault();
                write(index, drafts[index] ?? "");
              }}
            >
              {#if reg.type === "bool"}
                {#each [true, false] as on (on)}
                  <button
                    type="button"
                    class="btn accent"
                    disabled={busy !== null}
                    onclick={() => write(index, on ? "1" : "0")}
                    >{boolLabel(reg, on)}</button
                  >
                {/each}
              {:else if reg.enum.length}
                <select
                  class="input wide"
                  bind:value={drafts[index]}
                  aria-label="Value for {reg.name}"
                >
                  <option value={undefined} disabled selected>Choose…</option>
                  {#each reg.enum as [v, l] (v)}<option value={l}>{l}</option
                    >{/each}
                </select>
                <button
                  class="btn accent"
                  disabled={busy !== null || !drafts[index]}>Write</button
                >
              {:else}
                <input
                  class="input num"
                  bind:value={drafts[index]}
                  spellcheck="false"
                  placeholder={reg.type === "string" ? "Text" : "Value"}
                  aria-label="Value for {reg.name}"
                />
                {#if reg.unit}<span class="dim">{reg.unit}</span>{/if}
                <button
                  class="btn accent"
                  disabled={busy !== null || !drafts[index]}>Write</button
                >
              {/if}
              {#if busy === index}<span class="dim">Writing…</span>{/if}
              {#if note}<span class:good={note.ok} class:bad={!note.ok}
                  >{note.text}</span
                >{/if}
            </form>
          </td>
        </tr>
      {:else}
        <tr>
          <td colspan="4" class="empty">
            {total
              ? "No writable registers match this search."
              : "This map has no writable registers."}
          </td>
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
  }

  .count {
    color: var(--text-2);
  }

  .spacer {
    flex: 1;
  }

  .search {
    width: 240px;
  }

  .table-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-radius: 10px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--card);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: left;
    padding: 6px 10px;
  }

  td {
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }

  .addr {
    width: 104px;
    white-space: nowrap;
  }

  .name-col {
    width: 32%;
  }

  .cur {
    width: 180px;
  }

  .mono {
    font-family: var(--mono);
    font-size: 13px;
  }

  .area {
    display: inline-block;
    width: 24px;
    margin-right: 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
  }

  .name {
    font-weight: 600;
  }

  .grp {
    margin-left: 8px;
    font-weight: 400;
    font-size: 12px;
    color: var(--text-2);
  }

  .desc {
    margin-top: 2px;
    font-weight: 400;
    font-size: 12px;
    color: var(--text-3);
  }

  .edit {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .num {
    width: 140px;
  }

  .wide {
    min-width: 260px;
    max-width: 420px;
  }

  .dim {
    color: var(--text-3);
  }

  .good {
    color: var(--ok);
    font-weight: 600;
  }

  .bad {
    color: var(--bad);
  }

  .empty {
    color: var(--text-2);
    padding: 16px 10px;
  }
</style>
