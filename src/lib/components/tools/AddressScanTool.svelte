<script module lang="ts">
  import type { Area } from "#lib/types.ts";

  // Kept while other tools are shown.
  let area = $state<Area>("holding");
  let from = $state(0);
  let to = $state(999);
  let block = $state(100);
  let wait = $state(300);
  /** The span of the last scan, for the strip. */
  let scanned = $state<{ from: number; to: number } | null>(null);
</script>

<script lang="ts">
  import { AREA_NAME, AREA_TAG, formatAddress } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import AddressFormat from "../AddressFormat.svelte";
  import AddressInput from "../AddressInput.svelte";
  import IntInput from "../IntInput.svelte";
  import Select from "../Select.svelte";

  const areas: Area[] = ["holding", "input", "coil", "discrete"];
  const bits = $derived(area === "coil" || area === "discrete");
  const maxBlock = $derived(bits ? 2000 : 125);

  const busy = $derived(bench.scanning && bench.addressProgress !== null);
  const progress = $derived(
    bench.addressProgress
      ? bench.addressProgress.done / Math.max(1, bench.addressProgress.total)
      : 0,
  );
  const hits = $derived(bench.addressHits);
  const found = $derived(hits?.reduce((n, r) => n + r.count, 0) ?? 0);

  function addr(n: number) {
    return formatAddress(n, settings.addressFormat, bench.addressArea);
  }

  function scan() {
    scanned = { from, to };
    bench.scanAddresses({
      area,
      from,
      to,
      block: Math.min(block, maxBlock),
      timeout_ms: wait,
    });
  }

  /** Where a range sits on the strip, in percent. */
  function place(start: number, count: number) {
    const s = scanned ?? { from: 0, to: 1 };
    const span = s.to - s.from + 1;
    return {
      left: ((start - s.from) / span) * 100,
      width: Math.max(0.3, (count / span) * 100),
    };
  }
</script>

<header class="tool-head">
  <h1>Find addresses</h1>
  <p>
    Reads blocks of addresses and splits the ones that fail until it knows which
    addresses answer, so a device without a manual can still be mapped. Uses the
    connection of step 1 on the Main page. Reads only.
  </p>
</header>

<div class="row">
  <label class="field">
    <span>Area</span>
    <Select
      bind:value={area}
      label="Area"
      disabled={busy}
      options={areas.map((a) => ({ value: a, label: AREA_NAME[a] + "s" }))}
    />
  </label>
  <label class="field">
    <span>From</span>
    <AddressInput
      value={from}
      width="90px"
      label="First address"
      disabled={busy}
      onchange={(n, a) => {
        from = n;
        if (a) area = a;
      }}
    />
  </label>
  <label class="field">
    <span>To</span>
    <AddressInput
      value={to}
      width="90px"
      label="Last address"
      disabled={busy}
      onchange={(n, a) => {
        to = n;
        if (a) area = a;
      }}
    />
  </label>
  <label class="field" title="Addresses read at once before splitting">
    <span>Block</span>
    <IntInput
      value={Math.min(block, maxBlock)}
      min={1}
      max={maxBlock}
      width="72px"
      label="Block size"
      disabled={busy}
      onchange={(n) => (block = n)}
    />
  </label>
  <label class="field" title="How long to wait for each answer">
    <span>Wait (ms)</span>
    <IntInput
      value={wait}
      min={20}
      max={5000}
      width="80px"
      label="Wait per request"
      disabled={busy}
      onchange={(n) => (wait = n)}
    />
  </label>
  {#if busy}
    <button class="btn" onclick={() => bench.cancelScan()}>Stop</button>
  {:else}
    <button
      class="btn accent"
      disabled={bench.running || bench.scanning || from > to}
      title={bench.running ? "Stop the test on the Main page first" : undefined}
      onclick={scan}>Scan</button
    >
  {/if}
  <span class="spacer"></span>
  <AddressFormat />
</div>

{#if busy}
  <div
    class="bar"
    role="progressbar"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={Math.round(progress * 100)}
  >
    <div style:width="{progress * 100}%"></div>
  </div>
  <p class="dim mono">
    At {addr(bench.addressProgress!.address)} · {bench.addressProgress!.found}
    found so far
  </p>
{/if}

{#if bench.scanError && !busy}
  <p class="bad" role="alert">{bench.scanError}</p>
{:else if hits && scanned}
  <div class="strip" aria-hidden="true">
    {#each hits as r (r.start)}
      {@const p = place(r.start, r.count)}
      <span style:left="{p.left}%" style:width="{p.width}%"></span>
    {/each}
  </div>
  <div class="ends dim mono">
    <span>{addr(scanned.from)}</span><span>{addr(scanned.to)}</span>
  </div>
  {#if hits.length}
    <p>
      {found} of {scanned.to - scanned.from + 1} addresses answered, in {hits.length}
      {hits.length === 1 ? "range" : "ranges"}.
    </p>
    <table class="data">
      <thead>
        <tr>
          <th>From</th><th>To</th><th class="num">Count</th><th></th>
        </tr>
      </thead>
      <tbody>
        {#each hits as r (r.start)}
          <tr>
            <td class="mono"
              ><span class="tag">{AREA_TAG[bench.addressArea]}</span>{addr(
                r.start,
              )}</td
            >
            <td class="mono">{addr(r.start + r.count - 1)}</td>
            <td class="num mono">{r.count}</td>
            <td class="act">
              <button
                class="btn small"
                title="Read these addresses on the Main page"
                onclick={() =>
                  bench.readRange(bench.addressArea, r.start, r.count)}
                >Read</button
              >
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty">
      No address answered. Check the unit ID, or try another area or range.
    </p>
  {/if}
{/if}

<style>
  p {
    margin: 0;
  }

  .spacer {
    flex: 1;
  }

  .strip {
    position: relative;
    height: 14px;
    border-radius: 4px;
    background: var(--subtle);
    overflow: hidden;
  }

  .strip span {
    position: absolute;
    top: 0;
    bottom: 0;
    background: var(--accent);
  }

  .ends {
    display: flex;
    justify-content: space-between;
    margin-top: -10px;
    font-size: 11px;
  }

  .tag {
    margin-right: 6px;
    font-size: 10px;
    color: var(--text-3);
  }

  .act {
    text-align: right;
  }

  .btn.small {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
  }
</style>
