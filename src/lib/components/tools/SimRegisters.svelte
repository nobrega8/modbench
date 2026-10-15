<!--
  The registers of the map the simulator was shaped after, with their values
  in engineering units. Click a value to change it; right-click a register
  to see what the map says about it and give it a generator.
-->
<script lang="ts">
  import { api, errorText } from "#lib/api.ts";
  import {
    AREA_TAG,
    formatAddress,
    formatValue,
    parseForRegister,
    valueTitle,
  } from "#lib/format.ts";
  import { registerOf } from "#lib/generators.ts";
  import { popIn, popOut } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { sim } from "#lib/simulator.svelte.ts";
  import type { Register, Value } from "#lib/types.ts";
  import Icon from "../Icon.svelte";
  import RegisterInfo from "./RegisterInfo.svelte";

  let {
    ongenerate,
  }: {
    /** Add a generator for a register, or edit the one at `index`. */
    ongenerate: (r: Register, index: number | null) => void;
  } = $props();

  let values = $state.raw<(Value | null)[]>([]);
  let search = $state("");
  let editing = $state<number | null>(null);
  let draft = $state("");
  let problem = $state<string | null>(null);
  let menu = $state<{ i: number; x: number; y: number } | null>(null);

  const registers = $derived(sim.map?.registers ?? []);

  // Read again when a client, a generator or this view changed something.
  $effect(() => {
    void sim.changes;
    api
      .simValues(sim.selected)
      .then((v) => (values = v))
      .catch(() => (values = []));
  });

  const shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return registers
      .map((r, i) => ({ r, i }))
      .filter(
        ({ r }) =>
          !q ||
          r.name.toLowerCase().includes(q) ||
          r.group.toLowerCase().includes(q) ||
          r.description.toLowerCase().includes(q),
      );
  });

  /** Index of the generator that drives each register, by register index. */
  const driving = $derived.by(() => {
    const out = new Map<number, number>();
    sim.generators.forEach((g, gi) => {
      const r = registerOf(g, registers);
      const i = r ? registers.indexOf(r) : -1;
      if (i >= 0 && !out.has(i)) out.set(i, gi);
    });
    return out;
  });

  async function set(r: Register, value: number) {
    problem = null;
    try {
      await api.simSetValue(sim.selected, r.name, value);
      sim.changes++;
    } catch (e) {
      problem = errorText(e);
    }
  }

  function click(i: number) {
    const r = registers[i];
    const v = values[i];
    if (r.type === "string") return;
    if (typeof v === "boolean") {
      set(r, v ? 0 : 1);
      return;
    }
    editing = i;
    draft = v === null || v === undefined ? "" : formatValue(r, v);
  }

  function commit() {
    if (editing === null) return;
    const r = registers[editing];
    const parsed = parseForRegister(r, draft);
    if ("error" in parsed) {
      problem = parsed.error;
      return;
    }
    set(r, Number(parsed.value));
    editing = null;
  }

  function openMenu(e: MouseEvent, i: number) {
    e.preventDefault();
    menu = { i, x: e.clientX, y: e.clientY };
  }

  function generate(i: number) {
    menu = null;
    ongenerate(registers[i], driving.get(i) ?? null);
  }

  /** Keeps the menu inside the window, and closes it on a click outside. */
  function placed(node: HTMLElement) {
    const r = node.getBoundingClientRect();
    const x = Math.min(menu?.x ?? 0, innerWidth - r.width - 8);
    const y = Math.min(menu?.y ?? 0, innerHeight - r.height - 8);
    node.style.left = `${Math.max(8, x)}px`;
    node.style.top = `${Math.max(8, y)}px`;
    node.querySelector<HTMLElement>("button")?.focus();
    const away = (e: Event) => {
      if (!node.contains(e.target as Node)) menu = null;
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") menu = null;
    };
    const close = () => (menu = null);
    addEventListener("pointerdown", away, true);
    addEventListener("keydown", key);
    addEventListener("resize", close);
    addEventListener("wheel", close, { passive: true });
    return {
      destroy() {
        removeEventListener("pointerdown", away, true);
        removeEventListener("keydown", key);
        removeEventListener("resize", close);
        removeEventListener("wheel", close);
      },
    };
  }

  function focus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<div class="row">
  <input
    class="input search"
    type="search"
    placeholder="Search registers"
    aria-label="Search registers"
    bind:value={search}
  />
  <span class="dim hint"
    >Click a value to change it. Right-click a register for its details and a
    generator.</span
  >
</div>
{#if problem}<p class="bad">{problem}</p>{/if}

<div class="list">
  <table class="data regs">
    <thead>
      <tr>
        <th></th><th>Address</th><th>Register</th><th>Type</th><th class="num"
          >Value</th
        ><th></th>
      </tr>
    </thead>
    <tbody>
      {#each shown as { r, i } (i)}
        {@const v = values[i] ?? null}
        {@const gen = driving.get(i)}
        <tr
          class:menu-open={menu?.i === i}
          oncontextmenu={(e) => openMenu(e, i)}
        >
          <td class="tag mono">{AREA_TAG[r.area]}</td>
          <td class="mono"
            >{formatAddress(r.address, settings.addressFormat)}</td
          >
          <td>
            <span class="name">{r.name}</span>
            {#if r.group}<span class="dim">{r.group}</span>{/if}
          </td>
          <td class="mono dim">{r.type}</td>
          <td class="num value">
            {#if editing === i}
              <input
                class="input num"
                bind:value={draft}
                use:focus
                onkeydown={(e) => {
                  if (e.key === "Enter") commit();
                  if (e.key === "Escape") editing = null;
                }}
                onblur={commit}
                aria-label="Value of {r.name}"
              />
            {:else}
              <button
                class="val mono"
                class:gen={gen !== undefined}
                disabled={r.type === "string"}
                title={valueTitle(r, v)}
                onclick={() => click(i)}
                >{formatValue(r, v)}{#if r.unit && typeof v === "number"}<span
                    class="unit">{r.unit}</span
                  >{/if}</button
              >
            {/if}
          </td>
          <td class="act">
            <button
              class="btn ghost small"
              title="Details and generator"
              aria-label="Details of {r.name}"
              onclick={(e) => openMenu(e, i)}><Icon name="more" /></button
            >
          </td>
        </tr>
      {:else}
        <tr><td colspan="6" class="empty">No register matches.</td></tr>
      {/each}
    </tbody>
  </table>
</div>

{#if menu}
  {@const r = registers[menu.i]}
  {@const gen = driving.get(menu.i)}
  <div
    class="menu"
    role="menu"
    aria-label={r.name}
    use:placed
    in:popIn
    out:popOut
  >
    <RegisterInfo register={r} />
    <div class="actions">
      {#if r.type !== "string"}
        <button
          class="btn accent small"
          role="menuitem"
          onclick={() => generate(menu!.i)}
          >{gen === undefined ? "Add generator…" : "Edit generator…"}</button
        >
      {/if}
      {#if r.type !== "string" && values[menu.i] !== null}
        <button
          class="btn small"
          role="menuitem"
          onclick={() => {
            const i = menu!.i;
            menu = null;
            click(i);
          }}>Set value…</button
        >
      {/if}
    </div>
  </div>
{/if}

<style>
  p {
    margin: 0;
  }

  .row {
    align-items: center;
  }

  .search {
    width: 240px;
  }

  .hint {
    font-size: 12px;
  }

  .list {
    max-height: 420px;
    overflow: auto;
  }

  .regs thead th {
    position: sticky;
    top: 0;
    background: var(--card);
    z-index: 1;
  }

  .regs td {
    vertical-align: middle;
  }

  .regs tbody tr:hover td,
  .regs tr.menu-open td {
    background: var(--accent-soft);
  }

  .tag {
    width: 1px;
    color: var(--text-3);
    font-size: 12px;
  }

  .name {
    font-weight: 600;
    margin-right: 6px;
  }

  .dim {
    color: var(--text-3);
  }

  .value {
    width: 1px;
    white-space: nowrap;
  }

  .value input {
    width: 140px;
    height: 26px;
  }

  .val {
    min-width: 90px;
    height: 26px;
    padding: 0 8px;
    border: 0;
    border-radius: 4px;
    background: var(--cell);
    color: var(--text);
    font-size: 13px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    text-align: right;
    cursor: pointer;
  }

  .val:hover:not(:disabled) {
    background: var(--hover);
  }

  .val:disabled {
    cursor: default;
    font-weight: 500;
  }

  /* Set by a generator: the device's own green, under the value */
  .val.gen {
    box-shadow: inset 0 -2px 0 var(--rx);
  }

  .unit {
    margin-left: 4px;
    color: var(--text-3);
    font-weight: 500;
  }

  .act {
    width: 1px;
  }

  .small {
    height: 26px;
  }

  .menu {
    position: fixed;
    z-index: 50;
    width: min(380px, calc(100vw - 16px));
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    border-radius: 12px;
    background: var(--card);
    box-shadow: var(--e3);
  }

  .actions {
    display: flex;
    gap: 8px;
  }
</style>
