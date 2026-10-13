<script lang="ts">
  import { clock, hex } from "$lib/format";
  import { bench } from "$lib/state.svelte";

  let list = $state<HTMLElement>();
  let follow = $state(true);

  const last = $derived(bench.exchanges.at(-1) ?? null);

  function bytes(b: number[]): string {
    return b.map((x) => hex(x, 2)).join(" ");
  }

  $effect(() => {
    void bench.exchanges.length;
    if (follow && list) list.scrollTop = list.scrollHeight;
  });
</script>

<div class="toolbar">
  <span class="hint"
    >Bytes exactly as sent to and received from the device, newest last.</span
  >
  <span class="spacer"></span>
  <button
    class="btn"
    disabled={!bench.exchanges.length}
    onclick={() => bench.clearRaw()}>Clear</button
  >
</div>

<div class="latest">
  <div class="frame">
    <span class="tag out">Data out</span>
    <code>{last ? bytes(last.out) : "—"}</code>
  </div>
  <div class="frame">
    <span class="tag in">Data in</span>
    <code
      >{last
        ? last.in.length
          ? bytes(last.in)
          : "(no response yet)"
        : "—"}</code
    >
  </div>
</div>

<div
  class="history"
  bind:this={list}
  onscroll={() => {
    if (list)
      follow = list.scrollHeight - list.scrollTop - list.clientHeight < 24;
  }}
>
  {#each bench.exchanges as x, i (i)}
    <div class="ex">
      <span class="time">{clock(x.time_ms)}</span>
      <span class="dir out">TX</span><code>{bytes(x.out)}</code>
      <span></span>
      <span class="dir in">RX</span><code
        >{x.in.length ? bytes(x.in) : "—"}</code
      >
    </div>
  {:else}
    <p class="empty">Nothing sent yet. Press Start to send a request.</p>
  {/each}
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-bottom: 10px;
  }

  .hint {
    color: var(--text-2);
  }

  .spacer {
    flex: 1;
  }

  .latest {
    display: grid;
    gap: 6px;
    margin-bottom: 10px;
  }

  .frame {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--subtle);
  }

  .tag {
    flex: none;
    width: 64px;
    font-size: 12px;
    font-weight: 600;
  }

  .out {
    color: var(--accent);
  }

  .in {
    color: var(--ok);
  }

  code {
    font-family: var(--mono);
    font-size: 13px;
    word-break: break-all;
  }

  .history {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 4px 0;
  }

  .ex {
    display: grid;
    grid-template-columns: 96px 26px 1fr;
    column-gap: 8px;
    padding: 3px 10px;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }

  .time {
    font-family: var(--mono);
    color: var(--text-3);
  }

  .dir {
    font-weight: 600;
    font-size: 11px;
  }

  .empty {
    margin: 10px;
    color: var(--text-2);
  }
</style>
