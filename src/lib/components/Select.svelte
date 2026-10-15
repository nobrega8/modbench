<!--
  A dropdown that follows the app's theme: a filled trigger like .input and a
  card-like list that opens with the same motion as the other popovers. The
  list is position: fixed, so scrolling or clipped containers do not cut it.
  Keyboard: arrows, Home/End, Enter/Space, Escape, Tab, and typing a letter.
-->
<script lang="ts" module>
  export interface Option<V> {
    value: V;
    label: string;
    disabled?: boolean;
  }
</script>

<script lang="ts" generics="T extends string | number">
  import { popIn, popOut } from "#lib/motion.ts";

  let {
    value = $bindable(),
    options,
    disabled = false,
    label: ariaLabel,
    title,
    width,
    placeholder = "—",
    class: klass = "",
    onchange,
  }: {
    value: T;
    options: Option<T>[];
    disabled?: boolean;
    label?: string;
    title?: string;
    width?: string;
    placeholder?: string;
    class?: string;
    onchange?: (value: T) => void;
  } = $props();

  const id = `select-${Math.random().toString(36).slice(2, 9)}`;
  let trigger = $state<HTMLButtonElement>();
  let list = $state<HTMLDivElement>();
  let open = $state(false);
  let active = $state(-1);
  // Below the trigger (top) or above it (bottom), in viewport pixels.
  let place = $state({
    left: 0,
    top: 0,
    bottom: 0,
    width: 0,
    maxHeight: 280,
    above: false,
  });
  let typed = "";
  let typedAt = 0;

  const current = $derived(options.find((o) => o.value === value));

  function position() {
    if (!trigger) return;
    const r = trigger.getBoundingClientRect();
    const gap = 4;
    const margin = 8;
    const below = window.innerHeight - r.bottom - gap - margin;
    const aboveSpace = r.top - gap - margin;
    const wanted = Math.min(280, options.length * 30 + 10);
    const above = below < wanted && aboveSpace > below;
    const maxHeight = Math.max(80, Math.min(280, above ? aboveSpace : below));
    const width = Math.max(r.width, 120);
    place = {
      left: Math.max(
        margin,
        Math.min(r.left, window.innerWidth - width - margin),
      ),
      top: r.bottom + gap,
      bottom: window.innerHeight - r.top + gap,
      width,
      maxHeight,
      above,
    };
  }

  function show() {
    if (disabled || open) return;
    position();
    open = true;
    active = Math.max(
      0,
      options.findIndex((o) => o.value === value),
    );
    queueMicrotask(scrollToActive);
  }

  function hide(focus = true) {
    open = false;
    if (focus) trigger?.focus();
  }

  function choose(i: number) {
    const o = options[i];
    if (!o || o.disabled) return;
    hide();
    if (o.value !== value) {
      value = o.value;
      onchange?.(o.value);
    }
  }

  function move(step: number) {
    if (!options.length) return;
    let i = active;
    for (let n = 0; n < options.length; n++) {
      i = (i + step + options.length) % options.length;
      if (!options[i].disabled) break;
    }
    active = i;
    scrollToActive();
  }

  function scrollToActive() {
    list
      ?.querySelector<HTMLElement>(`[data-index="${active}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }

  function onKey(e: KeyboardEvent) {
    if (disabled) return;
    if (!open) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Home":
        e.preventDefault();
        active = -1;
        move(1);
        break;
      case "End":
        e.preventDefault();
        active = options.length;
        move(-1);
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        choose(active);
        break;
      case "Escape":
        e.preventDefault();
        hide();
        break;
      case "Tab":
        hide(false);
        break;
      default:
        // Typing jumps to the next option that starts with the letters.
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
          const now = Date.now();
          typed = (now - typedAt < 700 ? typed : "") + e.key.toLowerCase();
          typedAt = now;
          const i = options.findIndex(
            (o) => !o.disabled && o.label.toLowerCase().startsWith(typed),
          );
          if (i >= 0) {
            active = i;
            scrollToActive();
          }
        }
    }
  }

  // Close when clicking elsewhere, and follow the trigger when the window
  // changes; scrolling outside the list closes it.
  $effect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      const t = e.target as Node;
      if (!trigger?.contains(t) && !list?.contains(t)) hide(false);
    };
    const scrolled = (e: Event) => {
      if (!list?.contains(e.target as Node)) hide(false);
    };
    window.addEventListener("pointerdown", outside, true);
    window.addEventListener("resize", position);
    window.addEventListener("scroll", scrolled, true);
    return () => {
      window.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", scrolled, true);
    };
  });
</script>

<button
  bind:this={trigger}
  type="button"
  class="input select {klass}"
  class:open
  style:width
  {disabled}
  {title}
  aria-label={ariaLabel}
  role="combobox"
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={open ? id : undefined}
  aria-activedescendant={open && active >= 0 ? `${id}-${active}` : undefined}
  onclick={() => (open ? hide() : show())}
  onkeydown={onKey}
>
  <span class="value" class:empty={!current}
    >{current?.label ?? placeholder}</span
  >
  <svg class="chevron" viewBox="0 0 12 12" aria-hidden="true"
    ><path d="M3 4.5l3 3 3-3" /></svg
  >
</button>

{#if open}
  <div
    bind:this={list}
    {id}
    class="list"
    class:above={place.above}
    role="listbox"
    tabindex="-1"
    aria-label={ariaLabel}
    onclick={(e) => e.preventDefault()}
    onkeydown={onKey}
    style:left="{place.left}px"
    style:top={place.above ? null : `${place.top}px`}
    style:bottom={place.above ? `${place.bottom}px` : null}
    style:min-width="{place.width}px"
    style:max-height="{place.maxHeight}px"
    in:popIn={{ above: place.above }}
    out:popOut
  >
    {#each options as o, i (o.value)}
      <div
        id="{id}-{i}"
        data-index={i}
        class="option"
        class:active={i === active}
        class:selected={o.value === value}
        class:disabled={o.disabled}
        role="option"
        tabindex="-1"
        aria-selected={o.value === value}
        aria-disabled={o.disabled}
        onpointermove={() => (active = i)}
        onpointerdown={(e) => e.preventDefault()}
        onclick={(e) => {
          // Inside a <label>, a click would also reopen the list.
          e.preventDefault();
          choose(i);
        }}
        onkeydown={onKey}
      >
        <span class="label">{o.label}</span>
        {#if o.value === value}
          <svg class="check" viewBox="0 0 16 16" aria-hidden="true"
            ><path d="M4 8.5l2.5 2.5L12 5.5" /></svg
          >
        {/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .select {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }

  .select:disabled {
    cursor: default;
  }

  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value.empty {
    color: var(--text-3);
  }

  .chevron {
    flex: none;
    width: 12px;
    height: 12px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.6;
    transition: transform 0.2s var(--ease);
  }

  .select.open .chevron {
    transform: rotate(180deg);
  }

  /* The same card as the other popovers: --card fill, --e3 elevation */
  .list {
    position: fixed;
    z-index: 100;
    max-width: calc(100vw - 16px);
    overflow-y: auto;
    padding: 4px;
    border-radius: 11px;
    background: var(--card);
    box-shadow: var(--e3);
    transform-origin: top left;
    outline: none;
  }

  .list.above {
    transform-origin: bottom left;
  }

  .option {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 28px;
    padding: 0 8px 0 10px;
    border-radius: 7px;
    color: var(--text);
    white-space: nowrap;
    cursor: pointer;
    user-select: none;
  }

  .option.active {
    background: rgb(var(--fg-rgb) / 0.07);
  }

  .option.selected {
    font-weight: 500;
  }

  .option.disabled {
    opacity: 0.4;
    cursor: default;
  }

  .check {
    flex: none;
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
