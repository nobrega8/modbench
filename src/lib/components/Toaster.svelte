<script lang="ts">
  // The stack of notes from lib/toast.svelte.ts. The newest is in front and
  // the older ones peek out behind it; with the pointer over it the stack
  // fans out so each note can be read. After EasyUI's Notification Stack.
  import { t } from "#lib/i18n.svelte.ts";
  import { toasts } from "#lib/toast.svelte.ts";
  import Icon from "#lib/components/Icon.svelte";
  import { cubicOut } from "svelte/easing";
  import { fly } from "svelte/transition";

  const reduced =
    typeof matchMedia === "function" &&
    matchMedia("(prefers-reduced-motion: reduce)").matches;

  let open = $state(false);
  let heights = $state<Record<number, number>>({});

  /** Where each note sits: fanned out by the heights of the newer ones, or
   * tucked behind the front one. */
  function place(i: number) {
    if (open) {
      let y = 0;
      for (const n of toasts.list.slice(0, i)) y += (heights[n.id] ?? 48) + 8;
      return `translateY(${-y}px)`;
    }
    return `translateY(${-i * 8}px) scale(${1 - i * 0.04})`;
  }

  const front = $derived(heights[toasts.list[0]?.id] ?? 48);
  const tall = $derived(
    open
      ? toasts.list.reduce((s, n) => s + (heights[n.id] ?? 48) + 8, 0)
      : front + (toasts.list.length - 1) * 8,
  );

  function leave() {
    open = false;
    toasts.resume();
  }
</script>

{#if toasts.list.length}
  <section
    class="stack"
    aria-label={t("toaster.label")}
    style:height="{tall}px"
    onpointerenter={() => {
      open = true;
      toasts.pause();
    }}
    onpointerleave={leave}
  >
    {#each toasts.list as n, i (n.id)}
      <div
        class="toast"
        class:bad={!n.ok}
        class:behind={!open && i > 0}
        role={n.ok ? "status" : "alert"}
        style:transform={place(i)}
        style:z-index={toasts.list.length - i}
        style:height="{!open && i > 0 ? front : (heights[n.id] ?? 48)}px"
        in:fly={{ y: reduced ? 0 : 16, duration: 240, easing: cubicOut }}
        out:fly={{ x: reduced ? 0 : 48, duration: 180, easing: cubicOut }}
      >
        <div
          class="body"
          bind:offsetHeight={() => heights[n.id], (h) => (heights[n.id] = h)}
        >
          <span class="dot" aria-hidden="true"></span>
          <p>{n.text}</p>
          {#if n.undo}
            <button class="undo" onclick={() => toasts.undo(n.id)}
              >{n.undo}</button
            >
          {/if}
          <button
            class="close"
            title={t("action.dismiss")}
            aria-label={t("action.dismiss")}
            onclick={() => toasts.dismiss(n.id)}
            ><Icon name="close" size={14} /></button
          >
        </div>
      </div>
    {/each}
  </section>
{/if}

<style>
  .stack {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 50;
    width: min(360px, calc(100vw - 32px));
    transition: height 0.28s var(--ease);
  }

  .toast {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    overflow: hidden;
    border-radius: 12px;
    background: var(--card);
    box-shadow: var(--e3);
    transform-origin: bottom center;
    transition:
      transform 0.28s var(--ease),
      height 0.28s var(--ease);
  }

  .body {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 12px 12px 12px 14px;
    transition: opacity 0.2s var(--ease);
  }

  /* Behind the front note only its edge shows; the text would bleed through */
  .behind .body {
    opacity: 0;
  }

  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    margin-top: 6px;
    border-radius: 50%;
    background: var(--ok);
  }

  .bad .dot {
    background: var(--bad);
  }

  p {
    flex: 1;
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
  }

  .undo {
    flex: none;
    height: 22px;
    margin: -1px 0;
    padding: 0 9px;
    border: 0;
    border-radius: 6px;
    background: rgb(var(--fg-rgb) / 0.07);
    color: var(--text);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: background-color 0.14s var(--ease);
  }

  .undo:hover {
    background: rgb(var(--fg-rgb) / 0.12);
  }

  .close {
    display: grid;
    place-items: center;
    flex: none;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--text-3);
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      color 0.14s var(--ease);
  }

  .close:hover {
    color: var(--text);
    background: rgb(var(--fg-rgb) / 0.06);
  }

  @media (prefers-reduced-motion: reduce) {
    .stack,
    .toast {
      transition: none;
    }
  }
</style>
