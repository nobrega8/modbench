<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { popIn, popOut } from "#lib/motion.ts";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { modelMatches } from "#lib/search.ts";
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
        modelMatches(q, e.model, e.models) ||
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
              t("libraryview.added_added_not_added", { added }) +
              failed
                .map((r) => `${r.source.split(/[\\/]/).pop()} (${r.error})`)
                .join("; "),
          }
        : {
            ok: true,
            text: t("libraryview.added_to_the_library", { added }),
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
      filters: [
        { name: t("libraryview.modbus_map"), extensions: MAP_EXTENSIONS },
      ],
    });
    if (!picked) return;
    await importPaths(Array.isArray(picked) ? picked : [picked]);
  }

  async function remove(path: string) {
    confirm = null;
    try {
      await api.libraryDelete(path);
      await bench.refreshLibrary();
      message = { ok: true, text: t("libraryview.map_deleted_from_the") };
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
        text: t("libraryview.opened_in_the_text"),
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

  /** Copies a map as "<model> copy", to change it without losing the original. */
  async function duplicate(e: LibraryEntry) {
    try {
      await api.libraryDuplicate(e.path);
      await bench.refreshLibrary();
      message = { ok: true, text: `${e.vendor} ${e.model} copied.` };
    } catch (err) {
      message = { ok: false, text: errorText(err) };
    }
  }

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
        label: t("libraryview.use"),
        disabled: !!e.error || bench.running,
        hint: bench.running ? t("libraryview.stop_first") : undefined,
        action: () => use(e.path),
      },
      { label: t("libraryview.explore"), action: () => (exploring = e.path) },
      {
        label: t("libraryview.edit2"),
        action: () => (confirm = { path: e.path, kind: "edit" }),
      },
      {
        label: t("builder.edit_in_builder"),
        disabled: !!e.error,
        action: () =>
          api
            .builderOpenWindow({
              path: `${bench.libraryFolder}/${e.path}`,
              register: null,
            })
            .catch(() => {}),
      },
      {
        label: t("libraryview.duplicate"),
        disabled: !!e.error,
        action: () => duplicate(e),
      },
      {
        label: t("libraryview.simulate_this_device"),
        disabled: !!e.error,
        action: () => simulate(e.path),
      },
      null,
      {
        label: t("libraryview.copy_name"),
        action: () => copy(`${e.vendor} ${e.model}`),
      },
      { label: t("libraryview.copy_file_path"), action: () => copy(e.path) },
      {
        label: t("libraryview.open_the_library_folder"),
        action: () => api.libraryOpenFolder().catch(() => {}),
      },
      null,
      {
        label: t("libraryview.delete2"),
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
    placeholder={t("libraryview.search")}
    bind:value={search}
    aria-label={t("libraryview.search_the_library")}
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
  <button
    class="btn"
    title={t("builder.open_hint")}
    onclick={() => api.builderOpenWindow().catch(() => {})}
    ><Icon name="plus" />{t("builder.title")}</button
  >
  {#if bench.libraryFolder}
    <button
      class="btn icon"
      title={t("libraryview.open_the_library_folder")}
      aria-label={t("libraryview.open_the_library_folder")}
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
      >{t("libraryview.import")}
      <svg viewBox="0 0 10 6" aria-hidden="true"><path d="M1 1l4 4 4-4" /></svg
      ></button
    >
    {#if menuOpen}
      <div class="menu" role="menu" in:popIn out:popOut>
        <button role="menuitem" onclick={chooseFiles}>
          <span class="item">{t("libraryview.from_a_file")}</span>
        </button>
        <button
          role="menuitem"
          onclick={() => {
            menuOpen = false;
            onlineOpen = true;
          }}
        >
          <span class="item">{t("libraryview.from_the_online_library")}</span>
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
        <th>{t("libraryview.vendor")}</th>
        <th class="st"><span class="sr">{t("libraryview.status")}</span></th>
        <th class="st"><span class="sr">{t("libraryview.origin")}</span></th>
        <th>{t("libraryview.model")}</th>
        <th>{t("libraryview.description")}</th>
        <th class="num">{t("libraryview.registers")}</th>
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
          <td class="st"><MapStatus status={e.status} /></td>
          <td class="st origin">
            {#if e.edited}
              <span
                role="img"
                aria-label={t("libraryview.edited_changed_since_it")}
                title={t("libraryview.edited_changed_since_it")}
                ><Icon name="edited" size={14} /></span
              >
            {:else if e.origin === "imported"}
              <span
                role="img"
                aria-label={t("libraryview.imported_added_by_you")}
                title={t("libraryview.imported_added_by_you")}
                ><Icon name="imported" size={14} /></span
              >
            {/if}
          </td>
          <td class="model">{e.model}</td>
          <td class="desc">
            {#if e.error}
              <span class="bad" title={e.error}
                >{t("libraryview.has_errors_error", { error: e.error })}</span
              >
            {:else}
              {e.description ?? ""}
              {#if e.warnings}<span class="warn"
                  >{t("libraryview.warnings_count", {
                    count: e.warnings,
                  })}</span
                >{/if}
            {/if}
          </td>
          <td class="num">{e.error ? "" : e.registers}</td>
          <td class="act">
            {#if confirm?.path === e.path}
              <span class="ask"
                >{confirm.kind === "delete"
                  ? t("libraryview.delete_this_map")
                  : t("libraryview.edits_the_library_s")}</span
              >
              {#if confirm.kind === "delete"}
                <button class="btn small danger" onclick={() => remove(e.path)}
                  >{t("libraryview.delete")}</button
                >
              {:else}
                <button class="btn small accent" onclick={() => edit(e.path)}
                  >{t("libraryview.edit")}</button
                >
              {/if}
              <button class="btn small" onclick={() => (confirm = null)}
                >{t("computedform.cancel")}</button
              >
            {:else}
              <button
                class="btn small accent"
                disabled={!!e.error || bench.running}
                title={bench.running
                  ? t("libraryview.stop_the_test_first")
                  : undefined}
                onclick={() => use(e.path)}>{t("libraryview.use")}</button
              >
              <button
                class="btn small icon ghost"
                title={t("libraryview.explore")}
                aria-label={t("libraryview.explore_model", { model: e.model })}
                disabled={!!e.error}
                onclick={() => (exploring = e.path)}
                ><Icon name="explore" /></button
              >
              <button
                class="btn small icon ghost"
                title={t("libraryview.edit")}
                aria-label={t("libraryview.edit_model", { model: e.model })}
                onclick={() => (confirm = { path: e.path, kind: "edit" })}
                ><Icon name="edit" /></button
              >
              <button
                class="btn small icon ghost"
                title={t("libraryview.duplicate")}
                aria-label={t("libraryview.duplicate_model", {
                  model: e.model,
                })}
                disabled={!!e.error}
                onclick={() => duplicate(e)}><Icon name="duplicate" /></button
              >
              <button
                class="btn small icon ghost del"
                title={t("libraryview.delete")}
                aria-label={t("libraryview.delete_model", { model: e.model })}
                onclick={() => (confirm = { path: e.path, kind: "delete" })}
                ><Icon name="trash" /></button
              >
            {/if}
          </td>
        </tr>
      {:else}
        <tr>
          <td colspan="7" class="empty">
            {#if bench.library.length}
              {t("libraryview.no_maps_match_search", { search })}
            {:else}
              {t("libraryview.no_maps_yet_import")}
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

  /* The status icon in its own narrow column, so the icons line up */
  .st {
    width: 16px;
    padding-right: 0;
  }

  td.st :global(.status),
  td.origin span {
    display: flex;
  }

  td.origin {
    color: var(--text-3);
  }

  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
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
