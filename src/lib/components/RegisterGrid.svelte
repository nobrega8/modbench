<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import Select from "./Select.svelte";
  import { areaAccess, formatAddress, hex } from "#lib/format.ts";
  import { slidePill } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import AddressFormat from "./AddressFormat.svelte";
  import SnapshotBar from "./SnapshotBar.svelte";
  import { snapWord } from "#lib/snapshots.ts";
  import { bench } from "#lib/state.svelte.ts";
  import {
    contextMenu,
    copy,
    type MenuEntry,
  } from "#lib/contextmenu.svelte.ts";
  import type { CopyForm } from "#lib/manual-chart.ts";
  import type { ByteOrder } from "#lib/types.ts";
  import {
    chartType,
    copyForms,
    decodePair,
    type GridFormat as Format,
  } from "#lib/manual-chart.ts";
  import { MAX_SERIES } from "#lib/trend.ts";

  const formats: { id: Format; label: string }[] = $derived([
    { id: "unsigned", label: t("datadecodertool.unsigned") },
    { id: "signed", label: t("datadecodertool.signed") },
    { id: "hex", label: t("registergrid.hexadecimal") },
    { id: "binary", label: t("datadecodertool.binary") },
    { id: "ascii", label: "ASCII" },
    { id: "int32", label: t("registergrid.32_bit_signed") },
    { id: "uint32", label: t("registergrid.32_bit_unsigned") },
    { id: "float32", label: t("registergrid.32_bit_float") },
    { id: "bits", label: t("registergrid.bits") },
  ]);
  const orders: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const PER_ROW = 10;

  // Kept on the bench: the Chart tab reads addresses the same way.
  const format = $derived(bench.gridFormat);
  const order = $derived(bench.gridOrder);

  const wide = $derived(
    format === "int32" || format === "uint32" || format === "float32",
  );
  const read = $derived(bench.read);
  const bitView = $derived(
    format === "bits" && read?.data.kind === "registers",
  );
  /** Bits of holding registers can be switched one at a time. */
  const writable = $derived(read?.area === "holding" && !settings.readOnly);
  let bitError = $state<string | null>(null);
  let writing = $state<string | null>(null);

  const access = $derived(
    read ? areaAccess(read.area, settings.readOnly) : null,
  );
  const list = $derived(settings.registerLayout === "list");
  /** The value clicked last (its offset from the first address). */
  let selected = $state<number | null>(null);
  // A new range of addresses has nothing selected
  const range = $derived(read ? `${read.area}:${read.address}` : "");
  $effect(() => {
    void range;
    selected = null;
  });

  /** The read snapshot to compare with, when it covers this area. */
  const snap = $derived(
    bench.snapshot?.read && read && bench.snapshot.read.area === read.area
      ? bench.snapshot
      : null,
  );
  /** The words now: the live read, or the second snapshot. */
  function nowWord(address: number): number | undefined {
    if (bench.compareWith)
      return snapWord(bench.compareWith, read!.area, address);
    if (!read) return undefined;
    const v = read.data.values[address - read.address];
    return v === undefined ? undefined : Number(v);
  }
  /** The words of a cell in the snapshot, or undefined where it has none. */
  function snapWords(c: Cell): number[] | undefined {
    if (!snap || !read) return undefined;
    const out: number[] = [];
    for (let k = 0; k < c.span; k++) {
      const w = snapWord(snap, read.area, read.address + c.offset + k);
      if (w === undefined) return undefined;
      out.push(w);
    }
    return out;
  }
  /** What a snapshot cell says, in the format shown. */
  function snapCellText(c: Cell): string {
    const w = snapWords(c);
    if (!w) return "—";
    if (read?.data.kind === "bits") return String(w[0]);
    return c.span === 2 ? pair(w[0], w[1]) : word(w[0]);
  }
  /** Selects a value and gives it the focus, for the arrow keys. */
  function select(offset: number, e: MouseEvent) {
    selected = offset;
    (e.currentTarget as HTMLElement).focus();
  }

  function setLayout(layout: "list" | "grid") {
    settings.registerLayout = layout;
    settings.save();
  }

  /** The raw words behind a value, in hex. */
  function rawHex(c: Cell): string {
    if (!read || read.data.kind === "bits") return "";
    return read.data.values
      .slice(c.offset, c.offset + c.span)
      .map((w) => w.toString(16).toUpperCase().padStart(4, "0"))
      .join(" ");
  }

  /** Arrow keys move the selection. */
  function onKey(e: KeyboardEvent) {
    if (selected === null) return;
    const i = cells.findIndex((c) => c.offset === selected);
    const step =
      e.key === "ArrowDown"
        ? list
          ? 1
          : PER_ROW / (wide ? 2 : 1)
        : e.key === "ArrowUp"
          ? list
            ? -1
            : -PER_ROW / (wide ? 2 : 1)
          : e.key === "ArrowRight" && !list
            ? 1
            : e.key === "ArrowLeft" && !list
              ? -1
              : 0;
    const next = cells[i + step];
    if (!step || !next) return;
    e.preventDefault();
    selected = next.offset;
    document
      .querySelector<HTMLElement>(`[data-offset="${next.offset}"]`)
      ?.focus();
  }

  async function flip(address: number, bit: number, on: boolean) {
    bitError = null;
    writing = `${address}:${bit}`;
    try {
      await bench.writeBit(address, bit, on);
    } catch (e) {
      bitError = e instanceof Error ? e.message : String(e);
    } finally {
      writing = null;
    }
  }

  function addr(n: number) {
    return formatAddress(n, settings.addressFormat, read?.area ?? null, true);
  }

  function word(w: number): string {
    switch (format) {
      case "signed":
        return String(w > 0x7fff ? w - 0x10000 : w);
      case "hex":
        return hex(w);
      case "binary":
        return w
          .toString(2)
          .padStart(16, "0")
          .replace(/(.{4})(?!$)/g, "$1 ");
      case "ascii":
        return [w >> 8, w & 0xff]
          .map((c) => (c >= 32 && c < 127 ? String.fromCharCode(c) : "·"))
          .join("");
      default:
        return String(w);
    }
  }

  function pair(a: number, b: number): string {
    const type = format === "int32" || format === "uint32" ? format : "float32";
    return String(decodePair(a, b, type, order));
  }

  interface Cell {
    offset: number;
    text: string;
    span: number;
    changed: boolean;
  }

  /** Cells in display order: one per register, or one per pair for 32-bit. */
  const cells = $derived.by((): Cell[] => {
    if (!read) return [];
    const fresh = (i: number) => bench.fresh(`g:${i}`);
    const out: Cell[] = [];
    if (read.data.kind === "bits") {
      read.data.values.forEach((b, i) =>
        out.push({
          offset: i,
          text: b ? "1" : "0",
          span: 1,
          changed: fresh(i),
        }),
      );
      return out;
    }
    const v = read.data.values;
    if (wide) {
      for (let i = 0; i + 1 < v.length; i += 2) {
        out.push({
          offset: i,
          text: pair(v[i], v[i + 1]),
          span: 2,
          changed: fresh(i) || fresh(i + 1),
        });
      }
      return out;
    }
    v.forEach((w, i) =>
      out.push({
        offset: i,
        text: word(w),
        span: 1,
        changed: fresh(i),
      }),
    );
    return out;
  });

  /** Sets up the Write tab to write these values from `address`. */
  function writeFrom(address: number, values: number[]) {
    const bits = read?.area === "coil";
    bench.address = address;
    bench.fc = bits ? (values.length > 1 ? 15 : 5) : values.length > 1 ? 16 : 6;
    bench.setQuantity(values.length);
    bench.writeValues = values;
    bench.tab = "write";
  }

  /** The name of a way of copying a word, as the grid's formats are named. */
  function formatLabel(id: CopyForm["id"]): string {
    return formats.find((f) => f.id === id)?.label ?? id;
  }

  /** What a right-click on a value offers. */
  function cellMenu(c: Cell): MenuEntry[] {
    if (!read) return [];
    const r = read;
    const at = r.address + c.offset;
    const raw =
      r.data.kind === "bits"
        ? [r.data.values[c.offset] ? 1 : 0]
        : r.data.values.slice(c.offset, c.offset + c.span);
    const writable = r.area === "holding" || r.area === "coil";
    const type = chartType(format, r.area);
    const series = type && {
      area: r.area,
      address: at,
      type,
      order,
    };
    const charted = series ? bench.manualSeriesOf(series) : null;
    const full = bench.manualSeries.length >= MAX_SERIES;
    return [
      series &&
        (charted
          ? {
              label: t("registergrid.remove_from_chart"),
              action: () => bench.removeManualFromChart(charted.slot),
            }
          : {
              label: t("registergrid.add_to_chart"),
              disabled: full,
              hint: full
                ? t("registergrid.max_at_most", { max: MAX_SERIES })
                : undefined,
              action: () => bench.addManualToChart(series),
            }),
      null,
      { label: t("registergrid.copy_value"), action: () => copy(c.text) },
      ...(r.data.kind === "registers"
        ? copyForms(r.data.values, c.offset, c.span === 2 ? 2 : 1, order).map(
            (f) => ({
              label: t("registergrid.copy_as", { format: formatLabel(f.id) }),
              hint: f.text,
              action: () => copy(f.text),
            }),
          )
        : []),
      { label: t("registergrid.copy_address"), action: () => copy(addr(at)) },
      {
        label: t("registergrid.copy_all_values"),
        hint: "address, value",
        action: () =>
          copy(
            cells
              .map((x) => `${addr(r.address + x.offset)}\t${x.text}`)
              .join("\n"),
          ),
      },
      null,
      writable && {
        label: t("registergrid.write_here"),
        disabled: settings.readOnly,
        action: () => writeFrom(at, raw),
      },
      writable && {
        label: t("registergrid.write_all"),
        disabled: settings.readOnly,
        action: () => bench.copyReadToWrite(),
      },
    ];
  }

  const selectedCell = $derived(
    selected === null
      ? null
      : (cells.find((c) => c.offset === selected) ?? null),
  );

  /** Offsets of the cells whose words differ from the snapshot. */
  const different = $derived.by(() => {
    const out = new Set<number>();
    if (!snap || !read) return out;
    for (const c of cells) {
      const before = snapWords(c);
      if (!before) continue;
      for (let k = 0; k < c.span; k++) {
        const now = nowWord(read.address + c.offset + k);
        if (now !== undefined && now !== before[k]) {
          out.add(c.offset);
          break;
        }
      }
    }
    return out;
  });
  const listCells = $derived(
    snap && bench.onlyDifferences
      ? cells.filter((c) => different.has(c.offset))
      : cells,
  );

  const rows = $derived.by(() => {
    const out: Cell[][] = [];
    for (const c of cells) (out[Math.floor(c.offset / PER_ROW)] ??= []).push(c);
    return out;
  });
</script>

<div class="toolbar">
  {#if read?.data.kind !== "bits"}
    <label class="inline">
      <span>{t("registergrid.show_as")}</span>
      <Select
        bind:value={bench.gridFormat}
        label={t("registergrid.show_as")}
        options={formats.map((f) => ({ value: f.id, label: f.label }))}
      />
    </label>
    {#if wide}
      <label class="inline">
        <span>{t("req.byteOrder")}</span>
        <Select
          bind:value={bench.gridOrder}
          label={t("req.byteOrder")}
          options={orders.map((o) => ({ value: o, label: o }))}
        />
      </label>
    {/if}
  {/if}
  <AddressFormat />
  {#if !bitView}
    <div
      class="segmented"
      role="tablist"
      aria-label={t("registergrid.layout")}
      use:slidePill
    >
      <button
        role="tab"
        aria-selected={list}
        class:on={list}
        title={t("registergrid.one_row_per_value")}
        onclick={() => setLayout("list")}>{t("registergrid.list")}</button
      >
      <button
        role="tab"
        aria-selected={!list}
        class:on={!list}
        title={t("registergrid.ten_values_per_row")}
        onclick={() => setLayout("grid")}>{t("registergrid.grid")}</button
      >
    </div>
  {/if}
  <span class="spacer"></span>
  <SnapshotBar
    kind="read"
    canTake={!!read}
    differences={snap ? different.size : null}
  />
  <button class="btn" disabled={!read} onclick={() => bench.copyReadToWrite()}
    >{t("registergrid.copy_data_to_write")}</button
  >
</div>

{#if read && access}
  <div class="status">
    <span class="access" class:writable={access.writable}
      ><span class="glyph" aria-hidden="true"
        >{access.writable ? "✎" : "🔒︎"}</span
      >{access.text}</span
    >
    {#if selectedCell}
      <span class="picked" aria-live="polite"
        >{t("registergrid.selected")}
        <strong class="mono">{addr(read.address + selectedCell.offset)}</strong>
        = <strong class="mono">{selectedCell.text}</strong></span
      >
    {/if}
  </div>
{/if}

{#if bitView && read?.data.kind === "registers"}
  {@const r = read}
  <p class="bit-hint">
    {writable
      ? t("registergrid.click_a_bit_to")
      : read.area === "holding"
        ? t("registergrid.read_only_mode_is")
        : t("registergrid.input_registers_cannot_be")}
    {#if bitError}<span class="bad">{bitError}</span>{/if}
  </p>
  <div class="grid-wrap">
    <div class="bits">
      <span></span>
      {#each Array(16) as _, k (k)}<span class="colhead">{15 - k}</span>{/each}
      <span class="colhead">{t("registergrid.hex")}</span>
      {#each read.data.values as w, i (i)}
        <span class="rowhead">{addr(r.address + i)}</span>
        {#each Array(16) as _, k (k)}
          {@const bit = 15 - k}
          {@const on = ((w >> bit) & 1) === 1}
          <button
            class="bit"
            class:on
            class:nibble={bit % 4 === 0 && bit > 0}
            class:busy={writing === `${r.address + i}:${bit}`}
            disabled={!writable || writing !== null}
            title={t("registergrid.bit_bit_of_address2", {
              bit,
              address: addr(r.address + i),
              value: on ? 1 : 0,
            })}
            aria-label={t("registergrid.bit_bit_of_address", {
              bit,
              address: addr(r.address + i),
              state: on ? t("registergrid.on") : t("registergrid.off"),
            })}
            onclick={() => flip(r.address + i, bit, !on)}>{on ? 1 : 0}</button
          >
        {/each}
        <span
          class="cell hexcell"
          role="gridcell"
          tabindex="-1"
          oncontextmenu={(e) =>
            contextMenu.show(e, [
              {
                label: t("registergrid.copy_value"),
                hint: t("registergrid.hex2"),
                action: () => copy(hex(w)),
              },
              {
                label: t("registergrid.copy_value"),
                hint: t("registergrid.decimal"),
                action: () => copy(String(w)),
              },
              {
                label: t("registergrid.copy_address"),
                action: () => copy(addr(r.address + i)),
              },
            ])}>{hex(w)}</span
        >
      {/each}
    </div>
  </div>
{:else if !read}
  <p class="empty">
    {t("registergrid.values_read_from_the")}
  </p>
{:else if list}
  {@const r = read}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="grid-wrap" role="presentation" onkeydown={onKey}>
    <table class="regs" class:writable={access?.writable}>
      <thead>
        <tr>
          <th class="addr">{t("req.address")}</th>
          <th class="val">{t("maptable.value")}</th>
          {#if r.data.kind === "registers"}<th class="hex"
              >{t("maptable.raw")}</th
            >{/if}
          {#if snap}<th class="val">{t("maptable.snapshot")}</th>{/if}
        </tr>
      </thead>
      <tbody>
        {#each listCells as c (c.offset)}
          <tr
            class:diff={different.has(c.offset)}
            class:selected={selected === c.offset}
            aria-selected={selected === c.offset}
            data-offset={c.offset}
            tabindex="-1"
            onclick={(e) => select(c.offset, e)}
            oncontextmenu={(e) => {
              selected = c.offset;
              contextMenu.show(e, cellMenu(c));
            }}
          >
            <td class="addr mono"
              ><span class="mark" aria-hidden="true"
                >{selected === c.offset ? "▸" : ""}</span
              >{addr(r.address + c.offset)}{#if c.span > 1}<span class="dim"
                  >–{addr(r.address + c.offset + c.span - 1)}</span
                >{/if}</td
            >
            <td class="val"
              ><span class="cell" class:changed={c.changed}>{c.text}</span></td
            >
            {#if r.data.kind === "registers"}<td class="hex mono"
                >{rawHex(c)}</td
              >{/if}
            {#if snap}<td class="snapcell mono"
                >{#if different.has(c.offset)}<span class="ne">≠</span
                  >{/if}{snapCellText(c)}</td
              >{/if}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
{:else}
  {@const r = read}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="grid-wrap" role="presentation" onkeydown={onKey}>
    <div class="grid" class:writable={access?.writable}>
      <span class="corner"></span>
      {#each Array(PER_ROW) as _, i (i)}<span class="colhead">+{i}</span>{/each}
      {#each rows as row, ri (ri)}
        <span class="rowhead">{addr(r.address + ri * PER_ROW)}</span>
        {#each row as c (c.offset)}
          <span
            class="cell"
            class:changed={c.changed}
            class:selected={selected === c.offset}
            class:diff={different.has(c.offset)}
            style:grid-column="span {c.span}"
            role="gridcell"
            tabindex="-1"
            data-offset={c.offset}
            aria-selected={selected === c.offset}
            onclick={(e) => select(c.offset, e)}
            onkeydown={(e) => {
              if (e.key === "Enter" || e.key === " ") selected = c.offset;
            }}
            oncontextmenu={(e) => {
              selected = c.offset;
              contextMenu.show(e, cellMenu(c));
            }}
            title={t("registergrid.address_address_snapshot", {
              address: addr(r.address + c.offset),
              snapshot: different.has(c.offset)
                ? t("registergrid.snapshot_text", { text: snapCellText(c) })
                : "",
            })}>{c.text}</span
          >
        {/each}
        {#if row.length * row[0].span < PER_ROW}
          <span style:grid-column="span {PER_ROW - row.length * row[0].span}"
          ></span>
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    padding-bottom: 10px;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }

  .spacer {
    flex: 1;
  }

  .empty {
    color: var(--text-2);
    margin: 8px 0;
  }

  .grid-wrap {
    overflow: auto;
    /* Some values stay visible in a short window; the page scrolls */
    min-height: 120px;
    flex: 1;
  }

  .grid .cell.diff {
    box-shadow: inset 0 -3px 0 var(--text);
  }

  .regs tr.diff td.addr {
    box-shadow: inset 3px 0 0 var(--text);
  }

  .snapcell {
    font-size: 12px;
    color: var(--text-2);
  }

  .ne {
    margin-right: 6px;
    font-weight: 700;
    color: var(--text);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    padding-bottom: 8px;
    font-size: 12px;
    color: var(--text-2);
  }

  .access {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .access .glyph {
    font-size: 11px;
  }

  .picked strong {
    color: var(--text);
  }

  .grid {
    display: grid;
    grid-template-columns: 58px repeat(10, minmax(64px, 1fr));
    gap: 3px;
  }

  /* Headers stay in view while the values scroll */
  .colhead,
  .corner {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 2px 0;
    background: var(--card);
    font-size: 11px;
    color: var(--text-3);
    text-align: center;
  }

  .corner {
    left: 0;
    z-index: 2;
  }

  .rowhead {
    position: sticky;
    left: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 6px;
    background: var(--card);
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
  }

  /* Writable values look like fields (an outline); read-only ones do not */
  .writable .cell {
    box-shadow: inset 0 0 0 1px var(--border-strong);
  }

  .cell {
    cursor: default;
  }

  .grid .cell.selected,
  .regs tr.selected .cell {
    outline: 2px solid var(--text);
    outline-offset: 1px;
  }

  .cell:focus-visible {
    outline: 2px solid var(--text);
  }

  table.regs {
    border-collapse: separate;
    border-spacing: 0 2px;
    min-width: 280px;
  }

  .regs th {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 2px 10px;
    background: var(--card);
    font-size: 11px;
    font-weight: 500;
    color: var(--text-3);
    text-align: left;
  }

  .regs td {
    padding: 0 10px;
    white-space: nowrap;
  }

  .regs tr {
    cursor: default;
  }

  .regs tr:focus,
  .grid .cell:focus {
    outline: none;
  }

  .regs tr:focus-visible .cell {
    outline: 2px solid var(--text);
  }

  .regs td.addr {
    font-size: 12px;
    color: var(--text-2);
    text-align: right;
  }

  .regs tr.selected td.addr {
    color: var(--text);
    font-weight: 600;
  }

  .mark {
    display: inline-block;
    width: 12px;
    text-align: left;
  }

  .regs td.val .cell {
    display: inline-block;
    min-width: 120px;
    padding-block: 2px;
  }

  .regs td.hex {
    font-size: 12px;
    color: var(--text-3);
  }

  .cell {
    padding: 4px 6px;
    background: var(--cell);
    border-radius: 3px;
    font-family: var(--mono);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }

  /* Lit at once when the value changes, then fades back */
  .cell {
    transition: background-color 0.6s var(--ease);
  }

  .bit-hint {
    margin: 0 0 8px;
    color: var(--text-2);
    font-size: 12px;
  }

  .bit-hint .bad {
    margin-left: 8px;
    color: var(--bad);
  }

  .bits {
    display: grid;
    grid-template-columns: 58px repeat(16, 26px) 64px;
    gap: 3px;
    align-items: center;
  }

  .bit {
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: var(--cell);
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-3);
    cursor: pointer;
  }

  .bit.nibble {
    margin-right: 5px;
  }

  .bit.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  .bit:disabled {
    cursor: default;
  }

  .bit.busy {
    opacity: 0.5;
  }

  .bit:hover:not(:disabled) {
    box-shadow: inset 0 0 0 1px var(--text-3);
  }

  .hexcell {
    margin-left: 6px;
  }

  .cell.changed {
    background: var(--changed);
    transition: none;
  }
</style>
