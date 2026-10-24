<script lang="ts">
  // A palette opened with ⌘K (Ctrl+K): type to find a page, a tool, a map of
  // the library or an action, and Enter to go there. A highlight slides to the
  // chosen row as the arrows move it. After EasyUI's Command Menu.
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { slidePill } from "#lib/motion.ts";
  import { modelMatches } from "#lib/search.ts";
  import { bench, type Page, type ToolId } from "#lib/state.svelte.ts";
  import Icon, { type IconName } from "./Icon.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  interface Item {
    id: string;
    label: string;
    icon: IconName;
    /** Text after the label, dimmed (a map's description). */
    sub?: string;
    run: () => void;
  }

  let dialog = $state<HTMLDialogElement>();
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLElement>();
  let query = $state("");
  let active = $state(0);

  const PAGES: { id: Page; label: Key }[] = [
    { id: "main", label: "nav.main" },
    { id: "tools", label: "nav.tools" },
    { id: "library", label: "nav.library" },
    { id: "settings", label: "nav.settings" },
  ];

  const TOOLS: ToolId[] = [
    "identify",
    "addresses",
    "network",
    "simulator",
    "decoder",
    "frames",
    "player",
    "request",
    "stats",
    "alarms",
  ];

  const blocked = $derived(
    (bench.transport === "rtu" && bench.serialPort === "") ||
      (bench.source === "map" && bench.map === null),
  );

  const groups = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const hit = (...texts: (string | undefined)[]) =>
      !q || texts.some((s) => s?.toLowerCase().includes(q));

    const actions: Item[] = [];
    if (bench.running)
      actions.push({
        id: "stop",
        label: t("command.stop"),
        icon: "stoprecord",
        run: () => bench.stop(),
      });
    else if (!blocked)
      actions.push({
        id: "start",
        label: t("command.start"),
        icon: "main",
        run: () => {
          bench.page = "main";
          bench.start();
        },
      });

    const pages: Item[] = PAGES.map((p) => ({
      id: `page:${p.id}`,
      label: t(p.label),
      icon: p.id,
      run: () => (bench.page = p.id),
    }));

    const tools: Item[] = TOOLS.map((id) => ({
      id: `tool:${id}`,
      label: t(`tools.${id}` as Key),
      icon: id,
      run: () => {
        bench.tool = id;
        bench.page = "tools";
      },
    }));

    // A map cannot be swapped while a test reads the open one.
    const maps: Item[] = bench.running
      ? []
      : bench.library
          .filter((e) => !e.error)
          .filter(
            (e) =>
              !q ||
              e.vendor.toLowerCase().includes(q) ||
              modelMatches(q, e.model, e.models) ||
              `${e.vendor} ${e.model}`.toLowerCase().includes(q),
          )
          .map((e) => ({
            id: `map:${e.path}`,
            label: `${e.vendor} ${e.model}`,
            sub: e.description ?? undefined,
            icon: "map",
            run: () => bench.openMap(e.path),
          }));

    return [
      {
        title: t("command.actions"),
        items: actions.filter((i) => hit(i.label)),
      },
      { title: t("nav.pages"), items: pages.filter((i) => hit(i.label)) },
      { title: t("nav.tools"), items: tools.filter((i) => hit(i.label)) },
      // Without a search the maps would bury the rest: show a few.
      { title: t("command.maps"), items: q ? maps : maps.slice(0, 5) },
    ].filter((g) => g.items.length);
  });

  const items = $derived(groups.flatMap((g) => g.items));

  // A new search starts from the first row.
  $effect(() => {
    void query;
    active = 0;
  });

  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      query = "";
      active = 0;
      dialog.showModal();
      input?.focus();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  });

  // ⌘K / Ctrl+K anywhere in the window opens it, or closes it again.
  $effect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key.toLowerCase() !== "k" || !(e.metaKey || e.ctrlKey)) return;
      if (e.shiftKey || e.altKey) return;
      e.preventDefault();
      open = !open;
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  function choose(item: Item | undefined) {
    if (!item) return;
    open = false;
    item.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (!items.length) return;
      const step = e.key === "ArrowDown" ? 1 : -1;
      active = (active + step + items.length) % items.length;
      queueMicrotask(() =>
        list?.querySelector(".item.on")?.scrollIntoView({ block: "nearest" }),
      );
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(items[active]);
    }
  }
</script>

<dialog
  bind:this={dialog}
  onclose={() => (open = false)}
  onclick={(e) => {
    // A click on the backdrop (the dialog box itself, outside its content)
    if (e.target === dialog) open = false;
  }}
  aria-label={t("command.open")}
>
  <div class="box">
    <div class="search">
      <Icon name="addresses" size={16} />
      <input
        bind:this={input}
        bind:value={query}
        {onkeydown}
        placeholder={t("command.placeholder")}
        aria-label={t("command.placeholder")}
        role="combobox"
        aria-expanded="true"
        aria-controls="command-list"
        aria-activedescendant={items[active]
          ? `command-${items[active].id}`
          : undefined}
        spellcheck="false"
        autocomplete="off"
      />
      <kbd>esc</kbd>
    </div>
    <div
      class="list"
      id="command-list"
      role="listbox"
      bind:this={list}
      use:slidePill
    >
      {#each groups as g (g.title)}
        <div class="group" role="presentation">{g.title}</div>
        {#each g.items as item (item.id)}
          {@const i = items.indexOf(item)}
          <button
            class="item"
            class:on={i === active}
            id="command-{item.id}"
            role="option"
            aria-selected={i === active}
            tabindex="-1"
            onpointermove={() => (active = i)}
            onclick={() => choose(item)}
          >
            <Icon name={item.icon} size={16} />
            <span class="label">{item.label}</span>
            {#if item.sub}<span class="sub">{item.sub}</span>{/if}
          </button>
        {/each}
      {:else}
        <p class="empty">
          {t("onlinelibrary.nothing_matches_search", { search: query })}
        </p>
      {/each}
    </div>
  </div>
</dialog>

<style>
  dialog {
    width: min(560px, calc(100vw - 48px));
    margin: 12vh auto auto;
    padding: 0;
    border: 0;
    border-radius: 14px;
    background: var(--card);
    color: var(--text);
    box-shadow: var(--e3);
    overflow: hidden;
  }

  dialog[open] {
    animation: rise 0.2s var(--ease);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.32);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(-6px) scale(0.98);
    }
  }

  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    height: 48px;
    color: var(--text-3);
    box-shadow: inset 0 -1px 0 var(--border);
  }

  .search input {
    flex: 1;
    min-width: 0;
    height: 100%;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 15px;
  }

  kbd {
    padding: 1px 6px;
    border-radius: 5px;
    background: rgb(var(--fg-rgb) / 0.06);
    color: var(--text-3);
    font-family: var(--ui);
    font-size: 11px;
  }

  .list {
    max-height: min(380px, 56vh);
    overflow-y: auto;
    padding: 6px;
  }

  /* The highlight that slides to the chosen row (lib/motion.ts slidePill) */
  .list :global(.pill) {
    border-radius: 8px;
    background: var(--accent-soft);
    box-shadow: none;
  }

  .group {
    padding: 10px 10px 4px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .item.on {
    color: var(--text);
  }

  .label {
    flex: none;
    font-weight: 500;
  }

  .sub {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-3);
  }

  .empty {
    margin: 0;
    padding: 18px 10px;
    color: var(--text-2);
  }

  @media (prefers-reduced-motion: reduce) {
    dialog[open] {
      animation: none;
    }
  }
</style>
