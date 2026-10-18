<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { popIn, popOut } from "#lib/motion.ts";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { errorText } from "#lib/api.ts";
  import { modelMatches } from "#lib/search.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { MAP_EXTENSIONS } from "#lib/types.ts";
  import MapStatus from "./MapStatus.svelte";
  import VendorIcon from "./VendorIcon.svelte";

  interface Props {
    disabled?: boolean;
  }

  let { disabled = false }: Props = $props();

  interface Item {
    key: string;
    openIndex: number | null;
    path: string;
    vendor: string;
    model: string;
    models: string[];
    description: string;
    website: string | null;
    registers: number;
    warnings: number;
    error: string | null;
    status: string | null;
    current: boolean;
    last: boolean;
  }

  let isOpen = $state(false);
  let query = $state("");
  let cursor = $state<string | null>(null);
  let failure = $state<{ name: string; text: string } | null>(null);
  let root: HTMLDivElement | undefined = $state();
  let search: HTMLInputElement | undefined = $state();

  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  const openItems = $derived<Item[]>(
    bench.maps.map((m, i) => ({
      key: `open:${m.path}`,
      openIndex: i,
      path: m.path,
      vendor: m.map.meta.vendor,
      model: m.map.meta.model,
      models: m.map.meta.models ?? [],
      description: m.map.meta.description ?? "",
      website: m.map.meta.vendor_website,
      registers: m.map.registers.length,
      warnings: m.warnings.length,
      error: null,
      status: m.map.meta.extra.find(([k]) => k === "x_status")?.[1] ?? null,
      current: i === bench.activeMap,
      last: false,
    })),
  );

  const libraryItems = $derived<Item[]>(
    bench.library
      .filter((e) => !bench.maps.some((m) => m.path === e.path))
      .map((e) => ({
        key: `lib:${e.path}`,
        openIndex: null,
        path: e.path,
        vendor: e.vendor,
        model: e.model,
        models: e.models,
        description: e.description ?? "",
        website: e.vendor_website,
        registers: e.registers,
        warnings: e.warnings,
        error: e.error,
        status: e.status,
        current: false,
        last: e.path === bench.lastMap,
      })),
  );

  const matches = (it: Item) => {
    const q = query.trim().toLowerCase();
    return (
      !q ||
      it.vendor.toLowerCase().includes(q) ||
      modelMatches(q, it.model, it.models) ||
      it.description.toLowerCase().includes(q) ||
      fileName(it.path).toLowerCase().includes(q)
    );
  };

  const shownOpen = $derived(openItems.filter(matches));
  const shownLibrary = $derived(libraryItems.filter(matches));

  /** Library maps grouped by vendor, "last used" first. */
  const groups = $derived.by(() => {
    const byVendor = new Map<string, Item[]>();
    for (const it of shownLibrary) {
      const list = byVendor.get(it.vendor) ?? [];
      list.push(it);
      byVendor.set(it.vendor, list);
    }
    return [...byVendor.entries()]
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([vendor, items]) => ({
        vendor,
        items: items.sort(
          (a, b) =>
            Number(b.last) - Number(a.last) || a.model.localeCompare(b.model),
        ),
      }));
  });

  /** Everything the keyboard can land on, in the order it is shown. */
  const selectable = $derived([
    ...shownOpen,
    ...groups.flatMap((g) => g.items).filter((i) => !i.error),
  ]);

  const current = $derived(bench.map);

  $effect(() => {
    if (!isOpen) return;
    queueMicrotask(() => search?.focus());
    const click = (e: MouseEvent) => {
      if (root && !root.contains(e.target as Node)) close();
    };
    window.addEventListener("click", click, true);
    return () => window.removeEventListener("click", click, true);
  });

  $effect(() => {
    // Keep the highlighted entry valid while the list changes.
    if (!selectable.some((i) => i.key === cursor)) {
      cursor =
        selectable.find((i) => i.current)?.key ?? selectable[0]?.key ?? null;
    }
  });

  function toggle() {
    if (disabled) return;
    if (isOpen) close();
    else {
      isOpen = true;
      query = "";
      failure = null;
    }
  }

  function close() {
    isOpen = false;
  }

  async function choose(item: Item) {
    if (item.error) return;
    failure = null;
    if (item.openIndex !== null) {
      bench.selectMap(item.openIndex);
      close();
      return;
    }
    try {
      await bench.openMap(item.path);
      close();
    } catch (e) {
      // Usually the file was moved or deleted since the library was listed.
      failure = { name: `${item.vendor} ${item.model}`, text: errorText(e) };
      close();
      bench.refreshLibrary().catch(() => {});
    }
  }

  async function chooseFile() {
    close();
    const path = await openDialog({
      multiple: false,
      directory: false,
      filters: [
        { name: t("mappicker.modbus_map"), extensions: MAP_EXTENSIONS },
      ],
    });
    if (typeof path !== "string") return;
    failure = null;
    try {
      await bench.openMap(path);
    } catch (e) {
      failure = { name: fileName(path), text: errorText(e) };
    }
  }

  function browseOnline() {
    close();
    bench.onlineRequested = true;
    bench.page = "library";
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    const keys = selectable.map((i) => i.key);
    const at = cursor ? keys.indexOf(cursor) : -1;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!keys.length) return;
      const next = e.key === "ArrowDown" ? at + 1 : at - 1;
      cursor = keys[(next + keys.length) % keys.length];
      queueMicrotask(() =>
        root
          ?.querySelector(`[data-key="${CSS.escape(cursor ?? "")}"]`)
          ?.scrollIntoView({ block: "nearest" }),
      );
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = selectable.find((i) => i.key === cursor);
      if (item) choose(item);
    }
  }
</script>

{#snippet row(it: Item, showVendor: boolean)}
  <button
    type="button"
    role="option"
    aria-selected={it.current}
    class="item"
    class:cursor={cursor === it.key}
    class:current={it.current}
    class:broken={!!it.error}
    disabled={!!it.error}
    data-key={it.key}
    title={it.error ?? (it.description || undefined)}
    onmousemove={() => !it.error && (cursor = it.key)}
    onclick={() => choose(it)}
  >
    <VendorIcon vendor={it.vendor} website={it.website} size={22} />
    <span class="txt">
      <span class="model-row"
        ><span class="model">{it.model}</span><MapStatus
          status={it.status}
          size={14}
        /></span
      >
      <span class="sub">
        {#if it.error}
          <span class="bad"
            >{t("libraryview.has_errors_error", { error: it.error })}</span
          >
        {:else}
          {[
            showVendor ? it.vendor : it.description,
            it.warnings
              ? t("libraryview.warnings_count", { count: it.warnings })
              : "",
          ]
            .filter(Boolean)
            .join(" · ")}
        {/if}
      </span>
    </span>
    {#if it.current}<span class="tag">{t("mappicker.current")}</span>{/if}
    {#if it.last && !it.current}<span class="tag soft"
        >{t("mappicker.last_used")}</span
      >{/if}
    {#if !it.error}<span class="count"
        >{it.registers}
        <span class="unit">{t("mappicker.registers")}</span></span
      >{/if}
  </button>
{/snippet}

<div class="picker" bind:this={root}>
  <button
    type="button"
    class="trigger"
    class:empty={!current}
    aria-haspopup="listbox"
    aria-expanded={isOpen}
    {disabled}
    onclick={toggle}
  >
    {#if current}
      <VendorIcon
        vendor={current.map.meta.vendor}
        website={current.map.meta.vendor_website}
        size={24}
      />
      <span class="txt">
        <span class="model-row"
          ><span class="model">{current.map.meta.model}</span><MapStatus
            status={current.map.meta.extra.find(([k]) => k === "x_status")?.[1]}
            size={14}
          /></span
        >
        <span class="sub"
          >{current.map.meta.vendor} · {t("mappicker.registers_count", {
            count: current.map.registers.length,
          })}{#if current.warnings.length}
            · {t("libraryview.warnings_count", {
              count: current.warnings.length,
            })}{/if}</span
        >
      </span>
    {:else}
      <span class="txt">
        <span class="model">{t("mappicker.choose_a_map")}</span>
        <span class="sub">{t("mappicker.from_your_library_a")}</span>
      </span>
    {/if}
    <svg class="chev" viewBox="0 0 10 6" aria-hidden="true"
      ><path d="M1 1l4 4 4-4" /></svg
    >
  </button>

  {#if failure}
    <p class="problem" role="alert">
      {t("mappicker.could_not_open_name", {
        name: failure.name,
        text: failure.text,
      })}
    </p>
  {:else if bench.mapError}
    <p class="problem" role="alert">
      {t("mappicker.file_has_errors_and", {
        file: fileName(bench.mapError.path),
      })}
    </p>
  {/if}

  {#if isOpen}
    <div
      class="panel"
      role="presentation"
      onkeydown={onKey}
      in:popIn
      out:popOut
    >
      <input
        bind:this={search}
        class="input find"
        type="search"
        placeholder={t("mappicker.search_vendor_model_or")}
        bind:value={query}
        aria-label={t("mappicker.search_maps")}
        autocomplete="off"
        spellcheck="false"
      />

      <div class="list" role="listbox" aria-label={t("mappicker.device_maps")}>
        {#if shownOpen.length}
          <div class="key head">{t("mappicker.open")}</div>
          {#each shownOpen as it (it.key)}{@render row(it, true)}{/each}
        {/if}

        {#each groups as g (g.vendor)}
          <div class="key head">{g.vendor}</div>
          {#each g.items as it (it.key)}{@render row(it, false)}{/each}
        {/each}

        {#if !shownOpen.length && !groups.length}
          <p class="note">
            {#if query.trim()}
              {t("mappicker.no_map_matches_query", { query: query.trim() })}
            {:else}
              {t("mappicker.no_maps_yet_open")}
            {/if}
          </p>
        {/if}
      </div>

      <div class="foot">
        <button type="button" class="btn" onclick={chooseFile}
          >{t("mappicker.open_file")}</button
        >
        <button type="button" class="btn ghost" onclick={browseOnline}
          >{t("mappicker.online_library")}</button
        >
      </div>
    </div>
  {/if}
</div>

<style>
  .picker {
    position: relative;
    width: 330px;
    min-width: 0;
  }

  .trigger {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 44px;
    padding: 0 12px;
    border: 0;
    border-radius: 10px;
    background: var(--field);
    color: var(--text);
    text-align: left;
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      box-shadow 0.14s var(--ease);
  }

  .trigger:hover:not(:disabled) {
    background: rgb(var(--fg-rgb) / 0.1);
  }

  .trigger[aria-expanded="true"] {
    background: var(--card);
    box-shadow: var(--ring-focus);
  }

  .trigger:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .trigger.empty .model {
    color: var(--text-2);
  }

  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    line-height: 1.25;
  }

  /* The model name, then its status icon (verified, official,
     unofficial), as in the Library */
  .model-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .model {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 12px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chev {
    flex: none;
    width: 10px;
    height: 6px;
    fill: none;
    stroke: var(--text-3);
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .problem {
    margin: 6px 2px 0;
    font-size: 12px;
    color: var(--bad);
  }

  .panel {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    z-index: 30;
    width: 460px;
    max-width: calc(100vw - 48px);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-radius: 14px;
    background: var(--card);
    box-shadow: var(--e3);
    transform-origin: top left;
  }

  .find {
    width: 100%;
  }

  .list {
    max-height: 340px;
    overflow: auto;
    padding: 2px;
  }

  .head {
    padding: 10px 10px 4px;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: 9px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }

  .item.cursor:not(:disabled),
  .item:hover:not(:disabled) {
    background: var(--accent-soft);
  }

  .item.current {
    box-shadow: inset 0 0 0 1.5px rgb(var(--fg-rgb) / 0.85);
  }

  .item.broken {
    cursor: default;
    opacity: 0.7;
  }

  .tag {
    flex: none;
    padding: 1px 7px;
    border-radius: 5px;
    background: var(--accent);
    color: var(--on-accent);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .tag.soft {
    background: var(--field);
    color: var(--text-2);
  }

  .count {
    flex: none;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
  }

  .unit {
    color: var(--text-3);
  }

  .bad {
    color: var(--bad);
  }

  .note {
    margin: 0;
    padding: 18px 10px;
    color: var(--text-2);
    font-size: 13px;
  }

  .foot {
    display: flex;
    justify-content: space-between;
    padding: 6px 2px 0;
    box-shadow: inset 0 1px 0 var(--border);
  }
</style>
