<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "$lib/api";
  import { bench } from "$lib/state.svelte";

  let search = $state("");
  let confirmDelete = $state<string | null>(null);
  let message = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);

  const shown = $derived(
    bench.library.filter((e) => {
      const q = search.trim().toLowerCase();
      return (
        !q ||
        e.vendor.toLowerCase().includes(q) ||
        e.model.toLowerCase().includes(q) ||
        (e.description ?? "").toLowerCase().includes(q)
      );
    }),
  );

  const current = $derived(bench.map);
  const currentInLibrary = $derived(
    !!current && bench.library.some((e) => e.path === current.path),
  );

  async function importPaths(paths: string[]) {
    busy = true;
    try {
      const results = await api.libraryImport(paths);
      await bench.refreshLibrary();
      const failed = results.filter((r) => r.error);
      const added = results.length - failed.length;
      message = failed.length
        ? {
            ok: false,
            text:
              `${added} added. Not added: ` +
              failed
                .map((r) => `${r.source.split(/[\\/]/).pop()} (${r.error})`)
                .join("; "),
          }
        : {
            ok: true,
            text: `${added} map${added === 1 ? "" : "s"} added to the library.`,
          };
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    } finally {
      busy = false;
    }
  }

  async function chooseFiles() {
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [{ name: "Modbus map", extensions: ["mbmap"] }],
    });
    if (!picked) return;
    await importPaths(Array.isArray(picked) ? picked : [picked]);
  }

  async function remove(path: string) {
    confirmDelete = null;
    try {
      await api.libraryDelete(path);
      await bench.refreshLibrary();
      message = { ok: true, text: "Map deleted from the library." };
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    }
  }

  async function use(path: string) {
    await bench.openMap(path);
  }
</script>

<div class="toolbar">
  <input
    class="input search"
    type="search"
    placeholder="Search vendor or model"
    bind:value={search}
    aria-label="Search the library"
  />
  <span class="spacer"></span>
  {#if current && !currentInLibrary}
    <button
      class="btn"
      disabled={busy}
      onclick={() => importPaths([current.path])}
      >Add {current.map.meta.model} to library</button
    >
  {/if}
  <button class="btn accent" disabled={busy} onclick={chooseFiles}
    >Import maps…</button
  >
</div>

{#if message}
  <p class="msg" class:bad={!message.ok} class:good={message.ok} role="status">
    {message.text}
  </p>
{/if}

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th>Vendor</th>
        <th>Model</th>
        <th>Description</th>
        <th class="num">Registers</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each shown as e (e.path)}
        <tr class:invalid={!!e.error} class:active={current?.path === e.path}>
          <td>{e.vendor}</td>
          <td class="model">{e.model}</td>
          <td class="desc">
            {#if e.error}
              <span class="bad" title={e.error}>Has errors: {e.error}</span>
            {:else}
              {e.description ?? ""}
              {#if e.warnings}<span class="warn"
                  >{e.warnings} warning{e.warnings === 1 ? "" : "s"}</span
                >{/if}
            {/if}
          </td>
          <td class="num">{e.error ? "" : e.registers}</td>
          <td class="act">
            {#if confirmDelete === e.path}
              <span class="ask">Delete this map?</span>
              <button class="btn small danger" onclick={() => remove(e.path)}
                >Delete</button
              >
              <button class="btn small" onclick={() => (confirmDelete = null)}
                >Keep</button
              >
            {:else}
              <button
                class="btn small accent"
                disabled={!!e.error || bench.running}
                onclick={() => use(e.path)}>Use</button
              >
              <button class="btn small" onclick={() => (confirmDelete = e.path)}
                >Delete…</button
              >
            {/if}
          </td>
        </tr>
      {:else}
        <tr>
          <td colspan="5" class="empty">
            {#if bench.library.length}
              No maps match “{search}”.
            {:else}
              The library is empty. Import <code>.mbmap</code> files to keep your
              device maps here, ready for step 2.
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

{#if bench.libraryFolder}
  <p class="folder">Stored in <code>{bench.libraryFolder}</code></p>
{/if}

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 10px;
  }

  .search {
    width: 260px;
  }

  .spacer {
    flex: 1;
  }

  .msg {
    margin: 0 0 8px;
  }

  .bad {
    color: var(--bad);
  }

  .good {
    color: var(--ok);
  }

  .warn {
    margin-left: 8px;
    color: var(--warn);
    font-size: 12px;
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

  th {
    position: sticky;
    top: 0;
    background: var(--subtle);
    border-bottom: 1px solid var(--border);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    text-align: left;
    padding: 6px 10px;
  }

  td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
  }

  tbody tr:hover td {
    background: var(--accent-soft);
  }

  tr.active td {
    background: var(--accent-soft);
  }

  .model {
    font-weight: 600;
  }

  .desc {
    color: var(--text-2);
    max-width: 420px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .num {
    text-align: right;
    font-family: var(--mono);
    font-size: 13px;
  }

  .act {
    text-align: right;
    white-space: nowrap;
  }

  .act .btn + .btn {
    margin-left: 4px;
  }

  .ask {
    margin-right: 6px;
    color: var(--text-2);
  }

  .small {
    height: 26px;
    padding: 0 10px;
    font-size: 13px;
  }

  .danger {
    border-color: var(--bad);
    color: var(--bad);
  }

  .empty {
    color: var(--text-2);
    padding: 16px 10px;
  }

  .folder {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--text-3);
  }

  code {
    font-family: var(--mono);
    font-size: 12px;
  }
</style>
