<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { valuesCsv } from "#lib/csv.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import {
    AREA_TAG,
    AREA_NAME,
    formatAddress,
    formatRaw,
    formatValue,
    parseForRegister,
    limitError,
    valueTitle,
  } from "#lib/format.ts";
  import { bench, type OpenMap } from "#lib/state.svelte.ts";
  import { appliesTo, rangeText, versionText } from "#lib/identity.ts";
  import { byIndex, check, limitsText } from "#lib/alarms.ts";
  import { diagnosticWhere, type Register } from "#lib/types.ts";
  import { isCustom } from "#lib/custom-registers.ts";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";
  import { registerId } from "#lib/profiles.ts";
  import { MAX_SERIES } from "#lib/trend.ts";
  import AddressFormat from "./AddressFormat.svelte";
  import CustomRegisterForm from "./CustomRegisterForm.svelte";
  import ComputedForm from "./ComputedForm.svelte";
  import { formulaOf } from "#lib/computed.ts";
  import Select from "./Select.svelte";
  import { mapStatus } from "./MapStatus.svelte";
  import SnapshotBar from "./SnapshotBar.svelte";
  import { differs, shownText, snapText } from "#lib/snapshots.ts";

  let { open }: { open: OpenMap } = $props();

  let search = $state("");
  let group = $state("");
  let editing = $state<number | null>(null);
  let draft = $state("");
  /** A value of one's own is typed instead of picking a label. */
  let custom = $state(false);
  /** Picked in a labelled list to type a value instead. */
  const CUSTOM = "__custom__";
  let note = $state<{ index: number; ok: boolean; text: string } | null>(null);
  let busy = $state(false);
  let converting = $state(false);
  /** The form to add a register the map is missing is open. */
  let adding = $state(false);
  /** The form to add a computed value is open. */
  let computing = $state(false);
  let convertError = $state<string | null>(null);

  async function convert() {
    const index = bench.maps.indexOf(open);
    if (index < 0) return;
    converting = true;
    convertError = null;
    try {
      await bench.convertMap(index);
    } catch (e) {
      convertError = errorText(e);
    } finally {
      converting = false;
    }
  }

  const meta = $derived(open.map.meta);
  /** The library's x_status of the map: how far it can be trusted. */
  const status = $derived(
    mapStatus(meta.extra.find(([k]) => k === "x_status")?.[1]),
  );
  const alarms = $derived(byIndex(bench.alarms, open.map.registers));
  /** Registers whose value is outside their alarm limits right now. */
  const inAlarm = $derived.by(() => {
    const out = new Set<number>();
    for (const [index, a] of alarms) {
      const r = bench.mapResults.get(index);
      if (r && !r.error && !r.sentinel && check(a, r.value)) out.add(index);
    }
    return out;
  });
  const identity = $derived(bench.identity);
  /** The device's firmware, once its firmware registers were read. */
  const fwVersion = $derived(identity?.firmware.version ?? null);
  const modelNames = $derived(
    (meta.models?.length ? meta.models : [meta.model]).join(", "),
  );
  const mapRange = $derived(rangeText(meta.firmware_min, meta.firmware_max));
  const mismatch = $derived(
    identity?.model.status === "mismatch" ||
      identity?.firmware.status === "mismatch",
  );
  const recognised = $derived(
    !mismatch &&
      (identity?.model.status === "match" ||
        identity?.firmware.status === "match"),
  );
  const base = $derived(meta.address_base ?? 0);
  const groups = $derived([
    ...new Set(open.map.registers.map((r) => r.group).filter(Boolean)),
  ]);
  const rows = $derived(
    open.map.registers
      .map((reg, index) => ({ reg, index }))
      .filter(
        ({ index }) => !snap || !bench.onlyDifferences || different.has(index),
      )
      .filter(({ reg }) => {
        // Registers the device's firmware does not have are left out.
        if (!appliesTo(reg, fwVersion)) return false;
        if (group && reg.group !== group) return false;
        const q = search.trim().toLowerCase();
        return (
          !q ||
          reg.name.toLowerCase().includes(q) ||
          reg.description.toLowerCase().includes(q) ||
          String(reg.address).includes(q) ||
          formatAddress(reg.address, settings.addressFormat, reg.area)
            .toLowerCase()
            .includes(q)
        );
      })
      // Registers that are not read go to the end, in map order.
      .sort(
        (a, b) =>
          Number(bench.skipped.has(a.index)) -
            Number(bench.skipped.has(b.index)) || a.index - b.index,
      ),
  );
  const readCount = $derived(open.map.registers.length - bench.skipped.size);

  /** The map snapshot to compare with, when there is one. */
  const snap = $derived(bench.snapshot?.map ? bench.snapshot : null);
  /** A register's value on the other side of the comparison. */
  function after(reg: Register, index: number): string | undefined {
    if (bench.compareWith) return snapText(bench.compareWith, reg);
    const r = bench.mapResults.get(index);
    return r ? shownText(reg, r) : undefined;
  }
  /** Registers whose value differs from the snapshot. */
  const different = $derived.by(() => {
    const out = new Set<number>();
    if (!snap) return out;
    open.map.registers.forEach((reg, i) => {
      if (differs(snapText(snap, reg), after(reg, i))) out.add(i);
    });
    return out;
  });

  /** Outcome of the map's reference checks (spec 4.6), from the last cycle. */
  const refs = $derived.by(() => {
    const out = { total: 0, match: 0, mismatch: 0, failed: 0 };
    open.map.registers.forEach((reg, i) => {
      if (!reg.reference) return;
      out.total++;
      const r = bench.mapResults.get(i);
      if (bench.skipped.has(i) || !r) return;
      if (r.error) out.failed++;
      else if (r.reference === "match") out.match++;
      else if (r.reference === "mismatch") out.mismatch++;
    });
    return out;
  });

  let exportNote = $state<{ ok: boolean; text: string } | null>(null);
  const recording = $derived(
    bench.recording?.mapPath === open.path ? bench.recording : null,
  );

  function csvName(suffix: string) {
    const name = `${meta.vendor}-${meta.model}`.replace(/[^\w.-]+/g, "_");
    return `${name}-${suffix}.csv`;
  }

  /** Saves the values on screen, in the order shown. */
  async function exportValues() {
    exportNote = null;
    const path = await save({
      defaultPath: csvName("values"),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      await api.saveText(
        path,
        valuesCsv(
          open.map.registers,
          rows.map((r) => r.index),
          bench.mapResults,
          settings.csvSeparator,
        ),
      );
      exportNote = { ok: true, text: "Saved." };
    } catch (e) {
      exportNote = { ok: false, text: `Not saved: ${errorText(e)}` };
    }
  }

  async function toggleRecording() {
    if (recording) {
      bench.stopRecording();
      return;
    }
    const path = await save({
      defaultPath: csvName("recording"),
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (path) await bench.startRecording(path);
  }

  function typeLabel(reg: Register): string {
    const formula = formulaOf(reg);
    if (formula !== null) return `= ${formula}`;
    if (reg.type === "string") return `string[${reg.length}]`;
    if (reg.type === "bool" && reg.bit !== null) return `bit ${reg.bit}`;
    if (reg.bits !== null && reg.bit !== null)
      return reg.bits === 1
        ? `bit ${reg.bit}`
        : `bits ${reg.bit}–${reg.bit + reg.bits - 1}`;
    const wide = !["bool", "int16", "uint16"].includes(reg.type);
    const order =
      bench.orderOverride ?? reg.byte_order ?? meta.byte_order ?? "ABCD";
    const base = wide ? `${reg.type} ${order}` : reg.type;
    return reg.scale_factor ? `${base} ×10^SF` : base;
  }

  function edit(index: number) {
    editing = index;
    custom = false;
    note = null;
    const reg = open.map.registers[index];
    const v = bench.mapResults.get(index)?.value;
    draft = v === null || v === undefined ? "" : formatValue(reg, v);
  }

  /** Sets an alarm on a register (when it has none) and opens Alarms. */
  function alarmFor(reg: Register) {
    const id = registerId(reg);
    if (!bench.alarms.some((a) => a.id === id))
      bench.setAlarms([
        ...bench.alarms,
        {
          id,
          low: null,
          high: null,
          when: reg.type === "bool" ? true : null,
        },
      ]);
    bench.tool = "alarms";
    bench.page = "tools";
  }

  /** What a right-click on a register offers. */
  function rowMenu(reg: Register, index: number): MenuEntry[] {
    const r = bench.mapResults.get(index);
    const readable = reg.access !== "W";
    const charted = bench.chartSeries.some((s) => s.index === index);
    const shown =
      readable && r && !r.error && !r.sentinel
        ? formatValue(reg, r.value ?? null)
        : null;
    return [
      reg.access !== "R" && {
        label: "Write…",
        disabled: settings.readOnly,
        hint: settings.readOnly ? "read-only" : undefined,
        action: () => edit(index),
      },
      readable && {
        label: bench.skipped.has(index) ? "Read it" : "Stop reading it",
        action: () => bench.toggleRead(index),
      },
      readable &&
        reg.type !== "string" &&
        (charted
          ? {
              label: "Remove from chart",
              action: () => bench.removeFromChart(index),
            }
          : {
              label: "Add to chart",
              disabled: bench.chartSeries.length >= MAX_SERIES,
              hint:
                bench.chartSeries.length >= MAX_SERIES
                  ? `${MAX_SERIES} at most`
                  : undefined,
              action: () => bench.addToChart(index),
            }),
      readable &&
        reg.type !== "string" && {
          label: alarms.has(index) ? "Change alarm…" : "Set alarm…",
          action: () => alarmFor(reg),
        },
      null,
      shown !== null && {
        label: "Copy value",
        action: () => copy(reg.unit ? `${shown} ${reg.unit}` : shown),
      },
      r?.raw !== undefined &&
        r?.raw !== null &&
        !r.error && {
          label: "Copy raw value",
          action: () => copy(formatRaw(r.raw ?? null)),
        },
      { label: "Copy name", action: () => copy(reg.name) },
      {
        label: "Copy address",
        action: () =>
          copy(formatAddress(reg.address, settings.addressFormat, reg.area)),
      },
      null,
      isCustom(reg) && {
        label: "Remove register",
        danger: true,
        action: () => bench.removeCustomRegister(index),
      },
    ];
  }

  async function write(index: number, text: string) {
    const reg = open.map.registers[index];
    const parsed = parseForRegister(reg, text);
    if ("error" in parsed) {
      note = { index, ok: false, text: parsed.error };
      return;
    }
    const outside = limitError(reg, parsed.value);
    if (outside) {
      note = { index, ok: false, text: outside };
      return;
    }
    if (!(await bench.confirmWrite(`Write ${text} to ${reg.name}?`))) return;
    busy = true;
    try {
      await bench.writeMapValue(index, parsed.value);
      note = { index, ok: true, text: `Wrote ${text} to ${reg.name}.` };
      editing = null;
    } catch (e) {
      note = { index, ok: false, text: `Not written: ${errorText(e)}` };
    } finally {
      busy = false;
    }
  }
</script>

<div class="toolbar">
  <div class="title">
    <strong title="{meta.vendor} {meta.model}"
      >{meta.vendor} {meta.model}</strong
    >
    {#if meta.revision}<span
        class="rev"
        title="Revision {meta.revision} of this map{meta.revised
          ? `, ${meta.revised}`
          : ''}">rev {meta.revision}</span
      >{/if}
    {#if meta.description}<span class="desc" title={meta.description}
        >{meta.description}</span
      >{/if}
  </div>
  {#if status === "official"}
    <span
      class="refs"
      title="Written from the manufacturer's documentation, but not tested on a device yet. Check a value on the device before relying on the rest."
      >Official</span
    >
  {:else if status === "unofficial"}
    <span
      class="refs bad"
      title="Converted from another catalogue and not checked against the manufacturer's documentation. Check a value on the device before relying on the rest."
      >Unofficial</span
    >
  {/if}
  {#if recognised && identity}
    <span
      class="refs ok"
      title={[
        identity.model.found && `Model ${identity.model.found}`,
        fwVersion && `firmware ${versionText(fwVersion)}`,
        identity.serial && `serial number ${identity.serial}`,
      ]
        .filter(Boolean)
        .join(", ")}>Device recognised</span
    >
  {:else if mapRange && !fwVersion}
    <span
      class="refs"
      title="This map is for firmware {mapRange}. The device's firmware is not known, so it was not checked."
      >Firmware {mapRange}</span
    >
  {/if}
  {#if refs.total}
    <span
      class="refs"
      class:bad={refs.mismatch > 0 || refs.failed > 0}
      title="Registers with a known value in the map. A mismatch points to a wrong map or wrong settings (address, byte order, type); a failed read to a connection or address problem."
      >Reference {refs.match}/{refs.total}{#if refs.mismatch}
        · {refs.mismatch} differ{/if}{#if refs.failed}
        · {refs.failed} failed{/if}</span
    >
  {/if}
  {#if alarms.size}
    <button
      class="refs alarms"
      class:bad={inAlarm.size > 0}
      title="Alarm limits set on {alarms.size} registers. Click to change them."
      onclick={() => {
        bench.tool = "alarms";
        bench.page = "tools";
      }}
      >{inAlarm.size
        ? `${inAlarm.size} in alarm`
        : `Alarms ${alarms.size} · OK`}</button
    >
  {/if}
  <span class="count">Reading {readCount} of {open.map.registers.length}</span>
  <button
    class="btn small"
    disabled={readCount === open.map.registers.length}
    onclick={() => bench.setAllRead(true)}>Read all</button
  >
  <button
    class="btn small"
    disabled={readCount === 0}
    onclick={() => bench.setAllRead(false)}>Read none</button
  >
  <button
    class="btn small"
    title="Save the values shown to a CSV file"
    disabled={!bench.mapResults.size}
    onclick={exportValues}>Export…</button
  >
  <button
    class="btn small"
    class:rec={recording}
    title={recording
      ? `Recording to ${recording.path}. Click to stop.`
      : "Add one row per reading cycle to a CSV file, with a column for each register that is read now"}
    disabled={!recording && readCount === 0}
    onclick={toggleRecording}
    >{recording ? `Stop recording (${recording.rows})` : "Record…"}</button
  >
  <SnapshotBar
    kind="map"
    canTake={bench.mapResults.size > 0}
    differences={snap ? different.size : null}
  />
  {#if exportNote}<span class:bad={!exportNote.ok} class="note"
      >{exportNote.text}</span
    >{/if}
  {#if bench.recordError}<span class="bad note" title={bench.recordError}
      >Recording stopped: {bench.recordError}</span
    >{/if}
  <input
    class="input search"
    type="search"
    placeholder="Search name or address"
    bind:value={search}
    aria-label="Search registers"
  />
  <AddressFormat />
  {#if groups.length}
    <Select
      bind:value={group}
      label="Group"
      options={[
        { value: "", label: "All groups" },
        ...groups.map((g) => ({ value: g, label: g })),
      ]}
    />
  {/if}
</div>

{#if mismatch && identity}
  <div class="mismatch" role="alert">
    <strong>The connected device does not match this map.</strong>
    {#if identity.model.status === "mismatch"}<span
        >The map is for {modelNames}; the device reports {identity.model
          .found}.</span
      >{/if}
    {#if identity.firmware.status === "mismatch" && fwVersion}<span
        >The map is for firmware {mapRange}; the device reports {versionText(
          fwVersion,
        )}.</span
      >{/if}
    <span class="dim"
      >Values may be wrong. Writes ask for confirmation first.</span
    >
  </div>
{/if}

{#if open.version === 1}
  <div class="deprecated">
    <span
      >This map uses format version 1 (CSV), which is deprecated. Convert it to
      version 2 (JSON); the original file is kept.</span
    >
    <button class="btn" disabled={converting} onclick={convert}
      >{converting ? "Converting…" : "Convert to version 2"}</button
    >
    {#if convertError}<span class="error">{convertError}</span>{/if}
  </div>
{/if}

{#if open.warnings.length}
  <details class="warnings">
    <summary
      >{open.warnings.length} warning{open.warnings.length === 1 ? "" : "s"} in the
      map file</summary
    >
    <ul>
      {#each open.warnings as w, k (k)}
        <li>
          <span class="where">{diagnosticWhere(w)}</span>
          {w.message}
        </li>
      {/each}
    </ul>
  </details>
{/if}

<div class="table-wrap">
  <table>
    <thead>
      <tr>
        <th class="read">Read</th>
        <th class="addr">Address</th>
        <th>Name</th>
        <th>Type</th>
        <th class="val">Value</th>
        {#if snap}<th
            class="val"
            title="The value in the snapshot {snap.label}{bench.compareWith
              ? `, compared with ${bench.compareWith.label}`
              : ', compared with the live value'}">Snapshot</th
          >{/if}
        <th>Unit</th>
        <th>Raw</th>
        <th class="act"></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as { reg, index } (index)}
        {@const r = bench.mapResults.get(index)}
        {@const off = bench.skipped.has(index)}
        {@const shown =
          reg.access === "W" || r?.sentinel
            ? null
            : formatValue(reg, r?.value ?? null)}
        {@const shownTitle =
          shown === null ? undefined : valueTitle(reg, r?.value ?? null)}
        <tr
          class:err={!!r?.error}
          class:off
          class:diff={different.has(index)}
          oncontextmenu={(e) => contextMenu.show(e, rowMenu(reg, index))}
        >
          <td class="read">
            {#if reg.access !== "W"}
              <input
                type="checkbox"
                class="switch"
                role="switch"
                checked={!off}
                aria-label="Read {reg.name}"
                title={off ? "Not read. Switch on to read it." : "Read"}
                onchange={() => bench.toggleRead(index)}
              />
            {/if}
          </td>
          <td
            class="addr mono"
            title={formulaOf(reg) !== null
              ? "Computed on this computer, not read from the device"
              : `${AREA_NAME[reg.area]} ${reg.address} on the wire${base ? ` (${reg.address + base} in the manual)` : ""}`}
            >{#if formulaOf(reg) !== null}<span class="area">calc</span
              >{:else}<span class="area">{AREA_TAG[reg.area]}</span
              >{formatAddress(
                reg.address,
                settings.addressFormat,
                reg.area,
              )}{/if}</td
          >
          <td class="name" title={reg.description || undefined}
            >{reg.name}{#if reg.group && !group}<span class="grp"
                >{reg.group}</span
              >{/if}{#if !fwVersion && (reg.firmware_min || reg.firmware_max)}<span
                class="grp"
                title="Only on firmware {rangeText(
                  reg.firmware_min,
                  reg.firmware_max,
                )}. The device's firmware is not known, so it is read anyway."
                >fw {rangeText(reg.firmware_min, reg.firmware_max)}</span
              >{/if}{#if isCustom(reg)}<span
                class="grp custom"
                title="Added on this computer; not in the map file">custom</span
              >{/if}{#if reg.reference}<span
                class="refmark"
                class:ok={r?.reference === "match"}
                class:bad={r?.reference === "mismatch"}
                title={r?.reference === "match"
                  ? `Matches the reference value ${reg.reference}`
                  : r?.reference === "mismatch"
                    ? `Differs from the reference value ${reg.reference}`
                    : `Reference value ${reg.reference}`}>ref</span
              >{/if}</td
          >
          <td
            class="type mono"
            title={reg.scale_factor
              ? `Scaled by 10 to the power of ${reg.scale_factor}`
              : undefined}>{typeLabel(reg)}</td
          >
          <td
            class="val mono"
            class:changed={bench.fresh(`m:${index}`)}
            class:alarm={inAlarm.has(index)}
            title={alarms.has(index)
              ? `${shownTitle ?? ""}\nAlarm ${limitsText(alarms.get(index)!, reg.unit)}`.trim()
              : shownTitle}
          >
            {#if reg.access === "W"}<span class="dim">write only</span
              >{:else if r?.sentinel}<span
                class="sentinel"
                title="The device returned a reserved value: {r.sentinel}"
                >{r.sentinel}</span
              >{:else}{shown}{/if}
          </td>
          {#if snap}
            {@const before = snapText(snap, reg)}
            <td
              class="val mono snapcell"
              title={different.has(index)
                ? `Snapshot: ${before}\n${bench.compareWith ? bench.compareWith.label : "Now"}: ${after(reg, index)}`
                : undefined}
              >{#if different.has(index)}<span class="ne" aria-label="differs"
                  >≠</span
                >{/if}{before ?? "—"}</td
            >
          {/if}
          <td class="unit">{r?.sentinel ? "" : reg.unit}</td>
          <td class="raw mono" title={r?.error ?? undefined}>
            {#if r?.error}<span class="bad">{r.error}</span>{:else}{formatRaw(
                r?.raw ?? null,
              )}{/if}
          </td>
          <td class="act">
            {#if reg.access !== "R"}
              <button class="btn small" onclick={() => edit(index)}
                >Write…</button
              >
            {/if}
            {#if isCustom(reg)}
              <button
                class="btn ghost small"
                title="Remove this register you added"
                onclick={() => bench.removeCustomRegister(index)}>Remove</button
              >
            {/if}
          </td>
        </tr>
        {#if editing === index}
          <tr class="editor">
            <td colspan="2"></td>
            <td colspan={snap ? 7 : 6}>
              <form
                onsubmit={(e) => {
                  e.preventDefault();
                  write(index, draft);
                }}
              >
                <span>New value for <strong>{reg.name}</strong></span>
                {#if reg.type === "bool"}
                  {#each [true, false] as on (on)}
                    {@const label =
                      reg.enum.find(([v]) => v === (on ? 1 : 0))?.[1] ??
                      (on ? "ON (1)" : "OFF (0)")}
                    <button
                      type="button"
                      class="btn accent"
                      disabled={busy}
                      onclick={() => write(index, on ? "1" : "0")}
                      >{label}</button
                    >
                  {/each}
                {:else if reg.enum.length && !custom}
                  <Select
                    bind:value={draft}
                    label="Value for {reg.name}"
                    placeholder="Choose…"
                    options={[
                      ...reg.enum.map(([v, l]) => ({
                        value: l,
                        label: `${l} (${v})`,
                      })),
                      { value: CUSTOM, label: "Custom value…" },
                    ]}
                    onchange={(v) => {
                      if (v !== CUSTOM) return;
                      custom = true;
                      draft = "";
                    }}
                  />
                  <button class="btn accent" disabled={busy}>Write</button>
                {:else}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="input num"
                    bind:value={draft}
                    autofocus
                    spellcheck="false"
                    aria-label="New value"
                  />
                  {#if reg.unit}<span class="dim">{reg.unit}</span>{/if}
                  <button class="btn accent" disabled={busy}>Write</button>
                  {#if reg.enum.length}
                    <button
                      type="button"
                      class="btn ghost"
                      title="Pick one of the map's values again"
                      onclick={() => {
                        custom = false;
                        draft = "";
                      }}>Options</button
                    >
                  {/if}
                {/if}
                <button
                  type="button"
                  class="btn"
                  onclick={() => (editing = null)}>Cancel</button
                >
                {#if note?.index === index && !note.ok}<span class="bad"
                    >{note.text}</span
                  >{/if}
              </form>
            </td>
          </tr>
        {:else if note?.index === index && note.ok}
          <tr class="done"
            ><td colspan="2"></td><td colspan={snap ? 7 : 6}>{note.text}</td
            ></tr
          >
        {/if}
      {:else}
        <tr>
          <td colspan={snap ? 9 : 8} class="empty"
            >{open.map.registers.length
              ? "No registers match this search."
              : "This map has no registers."}</td
          >
        </tr>
      {/each}
      <tr class="add-row">
        <td colspan={snap ? 9 : 8}>
          {#if adding}
            <CustomRegisterForm {open} onclose={() => (adding = false)} />
          {:else if computing}
            <ComputedForm {open} onclose={() => (computing = false)} />
          {:else}
            <button class="btn ghost small" onclick={() => (adding = true)}
              >Add a register the map is missing…</button
            >
            <button class="btn ghost small" onclick={() => (computing = true)}
              >Add a computed value…</button
            >
          {/if}
        </td>
      </tr>
    </tbody>
  </table>
</div>

<style>
  tr.diff td {
    background: rgb(var(--fg-rgb) / 0.05);
  }

  tr.diff td:first-child {
    box-shadow: inset 3px 0 0 var(--text);
  }

  .snapcell {
    color: var(--text-2);
  }

  .ne {
    margin-right: 6px;
    font-weight: 700;
    color: var(--text);
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 10px;
    padding-bottom: 10px;
  }

  /* Title and description share the toolbar row; the description gives way */
  /* Room for the name first; when the window is narrow the title takes a
     line of its own (the toolbar wraps) instead of running under the
     buttons. */
  .title {
    flex: 1 1 280px;
    min-width: 0;
    overflow: hidden;
    display: flex;
    align-items: baseline;
    gap: 10px;
    white-space: nowrap;
  }

  .title strong {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title .desc {
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title strong {
    font-size: 15px;
  }

  .desc,
  .dim {
    color: var(--text-3);
  }

  .search {
    width: 220px;
  }

  .note {
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
  }

  .rec {
    color: rgb(var(--alert-rgb));
  }

  .warnings {
    margin: 0 0 8px;
    font-size: 13px;
  }

  .rev {
    margin-left: 8px;
    font-size: 12px;
    color: var(--text-3);
  }

  .refs.ok {
    background: rgb(var(--rx-rgb) / 0.1);
    color: var(--ok);
  }

  .mismatch {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 4px 12px;
    margin: 0 0 8px;
    padding: 8px 10px;
    border-radius: 9px;
    background: rgb(var(--alert-rgb) / 0.12);
    color: rgb(var(--alert-rgb));
    font-size: 13px;
  }

  .mismatch .dim {
    color: var(--text-2);
  }

  .deprecated {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
    margin: 0 0 8px;
    padding: 8px 10px;
    border-radius: 9px;
    background: var(--subtle);
    font-size: 13px;
    color: var(--text-2);
  }

  .deprecated .error {
    color: rgb(var(--alert-rgb));
  }

  .warnings summary {
    width: fit-content;
    color: var(--warn);
    cursor: pointer;
  }

  .warnings ul {
    margin: 6px 0 0;
    padding: 6px 10px;
    list-style: none;
    max-height: 120px;
    overflow: auto;
    border-radius: 9px;
    background: var(--subtle);
    color: var(--text-2);
  }

  .warnings .where {
    display: inline-block;
    min-width: 64px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-3);
  }

  .table-wrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-radius: 10px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--card);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: left;
    padding: 6px 10px;
  }

  td {
    padding: 3px 10px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  tbody tr:hover td {
    background: var(--accent-soft);
  }

  .mono {
    font-family: var(--mono);
    font-size: 13px;
  }

  table {
    table-layout: fixed;
  }

  .addr {
    width: 104px;
  }

  th:nth-child(3) {
    width: 30%;
  }

  th:nth-child(4) {
    width: 150px;
  }

  th.val {
    width: 170px;
  }

  th:nth-child(6) {
    width: 64px;
  }

  /* Raw takes whatever width is left, so values sit next to their names */
  th:nth-child(7) {
    width: auto;
  }

  .read {
    width: 52px;
    text-align: center;
  }

  /* Registers that are not read: greyed out, at the end of the list */
  tr.off td:not(.read) {
    opacity: 0.4;
  }

  .refs {
    font-size: 12px;
    font-weight: 500;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--field);
    color: var(--text-2);
    white-space: nowrap;
  }

  .refs.bad {
    background: rgb(var(--alert-rgb) / 0.12);
    color: rgb(var(--alert-rgb));
  }

  .refmark {
    margin-left: 8px;
    padding: 0 5px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    background: var(--field);
    color: var(--text-3);
  }

  .refmark.ok {
    background: var(--ok);
    color: rgb(var(--surface-rgb));
  }

  .refmark.bad {
    background: rgb(var(--alert-rgb));
    color: rgb(var(--alert-fg-rgb));
  }

  .count {
    color: var(--text-2);
    font-size: 13px;
    white-space: nowrap;
  }

  .area {
    display: inline-block;
    width: 24px;
    margin-right: 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
  }

  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .grp {
    margin-left: 8px;
    font-weight: 400;
    font-size: 12px;
    color: var(--text-2);
  }

  .type {
    color: var(--text-2);
    font-size: 12px;
  }

  th.val {
    text-align: right;
  }

  td.val {
    text-align: right;
    font-size: 14px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    /* Long enum labels are cut here (full text in the tooltip) instead of
       running over the unit and raw columns */
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background-color 0.6s var(--ease);
  }

  /* Lit at once when the value changes, then fades back */
  td.val.alarm {
    color: var(--bad);
    box-shadow: inset 3px 0 0 rgb(var(--alert-rgb));
  }

  button.alarms {
    border: 0;
    font: inherit;
    cursor: pointer;
  }

  td.val.changed {
    background: var(--changed);
    transition: none;
  }

  .unit {
    color: var(--text-2);
    width: 60px;
  }

  .raw {
    color: var(--text-3);
    font-size: 12px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .act {
    width: 80px;
    text-align: right;
  }

  .small {
    height: 24px;
    padding: 0 8px;
    font-size: 12px;
  }

  .bad {
    color: var(--bad);
  }

  .sentinel {
    font-family: var(--ui);
    font-size: 13px;
    font-weight: 400;
    font-style: italic;
    color: var(--text-3);
  }

  .grp.custom {
    font-style: italic;
  }

  .add-row td {
    padding-top: 8px;
    border-bottom: 0;
  }

  .editor td {
    background: var(--accent-soft);
  }

  .editor form {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .editor .input {
    width: 160px;
  }

  .done td {
    color: var(--ok);
    font-weight: 600;
  }

  .empty {
    color: var(--text-2);
    padding: 16px 10px;
  }
</style>
