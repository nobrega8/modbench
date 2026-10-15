<!--
  The right-click menu, drawn once for the whole window (see
  contextmenu.svelte.ts). It looks like the lists of the dropdowns, stays
  inside the window, and closes on a click outside, Escape, scrolling or a
  resize.
-->
<script lang="ts">
  import {
    contextMenu,
    fallback,
    type MenuItem,
  } from "#lib/contextmenu.svelte.ts";
  import { popIn, popOut } from "#lib/motion.ts";

  const open = $derived(contextMenu.open);
  let active = $state(-1);

  function run(item: MenuItem) {
    if (item.disabled) return;
    contextMenu.close();
    item.action();
  }

  /** Index of the next item that can be chosen, going `step` at a time. */
  function next(from: number, step: number): number {
    const items = open?.items ?? [];
    for (let k = 1; k <= items.length; k++) {
      const i = (from + step * k + items.length * k) % items.length;
      const item = items[i];
      if (item && !item.disabled) return i;
    }
    return -1;
  }

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") contextMenu.close();
    else if (e.key === "ArrowDown") active = next(active, 1);
    else if (e.key === "ArrowUp") active = next(active < 0 ? 0 : active, -1);
    else if (e.key === "Enter" || e.key === " ") {
      const item = open.items[active];
      if (item) run(item);
    } else return;
    e.preventDefault();
  }

  /** Keeps the menu inside the window, and closes it on a click outside. */
  function placed(node: HTMLElement) {
    const r = node.getBoundingClientRect();
    const x = Math.min(open?.x ?? 0, innerWidth - r.width - 8);
    const y = Math.min(open?.y ?? 0, innerHeight - r.height - 8);
    node.style.left = `${Math.max(8, x)}px`;
    node.style.top = `${Math.max(8, y)}px`;
    node.focus();
    const away = (e: Event) => {
      if (!node.contains(e.target as Node)) contextMenu.close();
    };
    const close = () => contextMenu.close();
    addEventListener("pointerdown", away, true);
    addEventListener("resize", close);
    addEventListener("blur", close);
    addEventListener("wheel", close, { passive: true });
    return {
      destroy() {
        removeEventListener("pointerdown", away, true);
        removeEventListener("resize", close);
        removeEventListener("blur", close);
        removeEventListener("wheel", close);
      },
    };
  }

  // Nothing is highlighted until the keyboard or the pointer picks.
  $effect(() => {
    void open;
    active = -1;
  });
</script>

<svelte:window oncontextmenu={fallback} />

{#if open}
  {#key open}
    <div
      class="menu"
      role="menu"
      tabindex="-1"
      use:placed
      onkeydown={onKey}
      oncontextmenu={(e) => e.preventDefault()}
      in:popIn
      out:popOut
    >
      {#each open.items as item, i (i)}
        {#if item}
          <button
            class="item"
            class:active={active === i}
            class:danger={item.danger}
            role="menuitem"
            tabindex="-1"
            disabled={item.disabled}
            onpointermove={() => (active = item.disabled ? -1 : i)}
            onclick={() => run(item)}
            ><span>{item.label}</span>{#if item.hint}<span class="hint"
                >{item.hint}</span
              >{/if}</button
          >
        {:else}
          <div class="sep" role="separator"></div>
        {/if}
      {/each}
    </div>
  {/key}
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 200;
    display: flex;
    flex-direction: column;
    min-width: 180px;
    max-width: calc(100vw - 16px);
    padding: 4px;
    border-radius: 11px;
    background: var(--card);
    box-shadow: var(--e3);
    transform-origin: top left;
    outline: none;
  }

  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    min-height: 28px;
    padding: 0 10px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
    user-select: none;
  }

  .item.active {
    background: rgb(var(--fg-rgb) / 0.07);
  }

  .item.danger {
    color: var(--bad);
  }

  .item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .hint {
    color: var(--text-3);
    font-size: 12px;
  }

  .sep {
    height: 1px;
    margin: 4px 6px;
    background: rgb(var(--fg-rgb) / 0.08);
  }
</style>
