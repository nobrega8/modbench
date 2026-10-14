<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "$lib/api";
  import { FUNCTION_NAME, clock, formatAddress } from "$lib/format";
  import AddressFormat from "./AddressFormat.svelte";
  import { logCsv } from "$lib/csv";
  import { settings } from "$lib/settings.svelte";
  import { bench } from "$lib/state.svelte";
  import type { TrafficEntry } from "$lib/types";

  let errorsOnly = $state(false);
  let saveError = $state<string | null>(null);
  let list = $state<HTMLElement>();
  let follow = $state(true);

  const shown = $derived(
    errorsOnly ? bench.log.filter((t) => t.error !== null) : bench.log,
  );

  function addr(n: number) {
    return formatAddress(n, settings.addressFormat);
  }

  function line(t: TrafficEntry): string {
    const fc = String(t.function).padStart(2, "0");
    return `${clock(t.time_ms)}\tFC${fc}\t${FUNCTION_NAME[t.function] ?? ""}\t${addr(t.address)}\t${t.count}\t${t.duration_ms.toFixed(1)} ms\t${t.error ?? "OK"}`;
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
        { name: "Text", extensions: ["txt"] },
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
    ><input type="checkbox" bind:checked={errorsOnly} /> Errors only</label
  >
  <AddressFormat />
  <span class="spacer"></span>
  {#if saveError}<span class="bad">Could not save: {saveError}</span>{/if}
  <button
    class="btn"
    disabled={!bench.log.length}
    onclick={() => bench.clearLog()}>Clear log</button
  >
  <button class="btn" disabled={!bench.log.length} onclick={saveLog}
    >Save log…</button
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
        <th>Time</th><th>Function</th><th>Address</th><th>Qty</th><th
          >Duration</th
        ><th>Result</th>
      </tr>
    </thead>
    <tbody>
      {#each shown as t, i (i)}
        <tr class:err={t.error !== null}>
          <td class="mono dim">{clock(t.time_ms)}</td>
          <td
            ><span class="mono">{String(t.function).padStart(2, "0")}</span>
            {FUNCTION_NAME[t.function] ?? ""}</td
          >
          <td class="mono">{addr(t.address)}</td>
          <td class="mono">{t.count}</td>
          <td class="mono dim">{t.duration_ms.toFixed(1)} ms</td>
          <td class="res">{t.error ?? "OK"}</td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty">No requests yet.</td></tr>
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
