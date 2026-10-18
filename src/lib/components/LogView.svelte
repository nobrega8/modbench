<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import {
    FUNCTION_NAME,
    areaOfFunction,
    clock,
    formatAddress,
  } from "#lib/format.ts";
  import AddressFormat from "./AddressFormat.svelte";
  import { logCsv } from "#lib/csv.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { TrafficEntry } from "#lib/types.ts";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";

  let errorsOnly = $state(false);
  let saveError = $state<string | null>(null);
  let list = $state<HTMLElement>();
  let follow = $state(true);

  const shown = $derived(
    errorsOnly
      ? bench.log.filter((e) => e.error !== null || e.note)
      : bench.log,
  );

  function addr(e: TrafficEntry) {
    return formatAddress(
      e.address,
      settings.addressFormat,
      areaOfFunction(e.function),
    );
  }

  function line(e: TrafficEntry): string {
    if (e.note) return `${clock(e.time_ms)}\e${e.note}`;
    const fc = String(e.function).padStart(2, "0");
    return `${clock(e.time_ms)}\tFC${fc}\e${FUNCTION_NAME[e.function] ?? ""}\e${addr(e)}\e${e.count}\e${e.duration_ms.toFixed(1)} ms\e${e.error ?? "OK"}`;
  }

  /** What a right-click on a request offers. */
  function rowMenu(e: TrafficEntry): MenuEntry[] {
    return [
      { label: t("logview.copy_line"), action: () => copy(line(e)) },
      e.error !== null && {
        label: t("logview.copy_error"),
        action: () => copy(e.error ?? ""),
      },
      {
        label: errorsOnly
          ? t("logview.copy_errors_shown")
          : t("logview.copy_all"),
        action: () => copy(shown.map(line).join("\n")),
      },
      null,
      {
        label: errorsOnly
          ? t("logview.show_every_request")
          : t("logview.show_errors_only"),
        action: () => (errorsOnly = !errorsOnly),
      },
      { label: t("logview.save_log"), action: saveLog },
      { label: "Clear log", danger: true, action: () => bench.clearLog() },
    ];
  }

  $effect(() => {
    void shown.length;
    if (follow && list) list.scrollTop = list.scrollHeight;
  });

  async function saveLog() {
    saveError = null;
    const path = await save({
      defaultPath: "modbench-log.csv",
      filters: [
        { name: "CSV", extensions: ["csv"] },
        { name: t("logview.text"), extensions: ["txt"] },
      ],
    });
    if (!path) return;
    const text = path.toLowerCase().endsWith(".csv")
      ? logCsv(bench.log, settings.csvSeparator)
      : bench.log.map(line).join("\n") + "\n";
    try {
      await api.saveText(path, text);
    } catch (e) {
      saveError = errorText(e);
    }
  }
</script>

<div class="toolbar">
  <label class="check"
    ><input type="checkbox" bind:checked={errorsOnly} />
    {t("logview.errors_only")}</label
  >
  <AddressFormat />
  <span class="spacer"></span>
  {#if saveError}<span class="bad"
      >{t("logview.could_not_save_error", { error: saveError })}</span
    >{/if}
  <button
    class="btn"
    disabled={!bench.log.length}
    onclick={() => bench.clearLog()}>{t("logview.clear_log")}</button
  >
  <button class="btn" disabled={!bench.log.length} onclick={saveLog}
    >{t("logview.save_log")}</button
  >
</div>

<div
  class="table-wrap"
  bind:this={list}
  onscroll={() => {
    if (list)
      follow = list.scrollHeight - list.scrollTop - list.clientHeight < 24;
  }}
>
  <table>
    <thead>
      <tr>
        <th>{t("logview.time")}</th><th>{t("logview.function")}</th><th
          >{t("req.address")}</th
        ><th>{t("logview.qty")}</th><th>{t("logview.duration")}</th><th
          >{t("logview.result")}</th
        >
      </tr>
    </thead>
    <tbody>
      {#each shown as row, i (i)}
        {#if row.note}
          <tr class="note">
            <td class="mono dim">{clock(row.time_ms)}</td>
            <td colspan="5">{row.note}</td>
          </tr>
        {:else}
          <tr
            class:err={row.error !== null}
            oncontextmenu={(e) => contextMenu.show(e, rowMenu(row))}
          >
            <td class="mono dim">{clock(row.time_ms)}</td>
            <td
              ><span class="mono">{String(row.function).padStart(2, "0")}</span>
              {FUNCTION_NAME[row.function] ?? ""}</td
            >
            <td class="mono">{addr(row)}</td>
            <td class="mono">{row.count}</td>
            <td class="mono dim">{row.duration_ms.toFixed(1)} ms</td>
            <td class="res">{row.error ?? "OK"}</td>
          </tr>
        {/if}
      {:else}
        <tr
          ><td colspan="6" class="empty">{t("logview.no_requests_yet")}</td></tr
        >
      {/each}
    </tbody>
  </table>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding-bottom: 10px;
  }

  .spacer {
    flex: 1;
  }

  .bad {
    color: var(--bad);
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
    font-size: 13px;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--card);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: left;
    padding: 5px 10px;
  }

  td {
    padding: 3px 10px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  tr.note td {
    color: var(--text-2);
    font-style: italic;
  }

  .mono {
    font-family: var(--mono);
    font-size: 12px;
  }

  .dim {
    color: var(--text-3);
  }

  .res {
    color: var(--text-3);
  }

  .err .res {
    color: var(--bad);
    font-weight: 600;
  }

  .empty {
    color: var(--text-2);
    padding: 12px 10px;
  }
</style>
