<script module lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
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

  // The list is only read while stopped: keep the port it listens on shown.
  const portOptions = $derived(
    (sim.serialPort && !ports.includes(sim.serialPort)
      ? [sim.serialPort, ...ports]
      : ports
    ).map((p) => ({ value: p, label: p })),
  );

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
    <h1>{t("tools.simulator")}</h1>
    <p>
      The simulator is open in its own window{sim.running !== null
        ? `, listening on ${whereText(sim.running)}`
        : ""}.
    </p>
  </header>
  <div class="row">
    <button class="btn accent" onclick={() => api.simOpenWindow()}
      >{t("simulatortool.show_its_window")}</button
    >
    <button class="btn" onclick={() => api.simCloseWindow()}
      >{t("simulatortool.bring_it_back_here")}</button
    >
  </div>
{:else}
  <header class="tool-head">
    <div class="title-row">
      <h1>{t("tools.simulator")}</h1>
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
        title={t("simulatortool.load_the_interface_devices")}
        onclick={openSetup}>{t("simulatortool.open_setup")}</button
      >
      <button
        class="btn ghost"
        title={t("simulatortool.save_the_interface_devices")}
        onclick={saveSetup}>{t("simulatortool.save_setup")}</button
      >
      {#if IN_SIM_WINDOW}
        <button
          class="btn ghost"
          title={t("simulatortool.close_this_window_and")}
          onclick={() => api.simCloseWindow()}
          >{t("simulatortool.back_to_the_main")}</button
        >
      {:else}
        <button
          class="btn ghost"
          title={t("simulatortool.open_the_simulator_in")}
          onclick={() => api.simOpenWindow()}
          ><Icon name="popout" />{t("simulatortool.pop_out")}</button
        >
      {/if}
    </div>
    <p>{t("simulatortool.devices_on_this_computer")}</p>
  </header>

  <div class="row">
    <label class="field">
      <span>{t("conn.interface")}</span>
      <Select
        bind:value={sim.kind}
        options={kinds}
        label={t("simulatortool.how_clients_reach_the")}
        disabled={!stopped}
      />
    </label>
    {#if serial}
      <label class="field">
        <span>{t("conn.serialPort")}</span>
        <span class="with-btn">
          <Select
            bind:value={sim.serialPort}
            disabled={!stopped}
            label={t("conn.serialPort")}
            placeholder={t("conn.noPorts")}
            options={portOptions}
          />
          <button
            class="btn icon"
            title={t("conn.rescan")}
            aria-label={t("conn.rescan")}
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
        <span>{t("conn.baud")}</span>
        <Select
          bind:value={sim.baudrate}
          disabled={!stopped}
          label={t("conn.baud")}
          options={baudrates.map((b) => ({ value: b, label: String(b) }))}
        />
      </label>
      <label class="field">
        <span>{t("conn.dataBits")}</span>
        <Select
          bind:value={sim.dataBits}
          disabled={!stopped}
          label={t("conn.dataBits")}
          options={[
            { value: 8, label: "8 bits" },
            { value: 7, label: "7 bits" },
          ]}
        />
      </label>
      <label class="field">
        <span>{t("conn.parity")}</span>
        <Select
          bind:value={sim.parity}
          disabled={!stopped}
          label={t("conn.parity")}
          options={[
            { value: "N", label: "None" },
            { value: "E", label: "Even" },
            { value: "O", label: "Odd" },
          ]}
        />
      </label>
      <label class="field">
        <span>{t("conn.stopBits")}</span>
        <Select
          bind:value={sim.stopBits}
          disabled={!stopped}
          label={t("conn.stopBits")}
          options={[
            { value: 1, label: "1 stop bit" },
            { value: 2, label: "2 stop bits" },
          ]}
        />
      </label>
    {:else}
      <label class="field">
        <span>{t("conn.port")}</span>
        <IntInput
          value={sim.port}
          min={1}
          max={65535}
          width="80px"
          label={t("conn.port")}
          disabled={!stopped}
          onchange={(n) => (sim.port = n)}
        />
      </label>
      <label
        class="check switch-row"
        title={t("simulatortool.let_other_computers_on")}
      >
        <input
          type="checkbox"
          class="switch"
          role="switch"
          bind:checked={sim.isPublic}
          disabled={!stopped}
        />
        {t("simulatortool.other_computers")}
      </label>
    {/if}
    {#if sim.running === null}
      <button class="btn accent" onclick={() => sim.start()}
        >{t("test.start")}</button
      >
    {:else}
      <button class="btn danger" onclick={() => sim.stop()}
        >{t("test.stop")}</button
      >
    {/if}
    <span class="status" aria-live="polite">
      {#if sim.running !== null}
        <span class="live"
          ><span class="dot"></span>Listening on {whereText(sim.running)}</span
        >
        {#if overNetwork(sim.running)}
          <button class="btn ghost" onclick={useInMain}
            >{t("simulatortool.connect_main_to_it")}</button
          >
        {/if}
      {:else}
        <span class="dim">{t("test.stopped")}</span>
      {/if}
    </span>
  </div>

  {#if serial}
    <p class="dim hint">
      {t("simulatortool.clients_connect_to_the")}
    </p>
  {/if}

  <div
    class="devices"
    role="tablist"
    aria-label={t("simulatortool.simulated_devices")}
  >
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
      title={t("simulatortool.another_device_at_the")}
      onclick={() => sim.addDevice()}>{t("simulatortool.add_device")}</button
    >
  </div>

  <div class="row source">
    <label class="field" title={t("simulatortool.leave_empty_to_answer")}>
      <span>{t("networkscantool.unit_id")}</span>
      {#key sim.selected}
        <input
          class="input num"
          style:width="80px"
          value={sim.unit ?? ""}
          placeholder={t("simulatortool.all")}
          aria-label={t("networkscantool.unit_id")}
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
      <span>{t("req.mapField")}</span>
      {#key [picks, sim.selected]}
        <Select
          value={sim.source ?? ""}
          options={sources}
          label={t("simulatortool.map_the_device_is")}
          width="300px"
          onchange={choose}
        />
      {/key}
    </label>
    <label class="check" title={t("simulatortool.only_the_map_s")}>
      <input
        type="checkbox"
        bind:checked={sim.strict}
        disabled={!sim.map}
        onchange={() => sim.reload()}
      />
      {t("simulatortool.only_the_map_s2")}
    </label>
    <span class="spacer"></span>
    {#if sim.devices.length > 1}
      <button
        class="btn ghost"
        title={t("simulatortool.remove_this_device_its")}
        onclick={() => sim.removeDevice()}
        >{t("simulatortool.remove_device")}</button
      >
    {/if}
    {#if sim.map}
      <button
        class="btn ghost"
        title={t("simulatortool.back_to_the_map")}
        onclick={() => sim.reload()}>{t("simulatortool.reset_values")}</button
      >
      <div
        class="segmented"
        role="tablist"
        aria-label={t("simulatortool.view")}
        use:slidePill
      >
        <button
          role="tab"
          aria-selected={mode === "registers"}
          class:on={mode === "registers"}
          onclick={() => (mode = "registers")}
          >{t("libraryview.registers")}</button
        >
        <button
          role="tab"
          aria-selected={mode === "grid"}
          class:on={mode === "grid"}
          onclick={() => (mode = "grid")}>{t("registergrid.grid")}</button
        >
      </div>
    {/if}
  </div>

  {#if sim.error}<p class="bad" role="alert">{sim.error}</p>{/if}

  {#if sim.map && mode === "registers"}
    <SimRegisters ongenerate={(r, i) => generators?.generateFor(r, i)} />
  {:else}
    <div class="row">
      <div
        class="segmented"
        role="tablist"
        aria-label={t("customregisterform.area")}
        use:slidePill
      >
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
        <span class="dim">{t("scanpanel.from")}</span>
        <AddressInput
          value={first}
          width="80px"
          label={t("simulatortool.first_address_shown")}
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
      {bits ? "Click a bit to switch it." : "Click a value to change it."}
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
                <th>{t("logview.time")}</th><th
                  >{t("framedecodertool.client")}</th
                ><th class="num">{t("computedform.unit")}</th><th
                  >{t("step.request")}</th
                ><th>{t("req.address")}</th><th>{t("logview.result")}</th>
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
      <h2 class="key">{t("simulatortool.activity")}</h2>
      <span class="spacer"></span>
      <button
        class="btn ghost"
        disabled={!sim.reads.length && !sim.writes.length}
        onclick={() => sim.clearEvents()}>{t("chartview.clear")}</button
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
