<!--
  "Find a map for this device": reads the identity registers the library's
  maps declare from the device of step 1, and lists the maps that fit.
-->
<script lang="ts">
  import { errorText } from "#lib/api.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { MapSearch } from "#lib/types.ts";

  let busy = $state(false);
  let result = $state.raw<MapSearch | null>(null);
  let error = $state<string | null>(null);

  async function find() {
    busy = true;
    error = null;
    result = null;
    try {
      result = await bench.findMaps();
    } catch (e) {
      error = errorText(e);
    } finally {
      busy = false;
    }
  }

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }
</script>

<section class="find">
  <div class="head">
    <div class="text">
      <strong>Find a map for this device</strong>
      <span class="dim"
        >Reads the model, firmware and reference registers that the maps declare
        from the device of step 1 ({bench.connectionText}, unit {bench.unitId}),
        each once, and lists the maps that fit. Reads only.</span
      >
    </div>
    <button
      class="btn"
      disabled={busy || bench.running}
      title={bench.running ? "Stop the test on the Main page first" : undefined}
      onclick={find}>{busy ? "Checking…" : "Find"}</button
    >
    {#if result || error}
      <button
        class="btn ghost"
        onclick={() => {
          result = null;
          error = null;
        }}>Close</button
      >
    {/if}
  </div>
  {#if error}
    <p class="bad" role="alert">{error}</p>
  {:else if result}
    <p class="dim">
      {result.candidates}
      {result.candidates === 1 ? "map declares" : "maps declare"} identity registers;
      {result.reads}
      {result.reads === 1 ? "read" : "reads"}.
    </p>
    {#if result.matches.length}
      <ul>
        {#each result.matches as m, i (m.path)}
          <li>
            <span class="rank">{i + 1}</span>
            <span class="name"
              ><strong>{m.vendor} {m.model}</strong>
              <span class="dim">{fileName(m.path)}</span></span
            >
            <span class="why"
              >{m.matched.join(" · ")}{#if m.warnings.length}<span class="warn">
                  · {m.warnings.join(" · ")}</span
                >{/if}</span
            >
            <button class="btn small" onclick={() => bench.openMap(m.path)}
              >Open</button
            >
          </li>
        {/each}
      </ul>
    {:else}
      <p class="empty">
        No map fits. Maps without model, firmware or reference registers cannot
        be checked; the online library may have one for this device.
      </p>
    {/if}
  {/if}
</section>

<style>
  .find {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dim {
    color: var(--text-3);
    font-size: 12px;
  }

  p {
    margin: 0;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: grid;
    grid-template-columns: 22px minmax(0, 1.2fr) minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 6px 0;
    border-top: 1px solid var(--border);
  }

  .rank {
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }

  .name {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .why {
    font-size: 12px;
    color: var(--text-2);
  }

  .warn {
    color: var(--bad);
  }

  .bad {
    color: var(--bad);
  }

  .empty {
    color: var(--text-2);
  }

  .btn.small {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
  }
</style>
