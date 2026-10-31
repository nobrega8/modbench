<script module lang="ts">
  /** Column choices for this session, by the header row (or the width of a
   * table without one), so the next paste of a like table needs none. */
  const remembered = new Map<string, (Column | null)[]>();
</script>

<!--
  Import a register table into the map builder: paste it (from a spreadsheet
  or a manual) or open a CSV file, check which column holds what, and add the
  rows to the map or put them in place of its registers.
-->
<script lang="ts">
  import { open as openFile } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { builder } from "#lib/builder.svelte.ts";
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import {
    COLUMNS,
    guessColumns,
    hasHeader,
    importRows,
    parseTable,
    type Column,
  } from "#lib/table-import.ts";
  import type { Area } from "#lib/types.ts";
  import Icon from "./Icon.svelte";

  let {
    open = $bindable(false),
    onimported,
  }: {
    open?: boolean;
    /** After the import, with the position of the first new register. */
    onimported?: (first: number) => void;
  } = $props();

  /** How many rows the preview shows. */
  const PREVIEW_ROWS = 50;

  const LABEL: Record<Column, Key> = {
    name: "computedform.name",
    group: "maptable.group",
    area: "customregisterform.area",
    address: "req.address",
    type: "customregisterform.type",
    length: "customregisterform.length_registers",
    scale: "customregisterform.scale",
    unit: "computedform.unit",
    access: "customregisterform.access",
    description: "libraryview.description",
  };
  const AREAS: { value: Area; label: Key }[] = [
    { value: "holding", label: "customregisterform.holding_register" },
    { value: "input", label: "customregisterform.input_register" },
    { value: "coil", label: "customregisterform.coil" },
    { value: "discrete", label: "customregisterform.discrete_input" },
  ];

  let dialog: HTMLDialogElement | undefined = $state();
  let text = $state("");
  let header = $state(false);
  let columns = $state<(Column | null)[]>([]);
  let area = $state<Area>("holding");
  let problem = $state<string | null>(null);

  const table = $derived(parseTable(text));
  const body = $derived(header ? table.slice(1) : table);
  const width = $derived(Math.max(0, ...table.map((r) => r.length)));
  const result = $derived(
    importRows(body, columns, {
      base: builder.meta.address_base ?? 0,
      area,
    }),
  );

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    else if (!open && dialog.open) dialog.close();
  });

  // A new table: guess its header and columns, or keep the ones chosen for
  // the last table of the same shape (see `remembered`).
  $effect(() => {
    const rows = table;
    const head = hasHeader(rows);
    const key = head ? rows[0].join("\t") : `${width}`;
    header = head;
    columns =
      remembered.get(key) ??
      (head
        ? guessColumns(rows[0])
        : Array.from({ length: width }, () => null));
  });

  function choose(index: number, col: Column | null) {
    const next = [...columns];
    // A column can be chosen once: another that had it is left out.
    if (col) next.forEach((c, i) => c === col && (next[i] = null));
    next[index] = col;
    columns = next;
    remembered.set(header ? table[0].join("\t") : `${width}`, next);
  }

  async function openCsv() {
    problem = null;
    const picked = await openFile({
      multiple: false,
      filters: [{ name: "CSV", extensions: ["csv", "tsv", "txt"] }],
    });
    if (typeof picked !== "string") return;
    try {
      text = await api.readText(picked);
    } catch (e) {
      problem = errorText(e);
    }
  }

  function apply(replace: boolean) {
    const { registers, problems } = $state.snapshot(result);
    const first = builder.importRegisters(registers, replace);
    const rows = [...problems.keys()].map((i) => first + i + 1);
    builder.message = rows.length
      ? {
          ok: false,
          text: t("tableimport.imported_check", {
            count: registers.length,
            rows: rows.join(", "),
          }),
        }
      : {
          ok: true,
          text: t("tableimport.imported", { count: registers.length }),
        };
    text = "";
    open = false;
    onimported?.(first);
  }
</script>

<dialog
  bind:this={dialog}
  onclose={() => (open = false)}
  aria-labelledby="import-title"
>
  <header>
    <div class="heading">
      <h2 id="import-title">{t("tableimport.title")}</h2>
      <p>
        {t("tableimport.hint", { base: builder.meta.address_base ?? 0 })}
      </p>
    </div>
    <button
      class="btn icon ghost"
      title={t("mapexplorer.close")}
      onclick={() => (open = false)}><Icon name="close" size={16} /></button
    >
  </header>

  <div class="body">
    {#if table.length === 0}
      <textarea
        class="input paste"
        placeholder={t("tableimport.paste")}
        aria-label={t("tableimport.paste")}
        bind:value={text}
        spellcheck="false"></textarea>
      <div class="row">
        <button class="btn" onclick={openCsv}
          ><Icon name="folder" />{t("tableimport.open_csv")}</button
        >
        {#if problem}<span class="bad">{problem}</span>{/if}
      </div>
    {:else}
      <div class="row">
        <label class="check">
          <input type="checkbox" bind:checked={header} />
          {t("tableimport.header")}
        </label>
        <label class="check">
          {t("tableimport.area")}
          <select class="input" bind:value={area}>
            {#each AREAS as a (a.value)}<option value={a.value}
                >{t(a.label)}</option
              >{/each}
          </select>
        </label>
        <span class="spacer"></span>
        <button class="btn ghost" onclick={() => (text = "")}
          >{t("tableimport.other")}</button
        >
      </div>

      <div class="preview">
        <table>
          <thead>
            <tr>
              <th class="n"></th>
              {#each { length: width } as _, i (i)}
                <th>
                  <select
                    class="input"
                    aria-label={t("tableimport.column", { n: i + 1 })}
                    value={columns[i] ?? ""}
                    onchange={(e) =>
                      choose(
                        i,
                        (e.currentTarget.value || null) as Column | null,
                      )}
                  >
                    <option value="">{t("tableimport.ignore")}</option>
                    {#each COLUMNS as c (c)}<option value={c}
                        >{t(LABEL[c])}</option
                      >{/each}
                  </select>
                  {#if header}<span class="dim">{table[0][i]}</span>{/if}
                </th>
              {/each}
            </tr>
          </thead>
          <tbody>
            {#each body.slice(0, PREVIEW_ROWS) as row, r (r)}
              {@const bad = result.problems.get(r)}
              <tr class:bad title={bad?.join("\n")}>
                <td class="n dim">{r + 1}</td>
                {#each { length: width } as _, i (i)}
                  <td class:off={!columns[i]}>{row[i] ?? ""}</td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
        {#if body.length > PREVIEW_ROWS}
          <p class="dim more">
            {t("tableimport.more", { count: body.length - PREVIEW_ROWS })}
          </p>
        {/if}
      </div>
    {/if}
  </div>

  <footer>
    {#if table.length}
      <span class:bad={result.problems.size > 0}>
        {result.problems.size
          ? t("tableimport.n_problems", {
              count: body.length,
              bad: result.problems.size,
            })
          : t("tableimport.n_rows", { count: body.length })}
      </span>
    {/if}
    <span class="spacer"></span>
    <button class="btn" onclick={() => (open = false)}
      >{t("computedform.cancel")}</button
    >
    <button
      class="btn"
      disabled={body.length === 0 || !columns.includes("address")}
      onclick={() => apply(true)}>{t("tableimport.replace")}</button
    >
    <button
      class="btn accent"
      disabled={body.length === 0 || !columns.includes("address")}
      onclick={() => apply(false)}>{t("tableimport.append")}</button
    >
  </footer>
</dialog>

<style>
  dialog {
    width: min(1120px, calc(100vw - 48px));
    height: min(640px, calc(100vh - 80px));
    padding: 0;
    border: 0;
    border-radius: 14px;
    background: var(--card);
    color: var(--text);
    box-shadow: var(--e3);
    flex-direction: column;
    overflow: hidden;
  }

  dialog[open] {
    display: flex;
    animation: rise 0.22s var(--ease);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.32);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.99);
    }
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 16px 16px 10px 20px;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  header p {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 20px;
  }

  .paste {
    flex: 1;
    resize: none;
    font-family: var(--mono);
    font-size: 12px;
  }

  .row,
  footer {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  footer {
    padding: 12px 20px 16px;
  }

  .spacer {
    flex: 1;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }

  .preview {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-radius: 10px;
    box-shadow: inset 0 0 0 1px var(--border);
  }

  table {
    border-collapse: collapse;
    font-size: 12px;
  }

  th {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 6px;
    background: var(--card);
    text-align: left;
    vertical-align: top;
  }

  th select {
    min-width: 110px;
  }

  th .dim {
    display: block;
    margin-top: 3px;
    font-weight: 400;
    white-space: nowrap;
  }

  td {
    padding: 4px 6px;
    border-top: 1px solid var(--border);
    white-space: nowrap;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .n {
    text-align: right;
  }

  td.off {
    color: var(--text-3);
  }

  tr.bad td {
    background: rgb(var(--alert-rgb) / 0.08);
  }

  .bad {
    color: var(--bad);
  }

  .more {
    padding: 6px 10px;
    margin: 0;
  }
</style>
