<script lang="ts">
  import { popIn, popOut } from "#lib/motion.ts";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { MAP_EXTENSIONS, type LibraryEntry } from "#lib/types.ts";
  import { sim } from "#lib/simulator.svelte.ts";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";
  import MapStatus from "./MapStatus.svelte";
  import OnlineLibrary from "./OnlineLibrary.svelte";
  import Icon from "./Icon.svelte";
  import MapExplorer from "./MapExplorer.svelte";
  import VendorIcon from "./VendorIcon.svelte";

  let search = $state("");
  /** A row asking to confirm a delete or an edit. */
  let confirm = $state<{ path: string; kind: "delete" | "edit" } | null>(null);
  let exploring = $state<string | null>(null);
  let message = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);
  let menuOpen = $state(false);
  let onlineOpen = $state(false);

  // The map picker can send people here to browse the online library.
  $effect(() => {
    if (bench.onlineRequested) {
      bench.onlineRequested = false;
      onlineOpen = true;
    }
  });
  let menu: HTMLDivElement | undefined = $state();

  // Close the import menu on a click outside it or on Esc
  $effect(() => {
    if (!menuOpen) return;
    const click = (e: MouseEvent) => {
      if (menu && !menu.contains(e.target as Node)) menuOpen = false;
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") menuOpen = false;
    };
    window.addEventListener("click", click, true);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("click", click, true);
      window.removeEventListener("keydown", key);
    };
  });

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
    menuOpen = false;
    const picked = await open({
      multiple: true,
      directory: false,
      filters: [{ name: "Modbus map", extensions: MAP_EXTENSIONS }],
    });
    if (!picked) return;
    await importPaths(Array.isArray(picked) ? picked : [picked]);
  }

  async function remove(path: string) {
    confirm = null;
    try {
      await api.libraryDelete(path);
      await bench.refreshLibrary();
      message = { ok: true, text: "Map deleted from the library." };
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    }
  }

  async function edit(path: string) {
    confirm = null;
    try {
      await api.libraryEdit(path);
      message = {
        ok: true,
        text: "Opened in the text editor. Changes show here when you come back.",
      };
    } catch (e) {
      message = { ok: false, text: errorText(e) };
    }
  }

  // A map edited outside the app is read again when the window is back.
  $effect(() => {
    const focus = () => bench.refreshLibrary().catch(() => {});
    window.addEventListener("focus", focus);
    return () => window.removeEventListener("focus", focus);
  });

  async function use(path: string) {
    await bench.openMap(path);
  }

  /** Shapes the simulator after a map and shows it. */
  async function simulate(path: string) {
    await sim.pick(path);
    bench.tool = "simulator";
    bench.page = "tools";
  }

  /** What a right-click on a map offers. */
  function rowMenu(e: LibraryEntry): MenuEntry[] {
    return [
      {
        label: "Use",
        disabled: !!e.error || bench.running,
        hint: bench.running ? "stop first" : undefined,
        action: () => use(e.path),
      },
      { label: "Explore", action: () => (exploring = e.path) },
      {
        label: "Edit…",
        action: () => (confirm = { path: e.path, kind: "edit" }),
      },
      {
        label: "Simulate this device",
        disabled: !!e.error,
        action: () => simulate(e.path),
      },
      null,
      {
        label: "Copy name",
        action: () => copy(`${e.vendor} ${e.model}`),
      },
      { label: "Copy file path", action: () => copy(e.path) },
      {
        label: "Open the library folder",
        action: () => api.libraryOpenFolder().catch(() => {}),
      },
      null,
      {
        label: "Delete…",
        danger: true,
        action: () => (confirm = { path: e.path, kind: "delete" }),
      },
    ];
  }
</script>

<div class="toolbar">
  <input
    class="input search"
    type="search"
    placeholder="Search"
    bind:value={search}
    aria-label="Search the library"
  />
  <span class="spacer"></span>
  {#if current && !currentInLibrary}
    <button
      class="btn"
      disabled={busy}
      onclick={() => importPaths([current.path])}
      ><Icon name="plus" />{current.map.meta.model}</button
    >
  {/if}
  {#if bench.libraryFolder}
    <button
      class="btn icon"
      title="Open the library folder"
      aria-label="Open the library folder"
      onclick={() => api.libraryOpenFolder().catch(() => {})}
      ><Icon name="folder" /></button
    >
  {/if}
  <div class="menu-wrap" bind:this={menu}>
    <button
      class="btn accent"
      disabled={busy}
      aria-haspopup="menu"
      aria-expanded={menuOpen}
      onclick={() => (menuOpen = !menuOpen)}
      >Import
      <svg viewBox="0 0 10 6" aria-hidden="true"><path d="M1 1l4 4 4-4" /></svg
      ></button
    >
    {#if menuOpen}
      <div class="menu" role="menu" in:popIn out:popOut>
        <button role="menuitem" onclick={chooseFiles}>
          <span class="item">From a file…</span>
        </button>
        <button
          role="menuitem"
          onclick={() => {
            menuOpen = false;
            onlineOpen = true;
          }}
        >
          <span class="item">From the online library…</span>
        </button>
      </div>
    {/if}
  </div>
</div>

<MapExplorer bind:path={exploring} />

<OnlineLibrary
  bind:open={onlineOpen}
  onadded={(text) => (message = { ok: true, text })}
/>

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
        <tr
          class:invalid={!!e.error}
          class:active={current?.path === e.path}
          oncontextmenu={(ev) => contextMenu.show(ev, rowMenu(e))}
        >
          <td
            ><span class="vendor"
              ><VendorIcon
                vendor={e.vendor}
                website={e.vendor_website}
              />{e.vendor}</span
            ></td
          >
          <td class="model"
            ><span class="model-name"
              >{e.model}<MapStatus status={e.status} /></span
            ></td
          >
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
            {#if confirm?.path === e.path}
              <span class="ask"
                >{confirm.kind === "delete"
                  ? "Delete this map?"
                  : "Edits the library's copy of this map."}</span
              >
              {#if confirm.kind === "delete"}
                <button class="btn small danger" onclick={() => remove(e.path)}
                  >Delete</button
                >
              {:else}
                <button class="btn small accent" onclick={() => edit(e.path)}
                  >Edit</button
                >
              {/if}
              <button class="btn small" onclick={() => (confirm = null)}
                >Cancel</button
              >
            {:else}
              <button
                class="btn small accent"
                disabled={!!e.error || bench.running}
                title={bench.running ? "Stop the test first" : undefined}
                onclick={() => use(e.path)}>Use</button
              >
              <button
                class="btn small icon ghost"
                title="Explore"
                aria-label="Explore {e.model}"
                disabled={!!e.error}
                onclick={() => (exploring = e.path)}
                ><Icon name="explore" /></button
              >
              <button
                class="btn small icon ghost"
                title="Edit"
                aria-label="Edit {e.model}"
                onclick={() => (confirm = { path: e.path, kind: "edit" })}
                ><Icon name="edit" /></button
              >
              <button
                class="btn small icon ghost del"
                title="Delete"
                aria-label="Delete {e.model}"
                onclick={() => (confirm = { path: e.path, kind: "delete" })}
                ><Icon name="trash" /></button
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
              No maps yet. Import some to use them in step 2.
            {/if}
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

  .search {
    width: 260px;
  }

  .spacer {
    flex: 1;
  }

  .menu-wrap {
    position: relative;
  }

  .menu-wrap svg {
    width: 10px;
    height: 6px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .menu {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 10;
    width: 220px;
    padding: 5px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: var(--e3);
    transform-origin: top right;
  }

  .menu button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    width: 100%;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    text-align: left;
    cursor: pointer;
  }

  .menu button:hover,
  .menu button:focus-visible {
    background: var(--accent-soft);
  }

  .menu .item {
    font-weight: 500;
  }

  .vendor {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .model-name {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  td {
    vertical-align: middle;
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
    border-radius: 10px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
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
    margin-left: 2px;
  }

  .act .icon {
    width: 26px;
    vertical-align: middle;
  }

  .act .del:hover:not(:disabled) {
    color: rgb(var(--alert-rgb));
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

  .empty {
    color: var(--text-2);
    padding: 16px 10px;
  }
</style>
