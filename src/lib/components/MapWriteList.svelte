<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import Select from "./Select.svelte";
  import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import {
    AREA_TAG,
    formatValue,
    limitError,
    limitHint,
    parseForRegister,
  } from "#lib/format.ts";
  import { bench, type OpenMap } from "#lib/state.svelte.ts";
  import {
    collect,
    parse,
    plan,
    serialize,
    type ParsedTemplate,
  } from "#lib/template.ts";
  import type { Register, Value } from "#lib/types.ts";

  let { open }: { open: OpenMap } = $props();

  let search = $state("");
  let drafts = $state<Record<number, string>>({});
  let notes = $state<Record<number, { ok: boolean; text: string }>>({});
  let busy = $state<number | null>(null);
  /** Rows with labelled values where a value of one's own is typed. */
  let custom = $state<Record<number, boolean>>({});
  /** Picked in a labelled list to type a value instead. */
  const CUSTOM = "__custom__";

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

  // ---- value templates

  let tpl = $state<{ name: string; parsed: ParsedTemplate } | null>(null);
  let onlyChanged = $state(true);
  let applying = $state(false);
  let tplNote = $state<{ ok: boolean; text: string } | null>(null);
  /** Values the device held before the last template was applied. */
  let undo = $state<{ index: number; value: Value }[] | null>(null);

  const tplPlan = $derived(
    tpl ? plan(open.map.registers, bench.mapResults, tpl.parsed.entries) : null,
  );
  const todo = $derived(
    tplPlan?.writes.filter((w) => !w.problem && (!onlyChanged || w.changed)) ??
      [],
  );
  const refused = $derived(tplPlan?.writes.filter((w) => w.problem) ?? []);
  const unchanged = $derived(
    tplPlan?.writes.filter((w) => !w.problem && !w.changed).length ?? 0,
  );

  function fileTitle(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }

  async function saveTemplate() {
    tplNote = null;
    const entries = collect(open.map.registers, bench.mapResults);
    if (!entries.length) {
      tplNote = {
        ok: false,
        text: t("mapwritelist.read_the_device_first"),
      };
      return;
    }
    const { vendor, model } = open.map.meta;
    const stamp = new Date().toISOString().slice(0, 16).replace(/[-:T]/g, "");
    const path = await save({
      defaultPath: `${model.replace(/[^\w.-]+/g, "_")}-${stamp}.mbtpl`,
      filters: [
        { name: t("mapwritelist.modbench_template"), extensions: ["mbtpl"] },
      ],
    });
    if (!path) return;
    try {
      await api.saveText(path, serialize(`${vendor} ${model}`.trim(), entries));
      tplNote = {
        ok: true,
        text: t("mapwritelist.values_saved_count", { count: entries.length }),
      };
    } catch (e) {
      tplNote = { ok: false, text: errorText(e) };
    }
  }

  async function loadTemplate() {
    tplNote = null;
    undo = null;
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [
        {
          name: t("mapwritelist.modbench_template"),
          extensions: ["mbtpl", "txt"],
        },
      ],
    });
    if (!picked || Array.isArray(picked)) return;
    try {
      const parsed = parse(await api.readText(picked));
      tpl = { name: fileTitle(picked), parsed };
    } catch (e) {
      tplNote = { ok: false, text: errorText(e) };
    }
  }

  async function applyTemplate() {
    if (!tpl || !todo.length || applying) return;
    const n = todo.length;
    const ok = await bench.confirmWrite(
      t("mapwritelist.write_count_values_from", {
        count: n,
        template: tpl.name,
        model: open.map.meta.model,
      }),
    );
    if (!ok) return;
    applying = true;
    tplNote = null;
    const before = todo
      .filter((w) => w.current !== null)
      .map((w) => ({ index: w.index, value: w.current as Value }));
    try {
      const results = await bench.writeMapValues(
        todo.map((w) => ({ index: w.index, value: w.value })),
      );
      const failed = results.filter((r) => r.error);
      const written = new Set(
        results.filter((r) => !r.error).map((r) => r.index),
      );
      undo = before.filter((b) => written.has(b.index));
      tplNote = failed.length
        ? {
            ok: false,
            text: t("mapwritelist.wrote_done_of_total", {
              done: results.length - failed.length,
              total: results.length,
              name: open.map.registers[failed[0].index].name,
              error: failed[0].error ?? "",
            }),
          }
        : {
            ok: true,
            text: t("mapwritelist.wrote_total", { total: results.length }),
          };
    } finally {
      applying = false;
    }
  }

  async function undoTemplate() {
    if (!undo?.length || applying) return;
    const items = undo;
    if (
      !(await bench.confirmWrite(
        t("mapwritelist.put_back_count_values", { count: items.length }),
      ))
    )
      return;
    applying = true;
    try {
      const results = await bench.writeMapValues(items);
      const failed = results.filter((r) => r.error);
      tplNote = failed.length
        ? {
            ok: false,
            text: t("mapwritelist.restored_done_of_total", {
              done: results.length - failed.length,
              total: results.length,
              error: failed[0].error ?? "",
            }),
          }
        : {
            ok: true,
            text: t("mapwritelist.restored_total", { total: results.length }),
          };
      undo = null;
    } finally {
      applying = false;
    }
  }

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
    const outside = limitError(reg, parsed.value);
    if (outside) {
      notes[index] = { ok: false, text: outside };
      return;
    }
    if (
      !(await bench.confirmWrite(
        t("mapwritelist.write_value_to_name", { value: text, name: reg.name }),
      ))
    )
      return;
    busy = index;
    try {
      await bench.writeMapValue(index, parsed.value);
      notes[index] = {
        ok: true,
        text: t("mapwritelist.wrote_value", { value: text }),
      };
    } catch (e) {
      notes[index] = { ok: false, text: errorText(e) };
    } finally {
      busy = null;
    }
  }
</script>

<div class="toolbar">
  <span class="count"
    >{t("mapwritelist.writable_registers_in_model", {
      model: open.map.meta.model,
      count: total,
    })}</span
  >
  <span class="spacer"></span>
  <button
    class="btn"
    title={t("mapwritelist.save_the_values_last")}
    onclick={saveTemplate}>{t("mapwritelist.save_template")}</button
  >
  <button
    class="btn"
    title={t("mapwritelist.write_the_values_of")}
    onclick={loadTemplate}>{t("mapwritelist.load_template")}</button
  >
  <input
    class="input search"
    type="search"
    placeholder={t("maptable.search_name_or_address")}
    bind:value={search}
    aria-label={t("mapwritelist.search_writable_registers")}
  />
</div>

{#if tplNote && !tpl}
  <p class="tpl-note" class:good={tplNote.ok} class:bad={!tplNote.ok}>
    {tplNote.text}
  </p>
{/if}

{#if tpl && tplPlan}
  <div class="tpl">
    <div class="tpl-head">
      <strong>{tpl.name}</strong>
      {#if tpl.parsed.device}<span class="dim">{tpl.parsed.device}</span>{/if}
      <span class="spacer"></span>
      <button
        class="btn"
        disabled={applying}
        onclick={() => {
          tpl = null;
          undo = null;
          tplNote = null;
        }}>{t("mapexplorer.close")}</button
      >
    </div>
    <p class="tpl-sum">
      {t("mapwritelist.count_values_match_this", {
        count: tplPlan.writes.length,
      })}{#if unchanged}, {t("mapwritelist.count_already_on_the", {
          count: unchanged,
        })}{/if}{#if tplPlan.unmatched.length}, {t(
          "mapwritelist.count_not_in_the",
          { count: tplPlan.unmatched.length },
        )}{/if}{#if tpl.parsed.errors.length}, {t(
          "mapwritelist.count_unreadable_lines",
          { count: tpl.parsed.errors.length },
        )}{/if}.
    </p>
    {#if tpl.parsed.errors.length}
      <ul class="tpl-list bad">
        {#each tpl.parsed.errors.slice(0, 5) as e (e.line)}
          <li>
            {t("mapwritelist.line_line_message", {
              line: e.line,
              message: e.message,
            })}
          </li>
        {/each}
      </ul>
    {/if}
    {#if refused.length}
      <ul class="tpl-list bad">
        {#each refused.slice(0, 5) as w, i (i)}
          <li>{w.problem} {t("mapwritelist.skipped")}</li>
        {/each}
      </ul>
    {/if}
    {#if !bench.mapResults.size}
      <p class="dim">
        {t("mapwritelist.nothing_has_been_read")}
      </p>
    {/if}
    <div class="tpl-actions">
      <label class="check"
        ><input type="checkbox" bind:checked={onlyChanged} />
        {t("mapwritelist.only_values_that_differ")}</label
      >
      <span class="spacer"></span>
      {#if tplNote}<span class:good={tplNote.ok} class:bad={!tplNote.ok}
          >{tplNote.text}</span
        >{/if}
      {#if undo?.length}
        <button class="btn" disabled={applying} onclick={undoTemplate}
          >{t("mapwritelist.undo")}</button
        >
      {/if}
      <button
        class="btn accent"
        disabled={applying || !todo.length}
        onclick={applyTemplate}
        >{applying
          ? t("mapwritelist.writing")
          : t("mapwritelist.write_count_values", {
              count: todo.length,
            })}</button
      >
    </div>
  </div>
{/if}

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th class="addr">{t("req.address")}</th>
        <th class="name-col">{t("computedform.name")}</th>
        <th class="cur">{t("mapwritelist.current_value")}</th>
        <th>{t("maptable.new_value")}</th>
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
            {#if reg.access === "W"}<span class="dim"
                >{t("maptable.write_only")}</span
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
              {:else if reg.enum.length && !custom[index]}
                <Select
                  bind:value={drafts[index]}
                  width="260px"
                  label={t("mapwritelist.value_for_name", { name: reg.name })}
                  placeholder={t("maptable.choose")}
                  options={[
                    ...reg.enum.map(([v, l]) => ({
                      value: l,
                      label: `${l} (${v})`,
                    })),
                    { value: CUSTOM, label: t("mapwritelist.custom_value") },
                  ]}
                  onchange={(v) => {
                    if (v !== CUSTOM) return;
                    custom[index] = true;
                    drafts[index] = "";
                  }}
                />
                <button
                  class="btn accent"
                  disabled={busy !== null || !drafts[index]}
                  >{t("maptable.write2")}</button
                >
              {:else}
                <input
                  class="input num"
                  bind:value={drafts[index]}
                  spellcheck="false"
                  placeholder={reg.type === "string"
                    ? t("mapwritelist.text")
                    : (limitHint(reg) ?? t("maptable.value"))}
                  title={limitHint(reg)
                    ? `Accepted range: ${limitHint(reg)}${reg.unit ? " " + reg.unit : ""}`
                    : undefined}
                  aria-label={t("mapwritelist.value_for_name", {
                    name: reg.name,
                  })}
                />
                <span class="dim unit">{reg.unit}</span>
                <button
                  class="btn accent"
                  disabled={busy !== null || !drafts[index]}
                  >{t("maptable.write2")}</button
                >
                {#if reg.enum.length}
                  <button
                    type="button"
                    class="btn ghost"
                    title={t("maptable.pick_one_of_the")}
                    onclick={() => {
                      custom[index] = false;
                      drafts[index] = "";
                    }}>{t("maptable.options")}</button
                  >
                {/if}
              {/if}
              {#if busy === index}<span class="dim"
                  >{t("mapwritelist.writing")}</span
                >{/if}
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
              ? t("mapwritelist.no_writable_registers_match")
              : t("mapwritelist.this_map_has_no")}
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

  .tpl {
    margin-bottom: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tpl-head,
  .tpl-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .tpl-sum,
  .tpl-note {
    margin: 0;
  }

  .tpl-list {
    margin: 0;
    padding-left: 18px;
    font-size: 12px;
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

  /* A fixed slot keeps every Write button in one column */
  .unit {
    width: 44px;
    margin-left: -2px;
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
