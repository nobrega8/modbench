<script lang="ts">
  import { api, errorText } from "#lib/api.ts";
  import { slideIn, smoothResize } from "#lib/motion.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { OnlineEntry, Parsed } from "#lib/types.ts";
  import { fade } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import MapStatus from "./MapStatus.svelte";
  import VendorIcon from "./VendorIcon.svelte";

  interface Props {
    open: boolean;
    /** Called with a status line after maps were added. */
    onadded?: (text: string) => void;
  }

  let { open = $bindable(), onadded }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let searchBox: HTMLInputElement | undefined = $state();
  let entries = $state.raw<OnlineEntry[] | null>(null);
  let loadError = $state<string | null>(null);
  let loading = $state(false);
  let search = $state("");
  let pending = $state<Set<string>>(new Set());
  let rowError = $state<Record<string, string>>({});
  /** The map whose details are open, and the details loaded so far. */
  let infoPath = $state<string | null>(null);
  let infoPop: HTMLElement | undefined = $state();
  let infoAt = $state({ top: 0, left: 0 });
  let closed: { path: string | null; at: number } = { path: null, at: 0 };
  const infoEntry = $derived(
    infoPath ? (entries ?? []).find((x) => x.path === infoPath) : undefined,
  );
  let details = $state<Record<string, Parsed | { error: string }>>({});

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
    dir = 0;
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

  /** The vendor whose maps are shown, or null for the list of vendors. */
  let vendor = $state<string | null>(null);
  /** Where the next view slides in from: 1 going into a vendor, -1 back. */
  let dir = $state(0);

  /** What the list shows; a change swaps the whole view. */
  const view = $derived(
    loading ? "loading" : loadError ? "error" : `vendor:${vendor ?? ""}`,
  );

  const vendorKey = (name: string) => name.trim().toLowerCase();
  const query = $derived(search.trim().toLowerCase());

  function matches(e: OnlineEntry): boolean {
    return (
      e.model.toLowerCase().includes(query) ||
      (e.description ?? "").toLowerCase().includes(query)
    );
  }

  /**
   * Vendors, filtered by the search. A vendor stays when its name matches, or
   * when some of its maps do (then only those are counted).
   */
  const vendors = $derived.by(() => {
    const groups = new Map<
      string,
      { name: string; website: string | null; total: number; hits: number }
    >();
    for (const e of entries ?? []) {
      const k = vendorKey(e.vendor);
      const g = groups.get(k) ?? {
        name: e.vendor.trim(),
        website: e.vendor_website,
        total: 0,
        hits: 0,
      };
      g.total++;
      g.website ??= e.vendor_website;
      if (query && matches(e)) g.hits++;
      groups.set(k, g);
    }
    return [...groups.values()]
      .filter((g) => !query || g.name.toLowerCase().includes(query) || g.hits)
      .map((g) => ({
        ...g,
        shown:
          query && !g.name.toLowerCase().includes(query) ? g.hits : g.total,
      }))
      .sort((a, b) => a.name.localeCompare(b.name));
  });

  const shown = $derived(
    (entries ?? [])
      .filter((e) => vendor !== null && vendorKey(e.vendor) === vendor)
      .filter((e) => !query || matches(e))
      .sort((a, b) => a.model.localeCompare(b.model)),
  );

  const vendorName = $derived(
    vendors.find((g) => vendorKey(g.name) === vendor)?.name ??
      (entries ?? []).find((e) => vendorKey(e.vendor) === vendor)?.vendor ??
      "",
  );

  function openVendor(name: string) {
    dir = 1;
    vendor = vendorKey(name);
    search = "";
    searchBox?.focus();
  }

  function back() {
    dir = -1;
    vendor = null;
    search = "";
    searchBox?.focus();
  }

  /** Esc inside a vendor goes back to the vendors; on them it closes. */
  function oncancel(e: Event) {
    if (vendor === null) return;
    e.preventDefault();
    back();
  }

  /** Opens the details of a map in a popup next to its button. */
  async function toggleInfo(entry: OnlineEntry, button: HTMLElement) {
    // A click on the button of the open popup first closes it (light
    // dismiss); don't open it again right away.
    if (closed.path === entry.path && performance.now() - closed.at < 400)
      return;
    if (infoPath === entry.path) {
      infoPop?.hidePopover();
      return;
    }
    const r = button.getBoundingClientRect();
    const width = 420;
    infoAt = {
      top: Math.min(r.bottom + 6, window.innerHeight - 200),
      left: Math.max(
        12,
        Math.min(r.right - width, window.innerWidth - width - 12),
      ),
    };
    infoPath = entry.path;
    infoPop?.showPopover();
    if (details[entry.path] && !("error" in details[entry.path])) return;
    try {
      const parsed = await api.libraryOnlineMap(entry.path);
      details = { ...details, [entry.path]: parsed };
    } catch (e) {
      details = { ...details, [entry.path]: { error: errorText(e) } };
    }
  }

  const PROTOCOL = {
    tcp: "Modbus TCP",
    rtu: "Modbus RTU",
    "tcp+rtu": "Modbus TCP and RTU",
  };

  /** The general data of a map, as label/value pairs (empty ones left out). */
  function facts(entry: OnlineEntry, p: Parsed): [string, string][] {
    const m = p.map?.meta;
    if (!m) return [];
    const serial = [
      m.baudrate ? `${m.baudrate} baud` : null,
      m.data_bits || m.parity || m.stop_bits
        ? `${m.data_bits ?? 8}${m.parity ?? "E"}${m.stop_bits ?? 1}`
        : null,
    ].filter(Boolean);
    const firmware = [
      m.firmware,
      m.firmware_min || m.firmware_max
        ? `${m.firmware_min ?? "any"} to ${m.firmware_max ?? "latest"}`
        : null,
    ].filter(Boolean);
    const rows: [string, string | null | undefined][] = [
      ["Models", m.models?.length ? m.models.join(", ") : null],
      ["Description", m.description],
      [
        "Revision",
        m.revision ? `${m.revision}${m.revised ? `, ${m.revised}` : ""}` : null,
      ],
      ["Firmware", firmware.join("; ") || null],
      ["Registers", String(p.map?.registers.length ?? entry.registers)],
      ["Protocol", m.protocol ? PROTOCOL[m.protocol] : null],
      ["Unit ID", m.unit_id?.toString()],
      ["TCP port", m.port?.toString()],
      ["Serial", serial.join(", ") || null],
      ["Byte order", m.byte_order],
      ["Addresses", m.address_base === null ? null : `${m.address_base}-based`],
      [
        "Max read",
        m.max_read_registers ? `${m.max_read_registers} registers` : null,
      ],
      ["Source", m.source],
      ["Website", m.vendor_website],
      ["File", entry.path],
    ];
    return rows.filter((r): r is [string, string] => !!r[1]);
  }

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
  use:smoothResize
  onclose={() => (open = false)}
  {oncancel}
  aria-labelledby="online-title"
>
  <header data-size>
    {#if vendor !== null}
      <button
        class="btn back"
        onclick={back}
        title="Back to all vendors (Esc)"
        in:fade={{ duration: 160 }}
        ><Icon name="back" size={16} />Vendors</button
      >
    {/if}
    <div class="heading">
      {#key vendor}
        <div in:slideIn={{ dir }}>
          {#if vendor !== null}
            <h2 id="online-title">{vendorName}</h2>
            <p>Online library</p>
          {:else}
            <h2 id="online-title">Online library</h2>
            <p>Maps shared on GitHub.</p>
          {/if}
        </div>
      {/key}
    </div>
    <input
      class="input search"
      type="search"
      placeholder={vendor === null ? "Search vendor or model" : "Search models"}
      bind:this={searchBox}
      bind:value={search}
      aria-label={vendor === null
        ? "Search vendors"
        : `Search ${vendorName} models`}
    />
  </header>

  <div class="list" onscroll={() => infoPop?.hidePopover()}>
    <div data-size>
      {#key view}
        <div in:slideIn={{ dir }}>
          {#if loading}
            <p class="note">Loading the online library…</p>
          {:else if loadError}
            <div class="note">
              <p class="bad">{loadError}</p>
              <button class="btn" onclick={load}>Try again</button>
            </div>
          {:else if vendor === null}
            {#each vendors as g (g.name.toLowerCase())}
              <button
                class="row vendor-row"
                onclick={() => openVendor(g.name)}
                in:fade={{ duration: 140 }}
              >
                <VendorIcon vendor={g.name} website={g.website} size={28} />
                <div class="text">
                  <span class="model">{g.name}</span>
                </div>
                <span class="count"
                  >{g.shown} map{g.shown === 1 ? "" : "s"}{query &&
                  g.shown !== g.total
                    ? ` of ${g.total}`
                    : ""}</span
                >
                <span class="chev" aria-hidden="true">›</span>
              </button>
            {:else}
              <p class="note">
                {entries?.length
                  ? `Nothing matches “${search}”.`
                  : "No maps yet."}
              </p>
            {/each}
          {:else}
            {#each shown as e (e.path)}
              {@const inLibrary = local.has(key(e.vendor, e.model))}
              {@const busy = pending.has(e.path)}
              <div class="row" in:fade={{ duration: 140 }}>
                <VendorIcon
                  vendor={e.vendor}
                  website={e.vendor_website}
                  size={28}
                />
                <div class="text">
                  <div class="title">
                    <span class="model">{e.model}</span>
                  </div>
                  {#if rowError[e.path]}
                    <div class="desc bad">{rowError[e.path]}</div>
                  {:else if e.description}
                    <div class="desc" title={e.description}>
                      {e.description}
                    </div>
                  {/if}
                </div>
                <span class="status"><MapStatus status={e.status} /></span>
                <span class="count">{e.registers} registers</span>
                <button
                  class="btn small icon ghost info-btn"
                  class:on={infoPath === e.path}
                  title="Map details"
                  aria-label="Details of {e.model}"
                  aria-expanded={infoPath === e.path}
                  onclick={(ev) => toggleInfo(e, ev.currentTarget)}
                  ><Icon name="about" size={16} /></button
                >
                {#if inLibrary}
                  <button
                    class="btn ghost small"
                    disabled={busy}
                    title="Download it again and replace the copy in your library"
                    onclick={() => add(e)}
                    >{busy ? "Downloading…" : "Replace"}</button
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
                No maps match “{search}”.
              </p>
            {/each}
          {/if}
        </div>
      {/key}
    </div>
  </div>

  <div
    class="info-pop"
    popover="auto"
    bind:this={infoPop}
    style:top="{infoAt.top}px"
    style:left="{infoAt.left}px"
    ontoggle={(ev) => {
      if ((ev as ToggleEvent).newState !== "closed") return;
      closed = { path: infoPath, at: performance.now() };
      infoPath = null;
    }}
  >
    {#if infoEntry}
      {@const e = infoEntry}
      {@const d = details[e.path]}
      <h3 class="pop-title">{e.vendor} {e.model}</h3>

      {#if !d}
        <p class="muted">Loading the map…</p>
      {:else if "error" in d}
        <p class="bad">{d.error}</p>
      {:else if !d.map}
        <p class="bad">
          The map has errors: {d.diagnostics
            .filter((x) => x.severity === "error")
            .map((x) => x.message)
            .join("; ")}
        </p>
      {:else}
        <dl>
          {#each facts(e, d) as [label, value] (label)}
            <dt>{label}</dt>
            <dd>{value}</dd>
          {/each}
        </dl>
        {#if d.map.meta.sources?.length}
          <h4>Sources</h4>
          <ul>
            {#each d.map.meta.sources as src, i (i)}
              <li>
                {src.title}{src.version ? `, ${src.version}` : ""}{src.date
                  ? ` (${src.date})`
                  : ""}{#if src.url}<br /><span class="url">{src.url}</span
                  >{/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if d.map.meta.notes?.length}
          <h4>Notes</h4>
          <ul>
            {#each d.map.meta.notes as note, i (i)}
              <li>{note}</li>
            {/each}
          </ul>
        {/if}
      {/if}
    {/if}
  </div>

  <footer>
    <span class="hint"
      >Check maps against the vendor's manual before writing.</span
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
    gap: 12px;
    padding: 20px 22px 12px;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  .back {
    flex: none;
    gap: 4px;
    padding: 0 12px 0 6px;
    margin-top: 3px;
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
    /* views slide in sideways; no horizontal scrollbar while they do */
    overflow-x: clip;
    padding: 0 12px;
  }

  /* Fixed columns after the text, so status icons, counts and buttons line
     up from row to row */
  .row {
    display: grid;
    grid-template-columns: 28px 1fr 16px 13ch 28px 96px;
    align-items: center;
    gap: 14px;
    padding: 10px;
    border-radius: 10px;
  }

  .vendor-row {
    grid-template-columns: 28px 1fr auto auto;
    width: 100%;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .chev {
    color: var(--text-3);
    font-size: 18px;
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

  .desc {
    color: var(--text-3);
    font-size: 13px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
  }

  .row:not(.vendor-row) .count {
    text-align: right;
  }

  .status {
    display: flex;
    justify-content: center;
  }

  .row .info-btn {
    width: 28px;
    min-width: 0;
    padding: 0;
    color: var(--text-3);
  }

  .row .info-btn.on {
    color: var(--accent);
  }

  .info-pop {
    position: fixed;
    inset: auto;
    margin: 0;
    width: 420px;
    max-height: min(420px, calc(100vh - 40px));
    overflow: auto;
    padding: 14px 16px;
    border: 0;
    border-radius: 12px;
    background: var(--card);
    color: var(--text);
    box-shadow: var(--e3);
    font-size: 13px;
    user-select: text;
  }

  .info-pop:popover-open {
    animation: rise 0.16s var(--ease);
  }

  .pop-title {
    margin: 0 0 10px;
    font-size: 14px;
    font-weight: 600;
  }

  .info-pop dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    margin: 0;
  }

  .info-pop dt {
    color: var(--text-3);
  }

  .info-pop dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .info-pop h4 {
    margin: 12px 0 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }

  .info-pop ul {
    margin: 0;
    padding-left: 18px;
    color: var(--text-2);
  }

  .info-pop li + li {
    margin-top: 2px;
  }

  .url {
    color: var(--text-3);
    font-family: var(--mono);
    font-size: 12px;
    overflow-wrap: anywhere;
  }

  .muted {
    margin: 0;
    color: var(--text-3);
  }

  .small {
    height: 28px;
    min-width: 72px;
    padding: 0 12px;
  }

  .row .accent.small,
  .row .ghost.small:not(.info-btn) {
    width: 96px;
    min-width: 0;
    padding: 0 8px;
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
