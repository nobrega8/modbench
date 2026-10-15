<script lang="ts">
  import { functionName } from "#lib/frames.ts";
  import {
    SUGGEST_AFTER,
    TIMEOUT_FLOOR_MS,
    allTimes,
    average,
    ms,
    suggestedTimeout,
  } from "#lib/stats.ts";
  import { bench } from "#lib/state.svelte.ts";

  const b = $derived(bench.breakdown);
  const causes = $derived(
    [...b.causes].sort(([a, x], [c, y]) =>
      a === "OK" ? -1 : c === "OK" ? 1 : y - x,
    ),
  );
  const functions = $derived([...b.functions].sort(([a], [c]) => a - c));
  const times = $derived(allTimes(b));
  const suggested = $derived(suggestedTimeout(times));

  function share(n: number) {
    return b.total ? (n / b.total) * 100 : 0;
  }

  function pct(n: number) {
    const p = share(n);
    return p > 0 && p < 0.1 ? "<0.1%" : `${p.toFixed(1)}%`;
  }
</script>

<header class="tool-head">
  <h1>Statistics</h1>
  <p>
    Every request sent on the Main page since the last reset, by result and by
    function code. A pattern points to the cause: timeouts to wiring, baud rate
    or unit ID; exception 2 to an address the device does not have.
  </p>
</header>

<div class="row">
  <span><strong>{b.total}</strong> requests</span>
  <span class="spacer"></span>
  <button class="btn" disabled={!b.total} onclick={() => bench.resetBreakdown()}
    >Reset</button
  >
</div>

{#if !b.total}
  <p class="empty">Nothing yet. Press Start on the Main page.</p>
{:else}
  <section>
    <h2 class="key">By result</h2>
    <table class="data">
      <tbody>
        {#each causes as [name, n] (name)}
          <tr class:ok={name === "OK"}>
            <td class="name">{name}</td>
            <td class="meter">
              <span class="track"
                ><span class:fail={name !== "OK"} style:width="{share(n)}%"
                ></span></span
              >
            </td>
            <td class="num mono">{n}</td>
            <td class="num mono dim">{pct(n)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>

  <section>
    <h2 class="key">By function</h2>
    <table class="data">
      <thead>
        <tr>
          <th>Function</th>
          <th class="num">Requests</th>
          <th class="num">Failed</th>
          <th class="num">Min ms</th>
          <th class="num">Avg ms</th>
          <th class="num">Max ms</th>
        </tr>
      </thead>
      <tbody>
        {#each functions as [fc, f] (fc)}
          <tr>
            <td><span class="mono dim fc">{fc}</span>{functionName(fc)}</td>
            <td class="num mono">{f.count}</td>
            <td class="num mono" class:bad={f.failed > 0}>{f.failed}</td>
            {#if f.times.count}
              <td class="num mono">{ms(f.times.min)}</td>
              <td class="num mono">{ms(average(f.times))}</td>
              <td class="num mono">{ms(f.times.max)}</td>
            {:else}
              <td class="num dim" colspan="3">no answers</td>
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="dim note">
      Times count answers only (also exceptions): a timeout says nothing about
      how fast the device is.
    </p>
  </section>

  <section>
    <h2 class="key">Suggested timeout</h2>
    {#if suggested === null}
      <p class="dim note">
        Shown after {SUGGEST_AFTER} answers ({times.count} so far).
      </p>
    {:else}
      <div class="row suggest">
        <span class="mono"><strong>{suggested}</strong> ms</span>
        <span class="dim"
          >4 × the slowest answer ({ms(times.max)} ms), at least {TIMEOUT_FLOOR_MS}
          ms. Step 1 uses {bench.timeoutMs} ms.</span
        >
        <span class="spacer"></span>
        <button
          class="btn"
          disabled={bench.running || suggested === bench.timeoutMs}
          title={bench.running
            ? "Stop the test to change step 1."
            : "Put it in step 1's Timeout field."}
          onclick={() => (bench.timeoutMs = suggested)}>Use in step 1</button
        >
      </div>
    {/if}
  </section>
{/if}

<style>
  .spacer {
    flex: 1;
  }

  h2 {
    margin: 0 0 6px 10px;
  }

  .name {
    white-space: nowrap;
  }

  .meter {
    width: 40%;
  }

  .track {
    display: block;
    height: 8px;
    margin-top: 6px;
    border-radius: 4px;
    background: var(--subtle);
    overflow: hidden;
  }

  .track span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .track span.fail {
    background: rgb(var(--alert-rgb));
  }

  .fc {
    display: inline-block;
    width: 28px;
  }

  .suggest {
    margin-left: 10px;
    gap: 12px;
  }

  .note {
    margin: 6px 0 0 10px;
    font-size: 12px;
  }

  p {
    margin: 0;
  }
</style>
