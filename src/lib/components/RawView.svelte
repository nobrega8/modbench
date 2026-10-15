<script lang="ts">
  import { clock, hex } from "#lib/format.ts";
  import { decodeFrame, type Framing } from "#lib/frames.ts";
  import { bench, type Exchange } from "#lib/state.svelte.ts";
  import FrameFields from "./FrameFields.svelte";

  let list = $state<HTMLElement>();
  let follow = $state(true);
  let decode = $state(false);
  /** The exchange picked in the history to decode; null follows the newest. */
  let picked = $state<Exchange | null>(null);

  const last = $derived(bench.exchanges.at(-1) ?? null);
  const shown = $derived(
    picked && bench.exchanges.includes(picked) ? picked : last,
  );
  const framing = $derived<Framing>(
    bench.transport === "tcp"
      ? "tcp"
      : bench.transport === "rtu" && bench.framing === "ascii"
        ? "ascii"
        : "rtu",
  );

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
  <label class="check">
    <input type="checkbox" bind:checked={decode} />
    Decode fields
  </label>
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

{#if decode && shown}
  <div class="decoded">
    <div>
      <span class="tag out"
        >Data out{#if picked && picked !== last}
          · {clock(picked.time_ms)}{/if}</span
      >
      <FrameFields
        compact
        decoded={decodeFrame(shown.out, framing, "request")}
      />
    </div>
    <div>
      <span class="tag in">Data in</span>
      {#if shown.in.length}
        <FrameFields
          compact
          decoded={decodeFrame(shown.in, framing, "response")}
        />
      {:else}
        <p class="empty">No response.</p>
      {/if}
    </div>
  </div>
{/if}

<div
  class="history"
  bind:this={list}
  onscroll={() => {
    if (list)
      follow = list.scrollHeight - list.scrollTop - list.clientHeight < 24;
  }}
>
  {#each bench.exchanges as x, i (i)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="ex"
      class:picked={decode && shown === x}
      role="button"
      tabindex="-1"
      title={decode ? "Decode this exchange" : undefined}
      onclick={() => (picked = x === last ? null : x)}
    >
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
    border-radius: 10px;
    background: var(--subtle);
  }

  .tag {
    flex: none;
    width: 64px;
    font-size: 12px;
    font-weight: 600;
  }

  .out {
    color: var(--tx);
  }

  .in {
    color: var(--rx);
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
    border-radius: 10px;
    background: var(--subtle);
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

  .decoded {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-bottom: 10px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--subtle);
    max-height: 40%;
    overflow: auto;
  }

  .decoded .tag {
    display: block;
    margin-bottom: 6px;
  }

  .ex.picked {
    background: rgb(var(--fg-rgb) / 0.07);
  }

  .empty {
    margin: 10px;
    color: var(--text-2);
  }
</style>
