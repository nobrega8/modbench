<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "$lib/api";
  import { valuesCsv } from "$lib/csv";
  import { settings } from "$lib/settings.svelte";
  import {
    AREA_TAG,
    AREA_NAME,
    formatAddress,
    formatRaw,
    formatValue,
    parseForRegister,
    limitError,
  } from "$lib/format";
  import { bench, type OpenMap } from "$lib/state.svelte";
  import { diagnosticWhere, type Register } from "$lib/types";
  import AddressFormat from "./AddressFormat.svelte";

  let { open }: { open: OpenMap } = $props();

  let search = $state("");
  let group = $state("");
  let editing = $state<number | null>(null);
  let draft = $state("");
  let note = $state<{ index: number; ok: boolean; text: string } | null>(null);
  let busy = $state(false);
  let converting = $state(false);
  let convertError = $state<string | null>(null);

  async function convert() {
    const index = bench.maps.indexOf(open);
    if (index < 0) return;
    converting = true;
    convertError = null;
    try {
      await bench.convertMap(index);
    } catch (e) {
      convertError = errorText(e);
    } finally {
      converting = false;
    }
  }

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
          String(reg.address).includes(q) ||
          formatAddress(reg.address, settings.addressFormat)
            .toLowerCase()
            .includes(q)
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

  /** Outcome of the map's reference checks (spec 4.6), from the last cycle. */
  const refs = $derived.by(() => {
    const out = { total: 0, match: 0, mismatch: 0, failed: 0 };
    open.map.registers.forEach((reg, i) => {
      if (!reg.reference) return;
      out.total++;
      const r = bench.mapResults.get(i);
      if (bench.skipped.has(i) || !r) return;
      if (r.error) out.failed++;
      else if (r.reference === "match") out.match++;
      else if (r.reference === "mismatch") out.mismatch++;
    });
    return out;
  });

  let exportNote = $state<{ ok: boolean; text: string } | null>(null);
  const recording = $derived(
    bench.recording?.mapPath === open.path ? bench.recording : null,
  );

  function csvName(suffix: string) {
    const name = `${meta.vendor}-${meta.model}`.replace(/[^\w.-]+/g, "_");
    return `${name}-${suffix}.csv`;
  }

  /** Saves the values on screen, in the order shown. */
  async function exportValues() {
    exportNote = null;
    const path = await save({
      defaultPath: csvName("values"),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      await api.saveText(
        path,
        valuesCsv(
          open.map.registers,
          rows.map((r) => r.index),
          bench.mapResults,
          settings.csvSeparator,
        ),
      );
      exportNote = { ok: true, text: "Saved." };
    } catch (e) {
      exportNote = { ok: false, text: `Not saved: ${errorText(e)}` };
    }
  }

  async function toggleRecording() {
    if (recording) {
      bench.stopRecording();
      return;
    }
    const path = await save({
      defaultPath: csvName("recording"),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (path) await bench.startRecording(path);
  }

  function typeLabel(reg: Register): string {
    if (reg.type === "string") return `string[${reg.length}]`;
    if (reg.type === "bool" && reg.bit !== null) return `bit ${reg.bit}`;
    if (reg.bits !== null && reg.bit !== null)
      return reg.bits === 1
        ? `bit ${reg.bit}`
        : `bits ${reg.bit}–${reg.bit + reg.bits - 1}`;
    const wide = !["bool", "int16", "uint16"].includes(reg.type);
    const order =
      bench.orderOverride ?? reg.byte_order ?? meta.byte_order ?? "ABCD";
    const base = wide ? `${reg.type} ${order}` : reg.type;
    return reg.scale_factor ? `${base} ×10^SF` : base;
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
    const outside = limitError(reg, parsed.value);
    if (outside) {
      note = { index, ok: false, text: outside };
      return;
    }
    if (!(await bench.confirmWrite(`Write ${text} to ${reg.name}?`))) return;
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
    {#if meta.description}<span class="desc" title={meta.description}
        >{meta.description}</span
      >{/if}
  </div>
  {#if meta.extra.some(([k, v]) => k === "x_status" && v === "unverified")}
    <span
      class="refs bad"
      title="This map was converted from another catalogue and has not been checked against the manufacturer's documentation. Check a value on the device before relying on the rest."
      >Unverified</span
    >
  {/if}
  {#if refs.total}
    <span
      class="refs"
      class:bad={refs.mismatch > 0 || refs.failed > 0}
      title="Registers with a known value in the map. A mismatch points to a wrong map or wrong settings (address, byte order, type); a failed read to a connection or address problem."
      >Reference {refs.match}/{refs.total}{#if refs.mismatch}
        · {refs.mismatch} differ{/if}{#if refs.failed}
        · {refs.failed} failed{/if}</span
    >
  {/if}
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
  <button
    class="btn small"
    title="Save the values shown to a CSV file"
    disabled={!bench.mapResults.size}
    onclick={exportValues}>Export…</button
  >
  <button
    class="btn small"
    class:rec={recording}
    title={recording
      ? `Recording to ${recording.path}. Click to stop.`
      : "Add one row per reading cycle to a CSV file, with a column for each register that is read now"}
    disabled={!recording && readCount === 0}
    onclick={toggleRecording}
    >{recording ? `Stop recording (${recording.rows})` : "Record…"}</button
  >
  {#if exportNote}<span class:bad={!exportNote.ok} class="note"
      >{exportNote.text}</span
    >{/if}
  {#if bench.recordError}<span class="bad note" title={bench.recordError}
      >Recording stopped: {bench.recordError}</span
    >{/if}
  <input
    class="input search"
    type="search"
    placeholder="Search name or address"
    bind:value={search}
    aria-label="Search registers"
  />
  <AddressFormat />
  {#if groups.length}
    <select class="input" bind:value={group} aria-label="Group">
      <option value="">All groups</option>
      {#each groups as g (g)}<option value={g}>{g}</option>{/each}
    </select>
  {/if}
</div>

{#if open.version === 1}
  <div class="deprecated">
    <span
      >This map uses format version 1 (CSV), which is deprecated. Convert it to
      version 2 (JSON); the original file is kept.</span
    >
    <button class="btn" disabled={converting} onclick={convert}
      >{converting ? "Converting…" : "Convert to version 2"}</button
    >
    {#if convertError}<span class="error">{convertError}</span>{/if}
  </div>
{/if}

{#if open.warnings.length}
  <details class="warnings">
    <summary
      >{open.warnings.length} warning{open.warnings.length === 1 ? "" : "s"} in the
      map file</summary
    >
    <ul>
      {#each open.warnings as w, k (k)}
        <li>
          <span class="where">{diagnosticWhere(w)}</span>
          {w.message}
        </li>
      {/each}
    </ul>
  </details>
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
        {@const shown =
          reg.access === "W" || r?.sentinel
            ? null
            : formatValue(reg, r?.value ?? null)}
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
            title="{AREA_NAME[reg.area]} {reg.address} on the wire{base
              ? ` (${reg.address + base} in the manual)`
              : ''}"
            ><span class="area">{AREA_TAG[reg.area]}</span>{formatAddress(
              reg.address,
              settings.addressFormat,
            )}</td
          >
          <td class="name" title={reg.description || undefined}
            >{reg.name}{#if reg.group && !group}<span class="grp"
                >{reg.group}</span
              >{/if}{#if reg.reference}<span
                class="refmark"
                class:ok={r?.reference === "match"}
                class:bad={r?.reference === "mismatch"}
                title={r?.reference === "match"
                  ? `Matches the reference value ${reg.reference}`
                  : r?.reference === "mismatch"
                    ? `Differs from the reference value ${reg.reference}`
                    : `Reference value ${reg.reference}`}>ref</span
              >{/if}</td
          >
          <td
            class="type mono"
            title={reg.scale_factor
              ? `Scaled by 10 to the power of ${reg.scale_factor}`
              : undefined}>{typeLabel(reg)}</td
          >
          <td
            class="val mono"
            class:changed={bench.fresh(`m:${index}`)}
            title={shown || undefined}
          >
            {#if reg.access === "W"}<span class="dim">write only</span
              >{:else if r?.sentinel}<span
                class="sentinel"
                title="The device returned a reserved value: {r.sentinel}"
                >{r.sentinel}</span
              >{:else}{shown}{/if}
          </td>
          <td class="unit">{r?.sentinel ? "" : reg.unit}</td>
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
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 10px;
    padding-bottom: 10px;
  }

  /* Title and description share the toolbar row; the description gives way */
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 10px;
    white-space: nowrap;
  }

  .title strong {
    flex: none;
  }

  .title .desc {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title strong {
    font-size: 15px;
  }

  .desc,
  .dim {
    color: var(--text-3);
  }

  .search {
    width: 220px;
  }

  .note {
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
  }

  .rec {
    color: rgb(var(--alert-rgb));
  }

  .warnings {
    margin: 0 0 8px;
    font-size: 13px;
  }

  .deprecated {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    margin: 0 0 8px;
    padding: 8px 10px;
    border-radius: 9px;
    background: var(--subtle);
    font-size: 13px;
    color: var(--text-2);
  }

  .deprecated .error {
    color: rgb(var(--alert-rgb));
  }

  .warnings summary {
    width: fit-content;
    color: var(--warn);
    cursor: pointer;
  }

  .warnings ul {
    margin: 6px 0 0;
    padding: 6px 10px;
    list-style: none;
    max-height: 120px;
    overflow: auto;
    border-radius: 9px;
    background: var(--subtle);
    color: var(--text-2);
  }

  .warnings .where {
    display: inline-block;
    min-width: 64px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-3);
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

  thead th {
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

  table {
    table-layout: fixed;
  }

  .addr {
    width: 104px;
  }

  th:nth-child(3) {
    width: 30%;
  }

  th:nth-child(4) {
    width: 150px;
  }

  th.val {
    width: 170px;
  }

  th:nth-child(6) {
    width: 64px;
  }

  /* Raw takes whatever width is left, so values sit next to their names */
  th:nth-child(7) {
    width: auto;
  }

  .read {
    width: 52px;
    text-align: center;
  }

  /* Registers that are not read: greyed out, at the end of the list */
  tr.off td:not(.read) {
    opacity: 0.4;
  }

  .refs {
    font-size: 12px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--field);
    color: var(--text-2);
    white-space: nowrap;
  }

  .refs.bad {
    background: rgb(var(--alert-rgb) / 0.12);
    color: rgb(var(--alert-rgb));
  }

  .refmark {
    margin-left: 8px;
    padding: 0 5px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    background: var(--field);
    color: var(--text-3);
  }

  .refmark.ok {
    background: var(--accent);
    color: var(--on-accent);
  }

  .refmark.bad {
    background: rgb(var(--alert-rgb));
    color: rgb(var(--alert-fg-rgb));
  }

  .count {
    color: var(--text-2);
    font-size: 13px;
    white-space: nowrap;
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
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .grp {
    margin-left: 8px;
    font-weight: 400;
    font-size: 12px;
    color: var(--text-2);
  }

  .type {
    color: var(--text-2);
    font-size: 12px;
  }

  th.val {
    text-align: right;
  }

  td.val {
    text-align: right;
    font-size: 14px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    /* Long enum labels are cut here (full text in the tooltip) instead of
       running over the unit and raw columns */
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background-color 0.6s var(--ease);
  }

  /* Lit at once when the value changes, then fades back */
  td.val.changed {
    background: var(--changed);
    transition: none;
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

  .sentinel {
    font-family: var(--ui);
    font-size: 13px;
    font-weight: 400;
    font-style: italic;
    color: var(--text-3);
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
