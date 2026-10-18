<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { hex, parseInteger } from "#lib/format.ts";
  import { bench } from "#lib/state.svelte.ts";

  const PER_ROW = 10;
  const bits = $derived(bench.fc === 5 || bench.fc === 15);
  const isWrite = $derived(!bench.isRead);
  let bad = $state<Set<number>>(new Set());

  function setWord(i: number, text: string) {
    const n = parseInteger(text);
    const next = new Set(bad);
    if (n === null || n < -32768 || n > 65535) {
      next.add(i);
    } else {
      next.delete(i);
      bench.writeValues[i] = n;
    }
    bad = next;
  }

  const rows = $derived(
    Array.from(
      { length: Math.ceil(bench.writeValues.length / PER_ROW) },
      (_, r) =>
        bench.writeValues
          .slice(r * PER_ROW, (r + 1) * PER_ROW)
          .map((v, k) => ({ i: r * PER_ROW + k, v })),
    ),
  );
</script>

{#if !isWrite}
  <p class="note">{t("writegrid.choose_a_write_command")}</p>
{:else}
  <p class="note">
    {bits
      ? "Tick the coils to switch on."
      : "Values are sent as 16-bit words: decimal, negative, 0x… or …h."}
    Press Start to write {bench.writeValues.length}
    {bits ? "coil" : "register"}{bench.writeValues.length === 1 ? "" : "s"} from address
    {bench.address}.
  </p>
{/if}

<div class="grid-wrap">
  <div class="grid">
    <span></span>
    {#each Array(PER_ROW) as _, i (i)}<span class="colhead">+{i}</span>{/each}
    {#each rows as row, r (r)}
      <span class="rowhead"
        >{String(bench.address + r * PER_ROW).padStart(5, "0")}</span
      >
      {#each row as { i, v } (i)}
        {#if bits}
          <label class="cell bit" title="Address {bench.address + i}">
            <input
              type="checkbox"
              checked={v !== 0}
              disabled={bench.running}
              onchange={(e) =>
                (bench.writeValues[i] = e.currentTarget.checked ? 1 : 0)}
            />
            {v !== 0 ? "1" : "0"}
          </label>
        {:else}
          <input
            class="cell"
            class:invalid={bad.has(i)}
            value={String(v)}
            disabled={bench.running}
            spellcheck="false"
            aria-label="Value for address {bench.address + i}"
            title="Address {bench.address + i} · {hex(v & 0xffff)}h"
            onchange={(e) => setWord(i, e.currentTarget.value)}
          />
        {/if}
      {/each}
      {#if row.length < PER_ROW}<span
          style:grid-column="span {PER_ROW - row.length}"
        ></span>{/if}
    {/each}
  </div>
</div>

<style>
  .note {
    margin: 0 0 10px;
    color: var(--text-2);
  }

  .grid-wrap {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }

  .grid {
    display: grid;
    grid-template-columns: 58px repeat(10, minmax(64px, 1fr));
    gap: 3px;
  }

  .colhead {
    font-size: 11px;
    color: var(--text-3);
    text-align: center;
  }

  .rowhead {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 6px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
  }

  .cell {
    min-width: 0;
    height: 28px;
    padding: 0 6px;
    border: 0;
    border-radius: 3px;
    background: var(--changed);
    font-family: var(--mono);
    font-size: 13px;
    font-weight: 600;
  }

  .cell:focus {
    outline: none;
    box-shadow: var(--ring-focus);
  }

  .cell.invalid {
    box-shadow: 0 0 0 2px rgb(var(--alert-rgb) / 0.55);
  }

  .bit {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .bit input {
    accent-color: rgb(var(--fg-rgb));
  }
</style>
