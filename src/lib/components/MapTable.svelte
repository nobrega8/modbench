<script lang="ts">
  import { untrack } from "svelte";
  import { windowOf } from "#lib/virtual.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import { localizeError } from "#lib/errors.ts";
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { valuesCsv } from "#lib/csv.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import {
    AREA_TAG,
    AREA_NAME,
    READ_FUNCTION,
    otherReadArea,
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
  import {
    diagnosticWhere,
    type CheckReport,
    type Register,
    type ReportContext,
  } from "#lib/types.ts";
  import { isCustom } from "#lib/custom-registers.ts";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";
  import { registerId } from "#lib/profiles.ts";
  import { markName, viewName } from "#lib/highlight-names.ts";
  import { MARKS, MARKS_VIEWS, compareRows, shown } from "#lib/highlights.ts";
  import { MAX_SERIES } from "#lib/trend.ts";
  import AddressFormat from "./AddressFormat.svelte";
  import Icon from "./Icon.svelte";
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
      .filter(({ index }) => shown(index, bench.marks, bench.marksView))
      // Registers that are not read go to the end, in map order; the
      // highlighted ones come first when asked.
      .sort((a, b) =>
        compareRows(a, b, bench.skipped, bench.marks, bench.marksView),
      ),
  );
  // Long maps render only the rows in view (see virtual.ts).
  let wrap = $state<HTMLElement>();
  let scrollTop = $state(0);
  let viewport = $state(600);
  let rowHeight = $state(29);
  const win = $derived(windowOf(rows.length, scrollTop, viewport, rowHeight));
  const visible = $derived(rows.slice(win.start, win.end));
  // The height of a row is the average of the ones shown. Rows differ in
  // height (a long name or label wraps), so measuring just the first one made
  // the window and the measurement chase each other; the average and a margin
  // of a pixel and a half settle.
  $effect(() => {
    void visible;
    const rows = wrap?.querySelectorAll<HTMLElement>("tbody tr.reg");
    if (!rows?.length) return;
    let sum = 0;
    for (const row of rows) sum += row.getBoundingClientRect().height;
    const h = sum / rows.length;
    if (h > 0 && Math.abs(h - untrack(() => rowHeight)) > 1.5) rowHeight = h;
  });
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
      exportNote = { ok: true, text: t("maptable.saved") };
    } catch (e) {
      exportNote = {
        ok: false,
        text: t("maptable.not_saved_error", { error: errorText(e) }),
      };
    }
  }

  /** The last check of the device against this map. */
  let checking = $state(false);
  let checked = $state<{
    path: string;
    report: CheckReport;
    context: ReportContext;
  } | null>(null);

  async function checkDevice() {
    exportNote = null;
    checking = true;
    try {
      const { report, context } = await bench.checkDevice();
      checked = { path: open.path, report, context };
    } catch (e) {
      exportNote = {
        ok: false,
        text: t("check.failed", { error: errorText(e) }),
      };
    } finally {
      checking = false;
    }
  }

  async function saveReport() {
    if (!checked) return;
    exportNote = null;
    const name = `${meta.vendor}-${meta.model}`.replace(/[^\w.-]+/g, "_");
    const path = await save({
      defaultPath: `${name}-commissioning.html`,
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!path) return;
    try {
      await api.saveText(
        path,
        await api.checkHtml(checked.report, checked.context),
      );
      exportNote = { ok: true, text: t("check.saved") };
    } catch (e) {
      exportNote = {
        ok: false,
        text: t("check.not_saved", { error: errorText(e) }),
      };
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

  /** The function code a register is read with, and the map's own one. */
  function readFc(reg: Register, index: number) {
    const area = bench.readAs.get(index) ?? reg.area;
    return { fc: READ_FUNCTION[area], own: READ_FUNCTION[reg.area] };
  }

  const fcText = (n: number) => String(n).padStart(2, "0");
  const otherFc = (reg: Register) => READ_FUNCTION[otherReadArea(reg.area)];

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
        label: t("maptable.write"),
        disabled: settings.readOnly,
        hint: settings.readOnly ? t("maptable.read_only") : undefined,
        action: () => edit(index),
      },
      readable && {
        label: bench.skipped.has(index)
          ? t("maptable.read_it")
          : t("maptable.stop_reading_it"),
        action: () => bench.toggleRead(index),
      },
      ...MARKS.map((m) => ({
        label: t("maptable.highlight_with", { colour: markName(m) }),
        disabled: bench.marks.get(index) === m,
        action: () => bench.setMark(index, m),
      })),
      bench.marks.has(index) && {
        label: t("maptable.highlight_remove"),
        action: () => bench.setMark(index, null),
      },
      null,
      readable &&
        formulaOf(reg) === null &&
        (bench.readAs.has(index)
          ? {
              label: t("maptable.read_with_map_fc", {
                fc: fcText(readFc(reg, index).own),
              }),
              action: () => bench.setReadArea(index, null),
            }
          : {
              label: t("maptable.read_with_fc", {
                fc: fcText(otherFc(reg)),
                area: AREA_NAME[otherReadArea(reg.area)],
              }),
              action: () => bench.setReadArea(index, otherReadArea(reg.area)),
            }),
      !isCustom(reg) &&
        !bench.demo && {
          label: t("builder.edit_register"),
          action: () =>
            api
              .builderOpenWindow({
                path: open.path,
                register: {
                  area: reg.area,
                  address: reg.address,
                  name: reg.name,
                },
              })
              .catch(() => {}),
        },
      readable &&
        reg.type !== "string" &&
        (charted
          ? {
              label: t("maptable.remove_from_chart"),
              action: () => bench.removeFromChart(index),
            }
          : {
              label: t("maptable.add_to_chart"),
              disabled: bench.chartSeries.length >= MAX_SERIES,
              hint:
                bench.chartSeries.length >= MAX_SERIES
                  ? t("maptable.max_at_most", { max: MAX_SERIES })
                  : undefined,
              action: () => bench.addToChart(index),
            }),
      readable &&
        reg.type !== "string" && {
          label: alarms.has(index)
            ? t("maptable.change_alarm")
            : t("maptable.set_alarm"),
          action: () => alarmFor(reg),
        },
      null,
      shown !== null && {
        label: t("maptable.copy_value"),
        action: () => copy(reg.unit ? `${shown} ${reg.unit}` : shown),
      },
      r?.raw !== undefined &&
        r?.raw !== null &&
        !r.error && {
          label: t("maptable.copy_raw_value"),
          action: () => copy(formatRaw(r.raw ?? null)),
        },
      { label: t("maptable.copy_name"), action: () => copy(reg.name) },
      {
        label: t("maptable.copy_address"),
        action: () =>
          copy(formatAddress(reg.address, settings.addressFormat, reg.area)),
      },
      null,
      isCustom(reg) && {
        label: t("maptable.remove_register"),
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
    if (
      !(await bench.confirmWrite(
        t("maptable.write_value_to_name", { value: text, name: reg.name }),
      ))
    )
      return;
    busy = true;
    try {
      await bench.writeMapValue(index, parsed.value);
      note = {
        index,
        ok: true,
        text: t("maptable.wrote_value_to_name", {
          value: text,
          name: reg.name,
        }),
      };
      editing = null;
    } catch (e) {
      note = {
        index,
        ok: false,
        text: t("maptable.not_written_error", { error: errorText(e) }),
      };
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
        title={t("maptable.revision_revision_of_this", {
          revision: meta.revision,
          date: meta.revised ? `, ${meta.revised}` : "",
        })}>{t("maptable.rev_revision", { revision: meta.revision })}</span
      >{/if}
  </div>
  {#if status === "official"}
    <span class="refs" title={t("maptable.written_from_the_manufacturer")}
      >{t("maptable.official")}</span
    >
  {:else if status === "unofficial"}
    <span
      class="refs bad"
      title={t("maptable.converted_from_another_catalogue")}
      >{t("maptable.unofficial")}</span
    >
  {/if}
  {#if recognised && identity}
    <span
      class="refs ok"
      title={[
        identity.model.found &&
          t("maptable.model_model", { model: identity.model.found }),
        fwVersion &&
          t("maptable.firmware_version", { version: versionText(fwVersion) }),
        identity.serial &&
          t("maptable.serial_number_serial", { serial: identity.serial }),
      ]
        .filter(Boolean)
        .join(", ")}>{t("maptable.device_recognised")}</span
    >
  {:else if mapRange && !fwVersion}
    <span
      class="refs"
      title={t("maptable.this_map_is_for", { range: mapRange })}
      >{t("maptable.firmware_range", { range: mapRange })}</span
    >
  {/if}
  {#if refs.total}
    <span
      class="refs"
      class:bad={refs.mismatch > 0 || refs.failed > 0}
      title={t("maptable.registers_with_a_known")}
      >{t("maptable.reference_match_total", {
        match: refs.match,
        total: refs.total,
      })}{#if refs.mismatch}
        · {t("maptable.count_differ", {
          count: refs.mismatch,
        })}{/if}{#if refs.failed}
        · {t("maptable.count_failed", { count: refs.failed })}{/if}</span
    >
  {/if}
  {#if alarms.size}
    <button
      class="refs alarms"
      class:bad={inAlarm.size > 0}
      title={t("maptable.alarm_limits_set_on", { count: alarms.size })}
      onclick={() => {
        bench.tool = "alarms";
        bench.page = "tools";
      }}
      >{inAlarm.size
        ? t("maptable.count_in_alarm", { count: inAlarm.size })
        : t("maptable.alarms_count_ok", { count: alarms.size })}</button
    >
  {/if}
  <span class="count"
    >{t("maptable.reading_read_of_total", {
      read: readCount,
      total: open.map.registers.length,
    })}</span
  >
  <button
    class="btn icon"
    title={t("maptable.read_all")}
    aria-label={t("maptable.read_all")}
    disabled={readCount === open.map.registers.length}
    onclick={() => bench.setAllRead(true)}><Icon name="readall" /></button
  >
  <button
    class="btn icon"
    title={t("maptable.read_none")}
    aria-label={t("maptable.read_none")}
    disabled={readCount === 0}
    onclick={() => bench.setAllRead(false)}><Icon name="readnone" /></button
  >
  <button
    class="btn icon"
    title={t("maptable.save_the_values_shown")}
    aria-label={t("maptable.export")}
    disabled={!bench.mapResults.size}
    onclick={exportValues}><Icon name="export" /></button
  >
  <button
    class="btn icon"
    class:rec={recording}
    title={recording
      ? `${t("maptable.stop_recording_rows", { rows: recording.rows })}: ${t("maptable.recording_to_path_click", { path: recording.path })}`
      : t("maptable.add_one_row_per")}
    aria-label={recording
      ? t("maptable.stop_recording_rows", { rows: recording.rows })
      : t("maptable.record")}
    disabled={!recording && readCount === 0}
    onclick={toggleRecording}
    ><Icon name={recording ? "stoprecord" : "record"} /></button
  >
  <button
    class="btn icon"
    title={t("check.title")}
    aria-label={t("check.run")}
    disabled={checking || bench.running}
    onclick={checkDevice}><Icon name="verified" /></button
  >
  {#if bench.marks.size}
    <button
      class="btn icon"
      class:accent={bench.marksView !== "all"}
      title={`${t("maptable.marks_view")}: ${viewName(bench.marksView)}`}
      aria-label={t("maptable.marks_view")}
      aria-haspopup="menu"
      onclick={(e) =>
        contextMenu.show(
          e,
          MARKS_VIEWS.map((v) => ({
            label: viewName(v),
            hint: bench.marksView === v ? "✓" : undefined,
            disabled: bench.marksView === v,
            action: () => bench.setMarksView(v),
          })),
        )}><Icon name="bookmark" /></button
    >
  {/if}
  <SnapshotBar
    kind="map"
    canTake={bench.mapResults.size > 0}
    differences={snap ? different.size : null}
  />
  {#if exportNote}<span class:bad={!exportNote.ok} class="note"
      >{exportNote.text}</span
    >{/if}
  {#if bench.recordError}<span class="bad note" title={bench.recordError}
      >{t("maptable.recording_stopped_error", {
        error: bench.recordError,
      })}</span
    >{/if}
  <input
    class="input search"
    type="search"
    placeholder={t("maptable.search_name_or_address")}
    bind:value={search}
    aria-label={t("mapexplorer.search_registers")}
  />
  <AddressFormat />
  {#if groups.length}
    <Select
      bind:value={group}
      label={t("maptable.group")}
      options={[
        { value: "", label: "All groups" },
        ...groups.map((g) => ({ value: g, label: g })),
      ]}
    />
  {/if}
</div>

{#if mismatch && identity}
  <div class="mismatch" role="alert">
    <strong>{t("maptable.the_connected_device_does")}</strong>
    {#if identity.model.status === "mismatch"}<span
        >{t("maptable.the_map_is_for", {
          map: modelNames,
          found: identity.model.found ?? "",
        })}</span
      >{/if}
    {#if identity.firmware.status === "mismatch" && fwVersion}<span
        >{t("maptable.the_map_is_for2", {
          map: mapRange ?? "",
          found: versionText(fwVersion),
        })}</span
      >{/if}
    <span class="dim">{t("maptable.values_may_be_wrong")}</span>
  </div>
{/if}

{#if open.version === 1}
  <div class="deprecated">
    <span>{t("maptable.this_map_uses_format")}</span>
    <button class="btn" disabled={converting} onclick={convert}
      >{converting
        ? t("maptable.converting")
        : t("maptable.convert_to_version_2")}</button
    >
    {#if convertError}<span class="error">{convertError}</span>{/if}
  </div>
{/if}

{#if open.warnings.length}
  <details class="warnings">
    <summary
      >{t("maptable.warnings_in_the_map", {
        count: open.warnings.length,
      })}</summary
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

{#if checked && checked.path === open.path}
  {@const failures = [
    ...checked.report.findings
      .filter((f) => f.status === "fail")
      .map((f) => ({ name: f.what, why: f.detail })),
    ...checked.report.rows
      .filter((r) => r.status === "fail")
      .map((r) => ({ name: r.name, why: r.reason ?? "" })),
  ]}
  <div class="checked" class:bad={failures.length > 0}>
    <strong>
      {t("check.summary", {
        pass: checked.report.pass,
        fail: checked.report.fail,
        not_read: checked.report.not_read,
      })}
    </strong>
    <button class="btn small" onclick={saveReport}>{t("check.save")}</button>
    <button
      class="btn ghost small"
      aria-label={t("check.close")}
      onclick={() => (checked = null)}>×</button
    >
    {#if failures.length === 0}
      <span class="dim">{t("check.none")}</span>
    {:else}
      <ul>
        {#each failures.slice(0, 20) as f, i (i)}
          <li><strong>{f.name}</strong> {f.why}</li>
        {/each}
        {#if failures.length > 20}<li class="dim">
            … {failures.length - 20}
          </li>{/if}
      </ul>
    {/if}
  </div>
{/if}

<div
  class="table-wrap"
  bind:this={wrap}
  bind:clientHeight={viewport}
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
>
  <table>
    <thead>
      <tr>
        <th class="read">{t("maptable.read")}</th>
        <th class="addr">{t("req.address")}</th>
        <th class="fc" title={t("maptable.fc_title")}>{t("maptable.fc")}</th>
        <th>{t("computedform.name")}</th>
        <th>{t("customregisterform.type")}</th>
        <th class="val">{t("maptable.value")}</th>
        {#if snap}<th
            class="val"
            title={t("maptable.the_value_in_the", {
              label: snap.label,
              compared: bench.compareWith
                ? t("maptable.compared_with_label", {
                    label: bench.compareWith.label,
                  })
                : t("maptable.compared_with_the_live"),
            })}>{t("maptable.snapshot")}</th
          >{/if}
        <th>{t("computedform.unit")}</th>
        <th>{t("maptable.raw")}</th>
        <th class="act"></th>
      </tr>
    </thead>
    <tbody>
      {#if win.before}
        <tr class="spacer" aria-hidden="true" style:height="{win.before}px"
          ><td colspan={snap ? 9 : 8}></td></tr
        >
      {/if}
      {#each visible as { reg, index } (index)}
        {@const r = bench.mapResults.get(index)}
        {@const off = bench.skipped.has(index)}
        {@const shown =
          reg.access === "W" || r?.sentinel
            ? null
            : formatValue(reg, r?.value ?? null)}
        {@const shownTitle =
          shown === null ? undefined : valueTitle(reg, r?.value ?? null)}
        <tr
          class="reg"
          class:err={!!r?.error}
          class:off
          class:diff={different.has(index)}
          data-mark={bench.marks.get(index)}
          oncontextmenu={(e) => contextMenu.show(e, rowMenu(reg, index))}
        >
          <td class="read">
            {#if reg.access !== "W"}
              <input
                type="checkbox"
                class="switch"
                role="switch"
                checked={!off}
                aria-label={t("maptable.read_name", { name: reg.name })}
                title={off
                  ? t("maptable.not_read_switch_on")
                  : t("maptable.read")}
                onchange={() => bench.toggleRead(index)}
              />
            {/if}
          </td>
          <td
            class="addr mono"
            title={formulaOf(reg) !== null
              ? t("maptable.computed_on_this_computer")
              : t("maptable.area_address_on_the", {
                  area: AREA_NAME[reg.area],
                  address: reg.address,
                  manual: base
                    ? t("maptable.address_in_the_manual", {
                        address: reg.address + base,
                      })
                    : "",
                })}
            >{#if formulaOf(reg) !== null}<span class="area"
                >{t("maptable.calc")}</span
              >{:else}<span class="area">{AREA_TAG[reg.area]}</span
              >{formatAddress(
                reg.address,
                settings.addressFormat,
                reg.area,
              )}{/if}</td
          >
          <td class="fc mono">
            {#if reg.access === "W" || formulaOf(reg) !== null}<span class="dim"
                >–</span
              >{:else}
              {@const f = readFc(reg, index)}
              {#if f.fc === f.own}<span
                  title={t("maptable.fc_map", { fc: fcText(f.fc) })}
                  >{fcText(f.fc)}</span
                >{:else}<span
                  class="forced"
                  title={t("maptable.fc_forced", {
                    fc: fcText(f.fc),
                    map: fcText(f.own),
                  })}
                  aria-label={t("maptable.fc_forced", {
                    fc: fcText(f.fc),
                    map: fcText(f.own),
                  })}>{fcText(f.fc)}*</span
                >{/if}
            {/if}
          </td>
          <td class="name" title={reg.description || undefined}
            >{#if bench.marks.has(index)}<span
                class="hldot"
                data-mark={bench.marks.get(index)}
                title={t("maptable.highlighted")}
                role="img"
                aria-label={t("maptable.highlighted")}
              ></span>{/if}{reg.name}{#if reg.group && !group}<span class="grp"
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
                title={t("maptable.added_on_this_computer")}
                >{t("maptable.custom")}</span
              >{/if}{#if reg.reference}<span
                class="refmark"
                class:ok={r?.reference === "match"}
                class:bad={r?.reference === "mismatch"}
                title={r?.reference === "match"
                  ? t("maptable.matches_the_reference_value", {
                      value: reg.reference,
                    })
                  : r?.reference === "mismatch"
                    ? t("maptable.differs_from_the_reference", {
                        value: reg.reference,
                      })
                    : t("maptable.reference_value_value", {
                        value: reg.reference,
                      })}>{t("maptable.ref")}</span
              >{/if}</td
          >
          <td
            class="type mono"
            title={reg.scale_factor
              ? t("maptable.scaled_by_10_to", { name: reg.scale_factor })
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
            {#if reg.access === "W"}<span class="dim"
                >{t("maptable.write_only")}</span
              >{:else if r?.sentinel}<span
                class="sentinel"
                title={t("maptable.the_device_returned_a", {
                  value: r.sentinel,
                })}>{r.sentinel}</span
              >{:else}{shown}{/if}
          </td>
          {#if snap}
            {@const before = snapText(snap, reg)}
            <td
              class="val mono snapcell"
              title={different.has(index)
                ? `Snapshot: ${before}\n${bench.compareWith ? bench.compareWith.label : "Now"}: ${after(reg, index)}`
                : undefined}
              >{#if different.has(index)}<span
                  class="ne"
                  aria-label={t("maptable.differs")}>≠</span
                >{/if}{before ?? "—"}</td
            >
          {/if}
          <td class="unit">{r?.sentinel ? "" : reg.unit}</td>
          <td
            class="raw mono"
            title={r?.error ? localizeError(r.error) : undefined}
          >
            {#if r?.error}<span class="bad">{localizeError(r.error)}</span
              >{:else}{formatRaw(r?.raw ?? null)}{/if}
          </td>
          <td class="act">
            {#if reg.access !== "R"}
              <button class="btn small" onclick={() => edit(index)}
                >{t("maptable.write")}</button
              >
            {/if}
            {#if isCustom(reg)}
              <button
                class="btn ghost small"
                title={t("maptable.remove_this_register_you")}
                onclick={() => bench.removeCustomRegister(index)}
                >{t("maptable.remove")}</button
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
                <span
                  >{t("maptable.new_value_for")}
                  <strong>{reg.name}</strong></span
                >
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
                    label={t("maptable.value_for_name", { name: reg.name })}
                    placeholder={t("maptable.choose")}
                    options={[
                      ...reg.enum.map(([v, l]) => ({
                        value: l,
                        label: `${l} (${v})`,
                      })),
                      { value: CUSTOM, label: t("maptable.custom_value") },
                    ]}
                    onchange={(v) => {
                      if (v !== CUSTOM) return;
                      custom = true;
                      draft = "";
                    }}
                  />
                  <button class="btn accent" disabled={busy}
                    >{t("maptable.write2")}</button
                  >
                {:else}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="input num"
                    bind:value={draft}
                    autofocus
                    spellcheck="false"
                    aria-label={t("maptable.new_value")}
                  />
                  {#if reg.unit}<span class="dim">{reg.unit}</span>{/if}
                  <button class="btn accent" disabled={busy}
                    >{t("maptable.write2")}</button
                  >
                  {#if reg.enum.length}
                    <button
                      type="button"
                      class="btn ghost"
                      title={t("maptable.pick_one_of_the")}
                      onclick={() => {
                        custom = false;
                        draft = "";
                      }}>{t("maptable.options")}</button
                    >
                  {/if}
                {/if}
                <button
                  type="button"
                  class="btn"
                  onclick={() => (editing = null)}
                  >{t("computedform.cancel")}</button
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
              ? t("maptable.no_registers_match_this")
              : t("maptable.this_map_has_no")}</td
          >
        </tr>
      {/each}
      {#if win.after}
        <tr class="spacer" aria-hidden="true" style:height="{win.after}px"
          ><td colspan={snap ? 9 : 8}></td></tr
        >
      {/if}
      <tr class="add-row">
        <td colspan={snap ? 9 : 8}>
          {#if adding}
            <CustomRegisterForm {open} onclose={() => (adding = false)} />
          {:else if computing}
            <ComputedForm {open} onclose={() => (computing = false)} />
          {:else}
            <button class="btn ghost small" onclick={() => (adding = true)}
              >{t("maptable.add_a_register_the")}</button
            >
            <button class="btn ghost small" onclick={() => (computing = true)}
              >{t("maptable.add_a_computed_value")}</button
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

  /* A highlighted row: a tinted background and a bar on its left edge. */
  tr[data-mark="1"],
  .hldot[data-mark="1"] {
    --hl: var(--hl-1);
  }
  tr[data-mark="2"],
  .hldot[data-mark="2"] {
    --hl: var(--hl-2);
  }
  tr[data-mark="3"],
  .hldot[data-mark="3"] {
    --hl: var(--hl-3);
  }
  tr[data-mark="4"],
  .hldot[data-mark="4"] {
    --hl: var(--hl-4);
  }

  tr[data-mark] td {
    background: rgb(var(--hl) / 0.16);
  }

  tr[data-mark] td:first-child {
    box-shadow: inset 3px 0 0 rgb(var(--hl));
  }

  .hldot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 7px;
    border-radius: 50%;
    background: rgb(var(--hl));
    vertical-align: middle;
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

  .title strong {
    font-size: 15px;
  }

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

  .checked {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--card);
  }

  .checked.bad {
    outline: 1px solid var(--danger, #c0392b);
  }

  .checked ul {
    flex-basis: 100%;
    margin: 0;
    padding-left: 18px;
    max-height: 140px;
    overflow: auto;
    font-size: 13px;
  }

  tr.spacer td {
    padding: 0;
    border: 0;
  }

  tbody tr.spacer:hover td {
    background: none;
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

  .fc {
    width: 52px;
    text-align: center;
  }

  .fc .forced {
    color: var(--accent);
    font-weight: 700;
  }

  th:nth-child(4) {
    width: 30%;
  }

  th:nth-child(5) {
    width: 150px;
  }

  th.val {
    width: 170px;
  }

  th:nth-child(7) {
    width: 64px;
  }

  /* Raw takes whatever width is left, so values sit next to their names */
  th:nth-child(8) {
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
