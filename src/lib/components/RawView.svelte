<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { clock, hex } from "#lib/format.ts";
  import { decodeFrame, type Framing } from "#lib/frames.ts";
  import { bench, type Exchange } from "#lib/state.svelte.ts";
  import FrameFields from "./FrameFields.svelte";
  import { showFrame } from "./tools/FrameDecoderTool.svelte";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";

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
    bench.transport === "tcp" || bench.transport === "udp"
      ? "tcp"
      : bench.transport === "rtu" && bench.framing === "ascii"
        ? "ascii"
        : "rtu",
  );

  function bytes(b: number[]): string {
    return b.map((x) => hex(x, 2)).join(" ");
  }

  /** A frame as the Frame decoder takes it: text for ASCII, else hex. */
  function frameText(b: number[]): string {
    return framing === "ascii" ? String.fromCharCode(...b).trim() : bytes(b);
  }

  function decodeInTool(b: number[], dir: "request" | "response") {
    showFrame(frameText(b), framing, dir);
    bench.tool = "frames";
    bench.page = "tools";
  }

  /** What a right-click on an exchange offers. */
  function exchangeMenu(x: Exchange): MenuEntry[] {
    return [
      { label: t("rawview.copy_data_out"), action: () => copy(bytes(x.out)) },
      x.in.length > 0 && {
        label: t("rawview.copy_data_in"),
        action: () => copy(bytes(x.in)),
      },
      {
        label: t("rawview.copy_both"),
        action: () =>
          copy(`TX ${bytes(x.out)}\nRX ${x.in.length ? bytes(x.in) : "—"}`),
      },
      null,
      {
        label: t("rawview.decode_here"),
        action: () => {
          decode = true;
          picked = x === last ? null : x;
        },
      },
      {
        label: t("rawview.open_request_in_frame"),
        action: () => decodeInTool(x.out, "request"),
      },
      x.in.length > 0 && {
        label: t("rawview.open_response_in_frame"),
        action: () => decodeInTool(x.in, "response"),
      },
      null,
      {
        label: t("chartview.clear"),
        danger: true,
        action: () => bench.clearRaw(),
      },
    ];
  }

  $effect(() => {
    void bench.exchanges.length;
    if (follow && list) list.scrollTop = list.scrollHeight;
  });
</script>

<div class="toolbar">
  <span class="hint">{t("rawview.bytes_exactly_as_sent")}</span>
  <span class="spacer"></span>
  <label class="check">
    <input type="checkbox" bind:checked={decode} />
    {t("rawview.decode_fields")}
  </label>
  <button
    class="btn"
    disabled={!bench.exchanges.length}
    onclick={() => bench.clearRaw()}>{t("chartview.clear")}</button
  >
</div>

<div class="latest">
  <div class="frame">
    <span class="tag out">{t("rawview.data_out")}</span>
    <code>{last ? bytes(last.out) : "—"}</code>
  </div>
  <div class="frame">
    <span class="tag in">{t("rawview.data_in")}</span>
    <code
      >{last
        ? last.in.length
          ? bytes(last.in)
          : t("rawview.no_response_yet")
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
      <span class="tag in">{t("rawview.data_in")}</span>
      {#if shown.in.length}
        <FrameFields
          compact
          decoded={decodeFrame(shown.in, framing, "response")}
        />
      {:else}
        <p class="empty">{t("rawview.no_response")}</p>
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
      title={decode ? t("rawview.decode_this_exchange") : undefined}
      onclick={() => (picked = x === last ? null : x)}
      oncontextmenu={(e) => contextMenu.show(e, exchangeMenu(x))}
    >
      <span class="time">{clock(x.time_ms)}</span>
      <span class="dir out">TX</span><code>{bytes(x.out)}</code>
      <span></span>
      <span class="dir in">RX</span><code
        >{x.in.length ? bytes(x.in) : "—"}</code
      >
    </div>
  {:else}
    <p class="empty">{t("rawview.nothing_sent_yet_press")}</p>
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
