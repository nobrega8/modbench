<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import Select from "./Select.svelte";
  import { bench } from "#lib/state.svelte.ts";
  import type { Area, Parity, SerialHit } from "#lib/types.ts";
  import AddressInput from "./AddressInput.svelte";
  import { settings } from "#lib/settings.svelte.ts";
  import { serialScanCsv } from "#lib/scan-export.ts";
  import ExportCsv from "./ExportCsv.svelte";
  import IntInput from "./IntInput.svelte";

  // Most common first: they are tried in this order.
  const BAUDS = [9600, 19200, 38400, 115200, 4800, 57600, 2400, 1200];
  const PARITIES: { id: Parity; label: string }[] = [
    { id: "E", label: "Even" },
    { id: "N", label: "None" },
    { id: "O", label: "Odd" },
  ];

  let bauds = $state<number[]>([9600, 19200, 38400, 115200]);
  let parities = $state<Parity[]>(["E", "N", "O"]);
  let stops = $state<number[]>([1, 2]);
  let from = $state(bench.unitId || 1);
  let to = $state(bench.unitId || 1);
  let area = $state<Area>("holding");
  let address = $state(0);
  let timeout = $state(200);
  let first = $state(true);

  const busy = $derived(bench.scanning);
  const combos = $derived(bauds.length * parities.length * stops.length);
  const units = $derived(Math.max(0, to - from + 1));
  /** Worst case: nothing answers, every request waits the full time. */
  const worstSeconds = $derived(Math.ceil((combos * units * timeout) / 1000));
  const p = $derived(bench.serialProgress);

  function toggle<T>(list: T[], item: T, order: T[]): T[] {
    const next = list.includes(item)
      ? list.filter((x) => x !== item)
      : [...list, item];
    return order.filter((x) => next.includes(x));
  }

  function duration(s: number): string {
    if (s < 90) return `${s} s`;
    if (s < 5400) return `${Math.round(s / 60)} min`;
    return `${(s / 3600).toFixed(1)} h`;
  }

  function inUse(h: SerialHit): boolean {
    return (
      bench.baudrate === h.baudrate &&
      bench.parity === h.parity &&
      bench.dataBits === h.data_bits &&
      bench.stopBits === h.stop_bits &&
      bench.unitId === h.unit
    );
  }

  function start() {
    bench.scanSerial({
      baudrates: BAUDS.filter((b) => bauds.includes(b)),
      parities,
      stop_bits: stops,
      from,
      to,
      area,
      address,
      timeout_ms: timeout,
      stop_at_first: first,
    });
  }
</script>

<div class="opts">
  <div class="field">
    <span>{t("serialscan.baud_rates")}</span>
    <div class="chips">
      {#each BAUDS as b (b)}
        <label class="chip"
          ><input
            type="checkbox"
            checked={bauds.includes(b)}
            disabled={busy}
            onchange={() => (bauds = toggle(bauds, b, BAUDS))}
          />{b}</label
        >
      {/each}
    </div>
  </div>
  <div class="field">
    <span>{t("conn.parity")}</span>
    <div class="chips">
      {#each PARITIES as par (par.id)}
        <label class="chip"
          ><input
            type="checkbox"
            checked={parities.includes(par.id)}
            disabled={busy}
            onchange={() =>
              (parities = toggle(
                parities,
                par.id,
                PARITIES.map((x) => x.id),
              ))}
          />{par.label}</label
        >
      {/each}
    </div>
  </div>
  <div class="field">
    <span>{t("conn.stopBits")}</span>
    <div class="chips">
      {#each [1, 2] as s (s)}
        <label class="chip"
          ><input
            type="checkbox"
            checked={stops.includes(s)}
            disabled={busy}
            onchange={() => (stops = toggle(stops, s, [1, 2]))}
          />{s}</label
        >
      {/each}
    </div>
  </div>
</div>

<div class="row">
  <label class="field">
    <span>{t("serialscan.unit_from")}</span>
    <IntInput
      value={from}
      min={0}
      max={255}
      width="72px"
      label={t("scanpanel.first_unit_id")}
      disabled={busy}
      onchange={(n) => (from = n)}
    />
  </label>
  <label class="field">
    <span>{t("scanpanel.to")}</span>
    <IntInput
      value={to}
      min={0}
      max={255}
      width="72px"
      label={t("scanpanel.last_unit_id")}
      disabled={busy}
      onchange={(n) => (to = n)}
    />
  </label>
  <label class="field">
    <span>{t("serialscan.ask_for")}</span>
    <Select
      bind:value={area}
      disabled={busy}
      label={t("serialscan.ask_for")}
      options={[
        { value: "holding", label: "Holding register" },
        { value: "input", label: "Input register" },
      ]}
    />
  </label>
  <label class="field">
    <span>{t("req.address")}</span>
    <AddressInput
      value={address}
      areas={["holding", "input"]}
      width="88px"
      label={t("serialscan.register_address")}
      disabled={busy}
      onchange={(n, a) => {
        address = n;
        if (a) area = a;
      }}
    />
  </label>
  <label class="field" title={t("serialscan.how_long_to_wait")}>
    <span>{t("scanpanel.wait_ms")}</span>
    <IntInput
      value={timeout}
      min={20}
      max={5000}
      width="80px"
      label={t("serialscan.wait_per_request")}
      disabled={busy}
      onchange={(n) => (timeout = n)}
    />
  </label>
  <label class="check stop"
    ><input type="checkbox" bind:checked={first} disabled={busy} />
    {t("serialscan.stop_at_the_first")}</label
  >
  {#if busy}
    <button class="btn" onclick={() => bench.cancelScan()}
      >{t("test.stop")}</button
    >
  {:else}
    <button
      class="btn accent"
      disabled={bench.running || !combos || !units || bench.serialPort === ""}
      onclick={start}>{t("scanpanel.scan")}</button
    >
  {/if}
</div>

{#if busy}
  <div class="status">
    <div
      class="bar"
      role="progressbar"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={p ? Math.round((p.done / p.total) * 100) : 0}
    >
      <div style:width="{p ? (p.done / p.total) * 100 : 0}%"></div>
    </div>
    {#if p}
      <span class="dim mono"
        >{p.baudrate} 8{p.parity}{p.stop_bits} · unit {p.unit} · {p.done + 1} of
        {p.total}</span
      >
    {/if}
  </div>
{:else}
  <p class="dim">
    {combos} setting{combos === 1 ? "" : "s"} × {units} unit ID{units === 1
      ? ""
      : "s"}. At most {duration(worstSeconds)}.
  </p>
{/if}

{#if bench.scanError}
  <p class="bad">{bench.scanError}</p>
{:else if bench.serialHits && !busy}
  {#if bench.lastSerialScan}
    {@const last = bench.lastSerialScan}
    <div class="export">
      <ExportCsv
        name="modbench-serial-scan.csv"
        make={() =>
          serialScanCsv(
            bench.serialHits ?? [],
            last.scan,
            last,
            settings.csvSeparator,
          )}
      />
    </div>
  {/if}
  {#if bench.serialHits.length}
    <ul class="hits">
      {#each bench.serialHits as h, i (i)}
        <li>
          <span class="mono setting"
            >{h.baudrate} 8{h.parity}{h.stop_bits} · unit {h.unit}</span
          >
          <span class="dim">{h.response}</span>
          <button
            class="btn"
            disabled={inUse(h)}
            onclick={() => bench.useSerial(h)}
            >{inUse(h) ? "In use" : "Use"}</button
          >
        </li>
      {/each}
    </ul>
  {:else}
    <p class="dim">
      {t("serialscan.no_answer_check_the")}
    </p>
  {/if}
{/if}

<style>
  .export {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .opts {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 22px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    min-height: 30px;
    align-items: center;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: var(--mono);
    font-size: 13px;
    cursor: pointer;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .stop {
    height: 30px;
  }

  .status {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent, currentColor);
  }

  .hits {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .hits li {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .setting {
    min-width: 170px;
    font-weight: 600;
  }

  .mono {
    font-family: var(--mono);
    font-size: 13px;
  }

  .dim {
    color: var(--text-3);
    margin: 0;
  }

  .bad {
    color: var(--bad);
    margin: 0;
  }
</style>
