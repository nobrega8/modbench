<script module lang="ts">
  import type { Area } from "$lib/types";

  // Kept while other tools are shown.
  let area = $state<Area>("holding");
  let start = $state(0);
</script>

<script lang="ts">
  import { api, errorText } from "$lib/api";
  import { clock, formatAddress, hex, parseInteger } from "$lib/format";
  import { functionName } from "$lib/frames";
  import { slidePill } from "$lib/motion";
  import { settings } from "$lib/settings.svelte";
  import { sim } from "$lib/simulator.svelte";
  import { bench } from "$lib/state.svelte";
  import type { SimView } from "$lib/types";
  import AddressFormat from "../AddressFormat.svelte";
  import IntInput from "../IntInput.svelte";

  const PER_ROW = 10;
  const SHOWN = 100;
  const areas: { id: Area; label: string }[] = [
    { id: "holding", label: "Holding registers" },
    { id: "input", label: "Input registers" },
    { id: "coil", label: "Coils" },
    { id: "discrete", label: "Discrete inputs" },
  ];

  let view = $state.raw<SimView | null>(null);
  let editing = $state<number | null>(null);
  let draft = $state("");
  let problem = $state<string | null>(null);

  const bits = $derived(area === "coil" || area === "discrete");
  const first = $derived(Math.min(start, 0x10000 - SHOWN));
  const recent = $derived(sim.events.slice(-120).reverse());

  // Read again when the page changes or a client wrote something.
  $effect(() => {
    void sim.changes;
    const [a, s] = [area, first];
    api
      .simRead(a, s, SHOWN)
      .then((v) => (view = v))
      .catch(() => (view = null));
  });

  function addr(n: number) {
    return formatAddress(n, settings.addressFormat, true);
  }

  async function write(offset: number, value: number | boolean) {
    problem = null;
    try {
      await api.simWrite(
        area,
        first + offset,
        typeof value === "boolean"
          ? { kind: "bits", values: [value] }
          : { kind: "registers", values: [value & 0xffff] },
      );
      sim.changes++;
    } catch (e) {
      problem = errorText(e);
    }
  }

  function edit(offset: number) {
    if (!view) return;
    if (view.data.kind === "bits") {
      write(offset, !view.data.values[offset]);
      return;
    }
    editing = offset;
    draft = String(view.data.values[offset]);
  }

  function commit() {
    if (editing === null) return;
    const n = parseInteger(draft);
    if (n === null || n < -32768 || n > 65535) {
      problem = "Type a whole number from −32768 to 65535, or hex like 0x1F.";
      return;
    }
    write(editing, n);
    editing = null;
  }

  function useInMain() {
    bench.transport = "tcp";
    bench.host = "127.0.0.1";
    bench.tcpPort = sim.running ?? sim.port;
    if (sim.unit !== null) bench.unitId = sim.unit;
    bench.page = "main";
  }

  function where(e: { address: number | null; count: number | null }) {
    if (e.address === null) return "";
    return e.count && e.count > 1
      ? `${addr(e.address)} × ${e.count}`
      : addr(e.address);
  }

  function focus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<header class="tool-head">
  <h1>Simulator</h1>
  <p>
    A Modbus TCP device that runs inside Modbench. Shape it after a map to try
    the map, the alarms or another program without hardware: it answers with the
    map's reference values, and with exception 2 at addresses the map does not
    have.
  </p>
</header>

<div class="row">
  <label class="field">
    <span>Port</span>
    <IntInput
      value={sim.port}
      min={1}
      max={65535}
      width="80px"
      label="Port"
      disabled={sim.running !== null}
      onchange={(n) => (sim.port = n)}
    />
  </label>
  <label class="field" title="Leave empty to answer every unit ID">
    <span>Unit ID</span>
    <input
      class="input num"
      style:width="80px"
      value={sim.unit ?? ""}
      placeholder="All"
      disabled={sim.running !== null}
      aria-label="Unit ID"
      onchange={(e) => {
        const n = parseInteger(e.currentTarget.value);
        sim.unit = n !== null && n >= 0 && n <= 255 ? n : null;
        e.currentTarget.value = sim.unit === null ? "" : String(sim.unit);
      }}
    />
  </label>
  <label
    class="check switch-row"
    title="Let other computers on the network connect (the firewall may ask)"
  >
    <input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={sim.isPublic}
      disabled={sim.running !== null}
    />
    Other computers
  </label>
  {#if sim.running === null}
    <button class="btn accent" onclick={() => sim.start()}>Start</button>
  {:else}
    <button class="btn danger" onclick={() => sim.stop()}>Stop</button>
  {/if}
  <span class="status" aria-live="polite">
    {#if sim.running !== null}
      <span class="live"
        ><span class="dot"></span>Listening on {sim.isPublic
          ? "every interface"
          : "127.0.0.1"}:{sim.running}</span
      >
      <button class="btn ghost" onclick={useInMain}>Connect Main to it</button>
    {:else}
      <span class="dim">Stopped</span>
    {/if}
  </span>
</div>

<div class="row source">
  <span
    >Values: <strong
      >{sim.loaded ? `shaped after ${sim.loaded}` : "an empty device"}</strong
    ></span
  >
  <button
    class="btn"
    disabled={!bench.map}
    title={bench.map ? undefined : "Open a map on the Main page first"}
    onclick={() =>
      bench.map &&
      sim.load(
        $state.snapshot(bench.map.map),
        bench.map.path.split(/[\\/]/).pop() ?? "the map",
      )}
    >{bench.map
      ? `Load ${bench.map.map.meta.vendor} ${bench.map.map.meta.model}`
      : "Load the open map"}</button
  >
  <label
    class="check"
    title="Only the map's addresses exist; the others answer exception 2"
  >
    <input type="checkbox" bind:checked={sim.strict} />
    Only the map's addresses
  </label>
  <button class="btn ghost" onclick={() => sim.clear()}>Clear</button>
</div>

{#if sim.error}<p class="bad" role="alert">{sim.error}</p>{/if}

<div class="row">
  <div class="segmented" role="tablist" aria-label="Area" use:slidePill>
    {#each areas as a (a.id)}
      <button
        role="tab"
        aria-selected={area === a.id}
        class:on={area === a.id}
        onclick={() => {
          area = a.id;
          editing = null;
        }}>{a.label}</button
      >
    {/each}
  </div>
  <label class="inline">
    <span class="dim">From</span>
    <IntInput
      value={first}
      min={0}
      max={0x10000 - SHOWN}
      width="80px"
      label="First address shown"
      onchange={(n) => {
        start = n;
        editing = null;
      }}
    />
  </label>
  <span class="spacer"></span>
  <AddressFormat />
</div>
<p class="dim hint">
  {bits
    ? "Click a bit to switch it."
    : "Click a value to change it. Clients see the change at once."}
</p>
{#if problem}<p class="bad">{problem}</p>{/if}

{#if view}
  <div class="grid" style:--cols={PER_ROW}>
    <span></span>
    {#each Array(PER_ROW) as _, i (i)}<span class="colhead">+{i}</span>{/each}
    {#each Array(SHOWN / PER_ROW) as _, row (row)}
      <span class="rowhead mono">{addr(first + row * PER_ROW)}</span>
      {#each Array(PER_ROW) as _, col (col)}
        {@const i = row * PER_ROW + col}
        {@const v = view.data.values[i]}
        {@const defined = view.defined[i]}
        {#if editing === i}
          <input
            class="cell input num"
            bind:value={draft}
            use:focus
            onkeydown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") editing = null;
            }}
            onblur={commit}
            aria-label="Value at {addr(first + i)}"
          />
        {:else}
          <button
            class="cell mono"
            class:off={!defined}
            class:on={v === true}
            title="{addr(first + i)}{defined
              ? ''
              : ' · not in the map'}{typeof v === 'number'
              ? ` · 0x${hex(v)}`
              : ''}"
            onclick={() => edit(i)}
            >{typeof v === "boolean" ? (v ? "1" : "0") : v}</button
          >
        {/if}
      {/each}
    {/each}
  </div>
{/if}

<section class="activity">
  <div class="row">
    <h2 class="key">Activity</h2>
    <span class="spacer"></span>
    <button
      class="btn ghost"
      disabled={!sim.events.length}
      onclick={() => sim.clearEvents()}>Clear</button
    >
  </div>
  {#if recent.length}
    <table class="data">
      <thead>
        <tr>
          <th>Time</th><th>Client</th><th class="num">Unit</th><th>Request</th
          ><th>Address</th><th>Result</th>
        </tr>
      </thead>
      <tbody>
        {#each recent as e, i (sim.events.length - i)}
          <tr>
            <td class="mono dim">{clock(e.time_ms)}</td>
            <td class="mono">{e.peer}</td>
            <td class="num mono">{e.unit}</td>
            <td>{functionName(e.function)}</td>
            <td class="mono">{where(e)}</td>
            <td class:bad={!!e.error} class:wrote={e.wrote}
              >{e.error ?? (e.wrote ? "written" : "OK")}</td
            >
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty">
      {sim.running === null
        ? "Start the simulator, then connect to it from the Main page or another program."
        : "Waiting for a client…"}
    </p>
  {/if}
</section>

<style>
  p {
    margin: 0;
  }

  .spacer {
    flex: 1;
  }

  .switch-row {
    height: 30px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 30px;
  }

  .live {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-weight: 600;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text);
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }

  .source {
    align-items: center;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--subtle);
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .hint {
    margin-top: -6px;
    font-size: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: 58px repeat(var(--cols), minmax(56px, 1fr));
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
    font-size: 12px;
    color: var(--text-2);
  }

  .cell {
    height: 26px;
    padding: 0 6px;
    border: 0;
    border-radius: 3px;
    background: var(--cell);
    font-size: 13px;
    font-weight: 600;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  button.cell:hover {
    background: var(--hover);
  }

  .cell.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  .cell.off {
    opacity: 0.35;
  }

  input.cell {
    min-width: 0;
    width: 100%;
    border-radius: 3px;
  }

  .activity h2 {
    margin: 0;
  }

  .activity .row {
    align-items: center;
    margin-bottom: 4px;
  }

  td.wrote {
    font-weight: 600;
  }
</style>
