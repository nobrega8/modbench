<script lang="ts">
  import { api, errorText } from "$lib/api";
  import { bench } from "$lib/state.svelte";
  import type { OnlineEntry } from "$lib/types";
  import VendorIcon from "./VendorIcon.svelte";

  interface Props {
    open: boolean;
    /** Called with a status line after maps were added. */
    onadded?: (text: string) => void;
  }

  let { open = $bindable(), onadded }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let entries = $state.raw<OnlineEntry[] | null>(null);
  let loadError = $state<string | null>(null);
  let loading = $state(false);
  let search = $state("");
  let pending = $state<Set<string>>(new Set());
  let rowError = $state<Record<string, string>>({});

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      dialog.showModal();
      if (!entries && !loading) load();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  });

  async function load() {
    loading = true;
    loadError = null;
    try {
      entries = await api.libraryOnlineIndex();
    } catch (e) {
      loadError = errorText(e);
    } finally {
      loading = false;
    }
  }

  const key = (vendor: string, model: string) =>
    `${vendor.trim().toLowerCase()}\n${model.trim().toLowerCase()}`;

  const local = $derived(
    new Set(bench.library.map((e) => key(e.vendor, e.model))),
  );

  const shown = $derived(
    (entries ?? []).filter((e) => {
      const q = search.trim().toLowerCase();
      return (
        !q ||
        e.vendor.toLowerCase().includes(q) ||
        e.model.toLowerCase().includes(q) ||
        (e.description ?? "").toLowerCase().includes(q)
      );
    }),
  );

  async function add(entry: OnlineEntry) {
    const replacing = local.has(key(entry.vendor, entry.model));
    pending = new Set(pending).add(entry.path);
    const { [entry.path]: _, ...rest } = rowError;
    rowError = rest;
    try {
      const [result] = await api.libraryOnlineImport([entry.path]);
      if (result?.error) {
        rowError = { ...rowError, [entry.path]: result.error };
      } else {
        await bench.refreshLibrary();
        onadded?.(
          `${entry.vendor} ${entry.model} ${replacing ? "updated in" : "added to"} the library.`,
        );
      }
    } catch (e) {
      rowError = { ...rowError, [entry.path]: errorText(e) };
    } finally {
      const next = new Set(pending);
      next.delete(entry.path);
      pending = next;
    }
  }
</script>

<dialog
  bind:this={dialog}
  onclose={() => (open = false)}
  aria-labelledby="online-title"
>
  <header>
    <div>
      <h2 id="online-title">Online library</h2>
      <p>
        Maps shared in the
        <span class="mono">nobrega8/mbmap-library</span> repository on GitHub.
      </p>
    </div>
    <input
      class="input search"
      type="search"
      placeholder="Search vendor or model"
      bind:value={search}
      aria-label="Search the online library"
    />
  </header>

  <div class="list">
    {#if loading}
      <p class="note">Loading the online library…</p>
    {:else if loadError}
      <div class="note">
        <p class="bad">{loadError}</p>
        <button class="btn" onclick={load}>Try again</button>
      </div>
    {:else}
      {#each shown as e (e.path)}
        {@const inLibrary = local.has(key(e.vendor, e.model))}
        {@const busy = pending.has(e.path)}
        <div class="row">
          <VendorIcon vendor={e.vendor} website={e.vendor_website} size={28} />
          <div class="text">
            <div class="title">
              <span class="model">{e.model}</span>
              <span class="vendor">{e.vendor}</span>
            </div>
            {#if rowError[e.path]}
              <div class="desc bad">{rowError[e.path]}</div>
            {:else if e.description}
              <div class="desc" title={e.description}>{e.description}</div>
            {/if}
          </div>
          {#if e.status === "unverified"}<span
              class="tag"
              title="Converted from another catalogue. Not checked against the manufacturer's documentation yet."
              >Unverified</span
            >{/if}
          <span class="count">{e.registers} registers</span>
          {#if inLibrary}
            <button
              class="btn ghost small"
              disabled={busy}
              title="Download it again and replace the copy in your library"
              onclick={() => add(e)}>{busy ? "Downloading…" : "Replace"}</button
            >
          {:else}
            <button
              class="btn accent small"
              disabled={busy}
              onclick={() => add(e)}>{busy ? "Adding…" : "Add"}</button
            >
          {/if}
        </div>
      {:else}
        <p class="note">
          {entries?.length ? `No maps match “${search}”.` : "No maps yet."}
        </p>
      {/each}
    {/if}
  </div>

  <footer>
    <span class="hint"
      >Check a map against the manufacturer's documentation before you write to
      a live device.</span
    >
    <button class="btn" onclick={() => (open = false)}>Done</button>
  </footer>
</dialog>

<style>
  dialog {
    width: min(760px, calc(100vw - 48px));
    max-height: min(640px, calc(100vh - 80px));
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
    justify-content: space-between;
    gap: 16px;
    padding: 20px 22px 12px;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.011em;
  }

  header p {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }

  .search {
    width: 220px;
    flex: none;
  }

  .list {
    flex: 1;
    min-height: 120px;
    overflow: auto;
    padding: 0 12px;
  }

  .row {
    display: grid;
    grid-template-columns: auto 1fr auto auto auto;
    align-items: center;
    gap: 14px;
    padding: 10px;
    border-radius: 10px;
  }

  .row + .row {
    box-shadow: inset 0 1px 0 var(--border);
  }

  .row:hover {
    background: var(--accent-soft);
  }

  .text {
    min-width: 0;
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .model {
    font-weight: 600;
  }

  .vendor {
    color: var(--text-2);
  }

  .desc {
    color: var(--text-3);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    padding: 1px 7px;
    border-radius: 5px;
    background: var(--field);
    color: var(--text-2);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .count {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
  }

  .small {
    height: 28px;
    min-width: 72px;
    padding: 0 12px;
  }

  .note {
    padding: 16px 10px;
    margin: 0;
    color: var(--text-2);
  }

  .note p {
    margin: 0 0 10px;
  }

  .bad {
    color: var(--bad);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 22px 18px;
  }

  .hint {
    font-size: 12px;
    color: var(--text-3);
  }
</style>
