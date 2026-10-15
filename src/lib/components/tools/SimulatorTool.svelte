<script module lang="ts">
  import type { Area } from "#lib/types.ts";

  // Kept while other tools are shown.
  let area = $state<Area>("holding");
  let start = $state(0);
  /** With a map loaded: its registers as a list, or the raw grid. */
  let mode = $state<"registers" | "grid">("registers");
</script>

<script lang="ts">
  import { api, errorText } from "#lib/api.ts";
  import {
    areaOfFunction,
    clock,
    formatAddress,
    hex,
    parseInteger,
  } from "#lib/format.ts";
  import { functionName } from "#lib/frames.ts";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";
  import { span } from "#lib/generators.ts";
  import { slidePill } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import {
    IN_SIM_WINDOW,
    deviceName,
    overNetwork,
    sim,
    whereText,
    type SimKind,
  } from "#lib/simulator.svelte.ts";
  import Icon from "../Icon.svelte";
  import { bench } from "#lib/state.svelte.ts";
  import { MAP_EXTENSIONS, type SimEvent, type SimView } from "#lib/types.ts";
  import { open as openDialog, save } from "@tauri-apps/plugin-dialog";
  import { SETUP_EXTENSION } from "#lib/sim-setup.ts";
  import AddressFormat from "../AddressFormat.svelte";
  import AddressInput from "../AddressInput.svelte";
  import IntInput from "../IntInput.svelte";
  import Select from "../Select.svelte";
  import SimGenerators from "./SimGenerators.svelte";
  import SimRegisters from "./SimRegisters.svelte";

  let generators: SimGenerators | undefined = $state();

  const kinds: { value: SimKind; label: string }[] = [
    { value: "tcp", label: "Modbus TCP" },
    { value: "rtuovertcp", label: "RTU over TCP" },
    { value: "udp", label: "Modbus UDP" },
    { value: "rtuoverudp", label: "RTU over UDP" },
    { value: "rtu", label: "Serial RTU" },
    { value: "ascii", label: "Serial ASCII" },
  ];
  const baudrates = [1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];
  const serial = $derived(sim.kind === "rtu" || sim.kind === "ascii");
  const stopped = $derived(sim.running === null);
  let ports = $state<string[]>([]);

  async function refreshPorts() {
    try {
      ports = await api.listSerialPorts();
      if (!ports.includes(sim.serialPort)) sim.serialPort = ports[0] ?? "";
    } catch {
      ports = [];
    }
  }

  $effect(() => {
    if (serial && stopped) refreshPorts();
  });

  /** A device's tab: its unit ID and what it is shaped after. */
  function deviceLabel(d: (typeof sim.devices)[number]) {
    const unit = d.unit === null ? "All units" : `Unit ${d.unit}`;
    return `${unit} · ${deviceName(d) ?? "empty"}`;
  }

  /** Maps to shape the device after: those open on Main, then the library. */
  let picks = $state(0);
  /** Picked in the Device list to open a map file instead. */
  const OPEN_FILE = "__open_file__";

  /** Maps to shape the device after: those open on Main, then the library,
   * then any file. The one loaded now is always listed. */
  const sources = $derived.by(() => {
    const list = [
      { value: "", label: "Empty device" },
      ...bench.maps.map((m) => ({
        value: m.path,
        label: `${m.map.meta.vendor} ${m.map.meta.model} · open`,
      })),
      ...bench.library
        .filter((e) => !e.error && !bench.maps.some((m) => m.path === e.path))
        .map((e) => ({ value: e.path, label: `${e.vendor} ${e.model}` })),
    ];
    if (sim.source && !list.some((o) => o.value === sim.source))
      list.push({ value: sim.source, label: sim.loaded ?? sim.source });
    list.push({ value: OPEN_FILE, label: "Open a map file…" });
    return list;
  });

  async function choose(path: string) {
    if (path === OPEN_FILE) {
      // Show the loaded map again, also when the dialog is cancelled.
      picks++;
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Device map", extensions: MAP_EXTENSIONS }],
      });
      if (typeof picked === "string") await sim.pick(picked);
      return;
    }
    const open = bench.maps.find((m) => m.path === path);
    sim.pick(path || null, open && $state.snapshot(open.map));
  }

  // The library lists maps to load; in the simulator's own window nothing
  // else has read it yet.
  $effect(() => {
    if (!bench.library.length) bench.refreshLibrary().catch(() => {});
  });

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
  const SHOWN_EVENTS = 120;
  const reads = $derived(sim.reads.slice(-SHOWN_EVENTS).reverse());
  const writes = $derived(sim.writes.slice(-SHOWN_EVENTS).reverse());

  // Read again when the page or the device changes, or a client wrote
  // something.
  $effect(() => {
    void sim.changes;
    const [d, a, s] = [sim.selected, area, first];
    api
      .simRead(d, a, s, SHOWN)
      .then((v) => (view = v))
      .catch(() => (view = null));
  });

  // While generators run, read the memory a few times a second.
  $effect(() => {
    if (sim.running === null || !sim.generators.length) return;
    const timer = setInterval(() => sim.changes++, 250);
    return () => clearInterval(timer);
  });

  /** Addresses of this area a generator writes to. */
  const driven = $derived.by(() => {
    const out = new Set<number>();
    for (const g of sim.generators) {
      const s = span(g, sim.registers);
      if (s?.area !== area) continue;
      for (let i = 0; i < s.count; i++) out.add(s.address + i);
    }
    return out;
  });

  function addr(n: number, at: Area | null = area) {
    return formatAddress(n, settings.addressFormat, at, true);
  }

  async function write(offset: number, value: number | boolean) {
    problem = null;
    try {
      await api.simWrite(
        sim.selected,
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

  /** What a right-click on a value in the grid offers. */
  function cellMenu(offset: number): MenuEntry[] {
    if (!view) return [];
    const v = view.data.values[offset];
    const text = typeof v === "boolean" ? (v ? "1" : "0") : String(v);
    return [
      typeof v === "boolean"
        ? { label: v ? "Switch off" : "Switch on", action: () => edit(offset) }
        : { label: "Change value…", action: () => edit(offset) },
      typeof v === "number" && {
        label: "Set to 0",
        disabled: v === 0,
        action: () => write(offset, 0),
      },
      null,
      { label: "Copy value", action: () => copy(text) },
      typeof v === "number" && {
        label: "Copy value",
        hint: "hex",
        action: () => copy(`0x${hex(v)}`),
      },
      { label: "Copy address", action: () => copy(addr(first + offset)) },
    ];
  }

  /** One request of the activity log as a line of text. */
  function eventLine(e: SimEvent): string {
    const result = e.error ?? (e.wrote ? "written" : "OK");
    return `${clock(e.time_ms)}\t${e.peer}\tunit ${e.unit}\t${functionName(e.function)}\t${where(e)}\t${result}`;
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

  const setupFilter = [
    { name: "Simulator setup", extensions: [SETUP_EXTENSION] },
  ];
  async function saveSetup() {
    sim.setupNote = null;
    const path = await save({
      defaultPath: `simulator.${SETUP_EXTENSION}`,
      filters: setupFilter,
    });
    if (!path) return;
    try {
      await api.saveText(path, sim.setupText());
      sim.setupNote = { ok: true, text: "Setup saved." };
    } catch (e) {
      sim.setupNote = { ok: false, text: `Not saved: ${errorText(e)}` };
    }
  }

  async function openSetup() {
    const path = await openDialog({ multiple: false, filters: setupFilter });
    if (typeof path === "string") await sim.openSetupFile(path);
  }

  /** Points step 1 of Main at the simulator, and its unit ID at the
   * device shown. Only over the network: a serial port has another end. */
  function useInMain() {
    const t = sim.running;
    if (!t || !overNetwork(t)) return;
    bench.transport = t.kind;
    bench.host = "127.0.0.1";
    bench.tcpPort = t.port;
    if (sim.unit !== null) bench.unitId = sim.unit;
    bench.page = "main";
  }

  function where(e: SimEvent) {
    if (e.address === null) return "";
    const at = areaOfFunction(e.function);
    return e.count && e.count > 1
      ? `${addr(e.address, at)} × ${e.count}`
      : addr(e.address, at);
  }

  function focus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

{#if sim.windowOpen && !IN_SIM_WINDOW}
  <header class="tool-head">
    <h1>Simulator</h1>
    <p>
      The simulator is open in its own window{sim.running !== null
        ? `, listening on ${whereText(sim.running)}`
        : ""}.
    </p>
  </header>
  <div class="row">
    <button class="btn accent" onclick={() => api.simOpenWindow()}
      >Show its window</button
    >
    <button class="btn" onclick={() => api.simCloseWindow()}
      >Bring it back here</button
    >
  </div>
{:else}
  <header class="tool-head">
    <div class="title-row">
      <h1>Simulator</h1>
      <span class="spacer"></span>
      {#if sim.setupNote}
        <span
          class="setup-note"
          class:bad={!sim.setupNote.ok}
          role={sim.setupNote.ok ? undefined : "alert"}
          >{sim.setupNote.text}</span
        >
      {/if}
      <button
        class="btn ghost"
        title="Load the interface, devices, unit IDs, maps and generators saved with Save setup…"
        onclick={openSetup}>Open setup…</button
      >
      <button
        class="btn ghost"
        title="Save the interface, devices, unit IDs, maps and generators to a file"
        onclick={saveSetup}>Save setup…</button
      >
      {#if IN_SIM_WINDOW}
        <button
          class="btn ghost"
          title="Close this window and show the simulator on the Tools page again"
          onclick={() => api.simCloseWindow()}>Back to the main window</button
        >
      {:else}
        <button
          class="btn ghost"
          title="Open the simulator in a window of its own, to see it next to the Main page"
          onclick={() => api.simOpenWindow()}
          ><Icon name="popout" />Pop out</button
        >
      {/if}
    </div>
    <p>
      Modbus devices that run inside Modbench, over TCP or UDP, RTU over TCP or
      UDP, or a serial port. Shape each after a map to try the map, the alarms
      or another program without hardware: it answers with the map's reference
      values, and with exception 2 at addresses the map does not have. Several
      devices answer their own unit IDs at the same address, like devices behind
      a gateway or on one RS-485 line; other unit IDs get no answer. Generators
      make values move on their own.
    </p>
  </header>

  <div class="row">
    <label class="field">
      <span>Interface</span>
      <Select
        bind:value={sim.kind}
        options={kinds}
        label="How clients reach the simulator"
        disabled={!stopped}
      />
    </label>
    {#if serial}
      <label class="field">
        <span>Serial port</span>
        <span class="with-btn">
          <Select
            bind:value={sim.serialPort}
            disabled={!stopped}
            label="Serial port"
            placeholder="No ports found"
            options={ports.map((p) => ({ value: p, label: p }))}
          />
          <button
            class="btn icon"
            title="Search for serial ports again"
            aria-label="Search for serial ports again"
            disabled={!stopped}
            onclick={refreshPorts}
          >
            <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
              ><path
                d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9M13.5 2.5v3h-3"
                fill="none"
                stroke="currentColor"
                stroke-width="1.4"
                stroke-linecap="round"
                stroke-linejoin="round"
              /></svg
            >
          </button>
        </span>
      </label>
      <label class="field">
        <span>Baud rate</span>
        <Select
          bind:value={sim.baudrate}
          disabled={!stopped}
          label="Baud rate"
          options={baudrates.map((b) => ({ value: b, label: String(b) }))}
        />
      </label>
      <label class="field">
        <span>Data bits</span>
        <Select
          bind:value={sim.dataBits}
          disabled={!stopped}
          label="Data bits"
          options={[
            { value: 8, label: "8 bits" },
            { value: 7, label: "7 bits" },
          ]}
        />
      </label>
      <label class="field">
        <span>Parity</span>
        <Select
          bind:value={sim.parity}
          disabled={!stopped}
          label="Parity"
          options={[
            { value: "N", label: "None" },
            { value: "E", label: "Even" },
            { value: "O", label: "Odd" },
          ]}
        />
      </label>
      <label class="field">
        <span>Stop bits</span>
        <Select
          bind:value={sim.stopBits}
          disabled={!stopped}
          label="Stop bits"
          options={[
            { value: 1, label: "1 stop bit" },
            { value: 2, label: "2 stop bits" },
          ]}
        />
      </label>
    {:else}
      <label class="field">
        <span>Port</span>
        <IntInput
          value={sim.port}
          min={1}
          max={65535}
          width="80px"
          label="Port"
          disabled={!stopped}
          onchange={(n) => (sim.port = n)}
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
          disabled={!stopped}
        />
        Other computers
      </label>
    {/if}
    {#if sim.running === null}
      <button class="btn accent" onclick={() => sim.start()}>Start</button>
    {:else}
      <button class="btn danger" onclick={() => sim.stop()}>Stop</button>
    {/if}
    <span class="status" aria-live="polite">
      {#if sim.running !== null}
        <span class="live"
          ><span class="dot"></span>Listening on {whereText(sim.running)}</span
        >
        {#if overNetwork(sim.running)}
          <button class="btn ghost" onclick={useInMain}
            >Connect Main to it</button
          >
        {/if}
      {:else}
        <span class="dim">Stopped</span>
      {/if}
    </span>
  </div>

  {#if serial}
    <p class="dim hint">
      Clients connect to the other end of the line. To try it on this computer,
      use a virtual null-modem pair (such as com0com or socat) and open its
      other port on the Main page. Unit ID 0 is a broadcast: every device takes
      the write and none answers.
    </p>
  {/if}

  <div class="devices" role="tablist" aria-label="Simulated devices">
    {#each sim.devices as d, i (i)}
      <button
        role="tab"
        class="device"
        aria-selected={sim.selected === i}
        class:on={sim.selected === i}
        onclick={() => sim.show(i)}>{deviceLabel(d)}</button
      >
    {/each}
    <button
      class="btn ghost"
      title="Another device at the same address, on its own unit ID"
      onclick={() => sim.addDevice()}>Add device</button
    >
  </div>

  <div class="row source">
    <label
      class="field"
      title="Leave empty to answer every unit ID that no other device has"
    >
      <span>Unit ID</span>
      {#key sim.selected}
        <input
          class="input num"
          style:width="80px"
          value={sim.unit ?? ""}
          placeholder="All"
          aria-label="Unit ID"
          onchange={async (e) => {
            const input = e.currentTarget;
            const n = parseInteger(input.value);
            await sim.setUnit(n !== null && n >= 0 && n <= 255 ? n : null);
            input.value = sim.unit === null ? "" : String(sim.unit);
          }}
        />
      {/key}
    </label>
    <label class="field">
      <span>Map</span>
      {#key [picks, sim.selected]}
        <Select
          value={sim.source ?? ""}
          options={sources}
          label="Map the device is shaped after"
          width="300px"
          onchange={choose}
        />
      {/key}
    </label>
    <label
      class="check"
      title="Only the map's addresses exist; the others answer exception 2"
    >
      <input
        type="checkbox"
        bind:checked={sim.strict}
        disabled={!sim.map}
        onchange={() => sim.reload()}
      />
      Only the map's addresses
    </label>
    <span class="spacer"></span>
    {#if sim.devices.length > 1}
      <button
        class="btn ghost"
        title="Remove this device; its unit ID gets no answer"
        onclick={() => sim.removeDevice()}>Remove device</button
      >
    {/if}
    {#if sim.map}
      <button
        class="btn ghost"
        title="Back to the map's reference values"
        onclick={() => sim.reload()}>Reset values</button
      >
      <div class="segmented" role="tablist" aria-label="View" use:slidePill>
        <button
          role="tab"
          aria-selected={mode === "registers"}
          class:on={mode === "registers"}
          onclick={() => (mode = "registers")}>Registers</button
        >
        <button
          role="tab"
          aria-selected={mode === "grid"}
          class:on={mode === "grid"}
          onclick={() => (mode = "grid")}>Grid</button
        >
      </div>
    {/if}
  </div>

  {#if sim.error}<p class="bad" role="alert">{sim.error}</p>{/if}

  {#if sim.map && mode === "registers"}
    <SimRegisters ongenerate={(r, i) => generators?.generateFor(r, i)} />
  {:else}
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
        <AddressInput
          value={first}
          width="80px"
          label="First address shown"
          onchange={(n, a) => {
            start = n;
            if (a) area = a;
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
        {#each Array(PER_ROW) as _, i (i)}<span class="colhead">+{i}</span
          >{/each}
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
                class:gen={driven.has(first + i)}
                title="{addr(first + i)}{defined
                  ? ''
                  : ' · not in the map'}{typeof v === 'number'
                  ? ` · 0x${hex(v)}`
                  : ''}{driven.has(first + i) ? ' · set by a generator' : ''}"
                onclick={() => edit(i)}
                oncontextmenu={(e) => contextMenu.show(e, cellMenu(i))}
                >{typeof v === "boolean" ? (v ? "1" : "0") : v}</button
              >
            {/if}
          {/each}
        {/each}
      </div>
    {/if}
  {/if}

  <SimGenerators bind:this={generators} />

  {#snippet log(title: string, list: SimEvent[], total: number, empty: string)}
    <div class="pane">
      <h3>{title} <span class="dim">{total}</span></h3>
      {#if list.length}
        <div class="log">
          <table class="data">
            <thead>
              <tr>
                <th>Time</th><th>Client</th><th class="num">Unit</th><th
                  >Request</th
                ><th>Address</th><th>Result</th>
              </tr>
            </thead>
            <tbody>
              {#each list as e, i (total - i)}
                <tr
                  oncontextmenu={(ev) =>
                    contextMenu.show(ev, [
                      { label: "Copy line", action: () => copy(eventLine(e)) },
                      {
                        label: "Copy all",
                        action: () =>
                          copy([...list].reverse().map(eventLine).join("\n")),
                      },
                      null,
                      {
                        label: "Clear activity",
                        danger: true,
                        action: () => sim.clearEvents(),
                      },
                    ])}
                >
                  <td class="mono dim">{clock(e.time_ms)}</td>
                  <td class="mono">{e.peer}</td>
                  <td class="num mono">{e.unit}</td>
                  <td>{functionName(e.function)}</td>
                  <td class="mono">{where(e)}</td>
                  <td
                    class:bad={!!e.error}
                    class:wrote={!e.error && e.wrote}
                    class:ok={!e.error && !e.wrote}
                    >{e.error ?? (e.wrote ? "written" : "OK")}</td
                  >
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <p class="empty">{empty}</p>
      {/if}
    </div>
  {/snippet}

  <section class="activity">
    <div class="row">
      <h2 class="key">Activity</h2>
      <span class="spacer"></span>
      <button
        class="btn ghost"
        disabled={!sim.reads.length && !sim.writes.length}
        onclick={() => sim.clearEvents()}>Clear</button
      >
    </div>
    <div class="panes">
      {@render log(
        "Reads",
        reads,
        sim.reads.length,
        sim.running === null
          ? "Start the simulator, then connect to it from the Main page or another program."
          : "Waiting for a client…",
      )}
      {@render log(
        "Writes",
        writes,
        sim.writes.length,
        "No client has written anything yet.",
      )}
    </div>
  </section>
{/if}

<style>
  p {
    margin: 0;
  }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .setup-note {
    color: var(--text-3);
    font-size: 12px;
  }

  .setup-note.bad {
    color: var(--bad);
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
    background: var(--live);
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }

  .with-btn {
    display: flex;
    gap: 4px;
  }

  .devices {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-bottom: -6px;
  }

  .device {
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: 9px 9px 0 0;
    background: transparent;
    color: var(--text-2);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
  }

  .device:hover {
    color: var(--text);
  }

  .device.on {
    background: var(--subtle);
    color: var(--text);
    font-weight: 600;
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

  /* Set by a generator: the device's own green, under the value */
  .cell.gen {
    box-shadow: inset 0 -2px 0 var(--rx);
  }

  input.cell {
    min-width: 0;
    width: 100%;
    border-radius: 3px;
  }

  /* The newest requests on top; older ones scroll inside, not the page */
  .log {
    max-height: 320px;
    overflow: auto;
    scrollbar-gutter: stable;
  }

  .log :global(thead th) {
    position: sticky;
    top: 0;
    background: var(--card);
  }

  /* Reads and writes side by side; one above the other when narrow */
  .panes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(440px, 1fr));
    gap: 10px 24px;
  }

  .pane {
    min-width: 0;
  }

  .pane h3 {
    margin: 0 0 4px;
    font-size: 13px;
    font-weight: 600;
  }

  .pane h3 .dim {
    margin-left: 4px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }

  .log td {
    white-space: nowrap;
  }

  .activity h2 {
    margin: 0;
  }

  .activity .row {
    align-items: center;
    margin-bottom: 4px;
  }

  td.wrote {
    color: var(--tx);
    font-weight: 600;
  }

  td.ok {
    color: var(--ok);
  }
</style>
