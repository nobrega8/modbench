<!--
  The map builder: the map's header and a table of its registers, checked
  with the same rules as a file while typing. It lives in its own window (see
  BuilderWindow) and saves a version 2 .mbmap.json.
-->
<script lang="ts">
  import { tick } from "svelte";
  import { t } from "#lib/i18n.svelte.ts";
  import { windowOf } from "#lib/virtual.ts";
  import { builder } from "#lib/builder.svelte.ts";
  import {
    enumText,
    fileAddress,
    isBitArea,
    parseEnum,
    wireAddress,
    withArea,
  } from "#lib/builder.ts";
  import type {
    Access,
    Area,
    ByteOrder,
    DataType,
    Parity,
  } from "#lib/types.ts";
  import Icon from "./Icon.svelte";
  import MapMetaLists from "./MapMetaLists.svelte";
  import TableImport from "./TableImport.svelte";

  const AREAS: { value: Area; label: string }[] = $derived([
    { value: "holding", label: t("customregisterform.holding_register") },
    { value: "input", label: t("customregisterform.input_register") },
    { value: "coil", label: t("customregisterform.coil") },
    { value: "discrete", label: t("customregisterform.discrete_input") },
  ]);
  const TYPES: DataType[] = [
    "uint16",
    "int16",
    "uint32",
    "int32",
    "float32",
    "uint64",
    "int64",
    "float64",
    "string",
  ];
  const ORDERS: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const PARITIES: { value: Parity; label: string }[] = $derived([
    { value: "N", label: t("conn.parity.none") },
    { value: "E", label: t("conn.parity.even") },
    { value: "O", label: t("conn.parity.odd") },
  ]);

  const meta = $derived(builder.meta);
  const base = $derived(meta.address_base ?? 0);

  /** Problems with what was typed in a row, before it reaches the map. */
  let local = $state<Record<number, string>>({});

  function setLocal(index: number, text: string | null) {
    const next = { ...local };
    if (text) next[index] = text;
    else delete next[index];
    local = next;
  }

  /** Optional header text: empty means the key is left out of the file. */
  const textOrNull = (s: string) => (s.trim() === "" ? null : s);

  /** A number field: a number, or null when it is empty. */
  function numberOrNull(s: string): number | null {
    if (s.trim() === "") return null;
    const n = Number(s);
    return Number.isFinite(n) ? n : null;
  }

  function setAddress(index: number, text: string) {
    const wire = wireAddress(Number(text), base);
    if (text.trim() === "" || wire === null) {
      setLocal(index, t("builder.bad_address"));
      return;
    }
    setLocal(index, null);
    builder.registers[index].address = wire;
    builder.touch();
  }

  function setLabels(index: number, text: string) {
    const pairs = parseEnum(text);
    if (pairs === null) {
      setLocal(index, t("builder.bad_labels"));
      return;
    }
    setLocal(index, null);
    builder.registers[index].enum = pairs;
    builder.touch();
  }

  function setArea(index: number, area: Area) {
    // Changed in place, so the row keeps its place and the focus.
    const r = builder.registers[index];
    Object.assign(r, withArea($state.snapshot(r), area));
    builder.touch();
  }

  function setType(index: number, type: DataType) {
    const r = builder.registers[index];
    r.type = type;
    r.length = type === "string" ? (r.length ?? 8) : null;
    builder.touch();
  }

  function setAccess(index: number, access: Access) {
    builder.registers[index].access = access;
    builder.touch();
  }

  function setBase(value: number) {
    // Keep each register at the same address on the wire while the way the
    // manual numbers them changes.
    meta.address_base = value;
    builder.touch();
  }

  let table: HTMLElement | undefined = $state();

  // Long maps render only the rows in view (see virtual.ts).
  let scrollTop = $state(0);
  let viewport = $state(600);
  let rowHeight = $state(36);
  const win = $derived(
    windowOf(builder.registers.length, scrollTop, viewport, rowHeight),
  );
  const visible = $derived(builder.registers.slice(win.start, win.end));
  // The height of a row is read from the first one shown.
  $effect(() => {
    void visible;
    const row = table?.querySelector<HTMLElement>("tbody tr.row");
    const h = row?.getBoundingClientRect().height ?? 0;
    if (h > 0 && Math.abs(h - rowHeight) > 0.5) rowHeight = h;
  });

  /** The row of a register, once it is rendered. */
  async function rowAt(index: number): Promise<HTMLElement | null> {
    if (!table) return null;
    // Out of the window: scroll there first, so that the row exists.
    table.scrollTop = Math.max(0, index * rowHeight - viewport / 2);
    scrollTop = table.scrollTop;
    await tick();
    return table.querySelector<HTMLElement>(
      `tbody tr.row[data-index="${index}"]`,
    );
  }

  /** Scrolls to the register a map was opened at, and puts the cursor in it. */
  $effect(() => {
    const at = builder.focus;
    if (at === null || !table) return;
    void rowAt(at).then((row) => {
      row?.scrollIntoView({ block: "center" });
      row?.querySelector<HTMLInputElement>("input")?.focus();
    });
  });

  let importing = $state(false);

  async function imported(first: number) {
    const row = await rowAt(first);
    row?.scrollIntoView({ block: "start" });
  }

  async function addRegister() {
    const at = builder.addRegister(builder.registers.length - 1);
    const row = await rowAt(at);
    row?.scrollIntoView({ block: "nearest" });
    row?.querySelector<HTMLInputElement>("input")?.focus();
  }
</script>

<TableImport bind:open={importing} onimported={imported} />

<div class="builder">
  <div class="toolbar">
    <button class="btn" onclick={() => builder.startNew()}
      ><Icon name="plus" />{t("builder.new")}</button
    >
    <button class="btn" onclick={() => builder.openFile()}
      ><Icon name="folder" />{t("builder.open_file")}</button
    >
    <button class="btn" onclick={() => (importing = true)}
      ><Icon name="imported" />{t("builder.import_table")}</button
    >
    <button class="btn accent" onclick={() => builder.save()}
      >{t("builder.save_here")}</button
    >
    <button class="btn" onclick={() => builder.saveAs()}
      >{t("builder.save")}</button
    >
    <span class="state" aria-live="polite">
      {#if builder.checking}<span class="dim">{t("builder.checking")}</span
        >{:else if builder.errors + builder.warnings === 0}<span class="ok"
          >{t("builder.no_problems")}</span
        >{:else}<span class:bad={builder.errors > 0}
          >{t("builder.n_problems", {
            errors: builder.errors,
            warnings: builder.warnings,
          })}</span
        >{/if}
      {#if builder.dirty}<span class="dim"> · {t("builder.unsaved")}</span>{/if}
    </span>
  </div>

  {#if builder.message}
    <p class="note" class:ok={builder.message.ok} role="status">
      {builder.message.text}
    </p>
  {/if}
  {#if builder.path}<p class="path dim mono">{builder.path}</p>{/if}

  {#if builder.changes}
    {@const c = builder.changes}
    <details class="card">
      <summary
        >{t("builder.changes")}
        <span class="dim"
          >{c.changed.length + c.added.length + c.removed.length}</span
        ></summary
      >
      {#if c.changed.length + c.added.length + c.removed.length === 0}
        <p class="dim">{t("builder.no_changes")}</p>
      {:else}
        <ul class="changes">
          {#each c.changed as ch, i (i)}
            <li>
              <strong>{t("builder.change_changed")}</strong>
              {ch.after.name}
              <span class="dim">({ch.fields.join(", ")})</span>
            </li>
          {/each}
          {#each c.added as r, i (i)}
            <li><strong>{t("builder.change_added")}</strong> {r.name}</li>
          {/each}
          {#each c.removed as r, i (i)}
            <li><strong>{t("builder.change_removed")}</strong> {r.name}</li>
          {/each}
        </ul>
      {/if}
    </details>
  {/if}

  <details class="card device" open>
    <summary>{t("builder.header")}</summary>
    <div class="fields">
      <label class="field">
        <span>{t("libraryview.vendor")}</span>
        <input
          class="input"
          bind:value={meta.vendor}
          oninput={() => builder.touch()}
          spellcheck="false"
        />
      </label>
      <label class="field">
        <span>{t("libraryview.model")}</span>
        <input
          class="input"
          bind:value={meta.model}
          oninput={() => builder.touch()}
          spellcheck="false"
        />
      </label>
      <label class="field wide">
        <span>{t("libraryview.description")}</span>
        <input
          class="input"
          value={meta.description ?? ""}
          oninput={(e) => {
            meta.description = textOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
      <label class="field wide">
        <span>{t("builder.source")}</span>
        <input
          class="input"
          value={meta.source ?? ""}
          oninput={(e) => {
            meta.source = textOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
    </div>
    <h4>{t("builder.defaults")}</h4>
    <div class="fields">
      <label class="field">
        <span>{t("conn.protocol")}</span>
        <select
          class="input"
          value={meta.protocol ?? ""}
          onchange={(e) => {
            const v = e.currentTarget.value;
            meta.protocol = v === "" ? null : (v as "tcp" | "rtu" | "tcp+rtu");
            builder.touch();
          }}
        >
          <option value="">—</option>
          <option value="tcp">Modbus TCP</option>
          <option value="rtu">Modbus RTU</option>
          <option value="tcp+rtu">TCP + RTU</option>
        </select>
      </label>
      <label class="field">
        <span>{t("req.unitId")}</span>
        <input
          class="input num"
          type="number"
          min="0"
          max="255"
          value={meta.unit_id ?? ""}
          oninput={(e) => {
            meta.unit_id = numberOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
      <label class="field">
        <span>{t("conn.port")}</span>
        <input
          class="input num"
          type="number"
          min="1"
          max="65535"
          value={meta.port ?? ""}
          oninput={(e) => {
            meta.port = numberOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
      <label class="field">
        <span>{t("conn.baud")}</span>
        <input
          class="input num"
          type="number"
          min="1"
          value={meta.baudrate ?? ""}
          oninput={(e) => {
            meta.baudrate = numberOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
      <label class="field">
        <span>{t("conn.parity")}</span>
        <select
          class="input"
          value={meta.parity ?? "N"}
          onchange={(e) => {
            meta.parity = e.currentTarget.value as Parity;
            builder.touch();
          }}
        >
          {#each PARITIES as p (p.value)}<option value={p.value}
              >{p.label}</option
            >{/each}
        </select>
      </label>
      <label class="field">
        <span>{t("conn.dataBits")}</span>
        <input
          class="input num"
          type="number"
          min="5"
          max="8"
          value={meta.data_bits ?? ""}
          oninput={(e) => {
            meta.data_bits = numberOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
      <label class="field">
        <span>{t("conn.stopBits")}</span>
        <input
          class="input num"
          type="number"
          min="1"
          max="2"
          value={meta.stop_bits ?? ""}
          oninput={(e) => {
            meta.stop_bits = numberOrNull(e.currentTarget.value);
            builder.touch();
          }}
        />
      </label>
      <label class="field">
        <span>{t("req.byteOrder")}</span>
        <select
          class="input"
          value={meta.byte_order ?? "ABCD"}
          onchange={(e) => {
            meta.byte_order = e.currentTarget.value as ByteOrder;
            builder.touch();
          }}
        >
          {#each ORDERS as o (o)}<option value={o}>{o}</option>{/each}
        </select>
      </label>
      <label class="field" title={t("builder.address_base_hint")}>
        <span>{t("builder.address_base")}</span>
        <select
          class="input"
          value={String(base)}
          onchange={(e) => setBase(Number(e.currentTarget.value))}
        >
          <option value="0">0</option>
          <option value="1">1</option>
        </select>
      </label>
    </div>
    <details class="opt">
      <summary>{t("builder.optional")}</summary>
      <div class="fields">
        <label class="field wide">
          <span>{t("builder.models_hint")}</span>
          <input
            class="input"
            value={(meta.models ?? []).join(", ")}
            oninput={(e) => {
              meta.models = e.currentTarget.value
                .split(",")
                .map((m) => m.trim())
                .filter(Boolean);
              builder.touch();
            }}
          />
        </label>
        <label class="field wide">
          <span>{t("hdr.website")}</span>
          <input
            class="input"
            value={meta.vendor_website ?? ""}
            oninput={(e) => {
              meta.vendor_website = textOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("hdr.firmware")}</span>
          <input
            class="input"
            value={meta.firmware ?? ""}
            oninput={(e) => {
              meta.firmware = textOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("builder.firmware_min")}</span>
          <input
            class="input"
            value={meta.firmware_min ?? ""}
            oninput={(e) => {
              meta.firmware_min = textOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("builder.firmware_max")}</span>
          <input
            class="input"
            value={meta.firmware_max ?? ""}
            oninput={(e) => {
              meta.firmware_max = textOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("hdr.revision")}</span>
          <input
            class="input num"
            type="number"
            min="0"
            value={meta.revision ?? ""}
            oninput={(e) => {
              meta.revision = numberOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("builder.revised")}</span>
          <input
            class="input"
            placeholder="YYYY-MM-DD"
            value={meta.revised ?? ""}
            oninput={(e) => {
              meta.revised = textOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("builder.max_regs")}</span>
          <input
            class="input num"
            type="number"
            min="1"
            value={meta.max_read_registers ?? ""}
            oninput={(e) => {
              meta.max_read_registers = numberOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field">
          <span>{t("builder.max_bits")}</span>
          <input
            class="input num"
            type="number"
            min="1"
            value={meta.max_read_bits ?? ""}
            oninput={(e) => {
              meta.max_read_bits = numberOrNull(e.currentTarget.value);
              builder.touch();
            }}
          />
        </label>
        <label class="field wide">
          <span>{t("builder.notes")}</span>
          <textarea
            class="input"
            rows="3"
            value={(meta.notes ?? []).join("\n")}
            oninput={(e) => {
              meta.notes = e.currentTarget.value
                .split("\n")
                .map((n) => n.trim())
                .filter(Boolean);
              builder.touch();
            }}></textarea>
        </label>
      </div>
      <MapMetaLists {meta} ontouch={() => builder.touch()} />
    </details>
    {#each builder.sorted.file as d, i (i)}
      <p class="msg" class:warn={d.severity === "warning"}>
        {d.pointer ?? ""}
        {d.message}
      </p>
    {/each}
  </details>

  <div class="regs-head">
    <strong>{t("builder.registers")}</strong>
    <span class="dim">{builder.registers.length}</span>
    <button class="btn" onclick={addRegister}
      ><Icon name="plus" />{t("builder.add_register")}</button
    >
  </div>

  <div
    class="table-wrap"
    bind:this={table}
    bind:clientHeight={viewport}
    onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  >
    <table>
      <thead>
        <tr>
          <th class="name">{t("computedform.name")}</th>
          <th class="grp">{t("maptable.group")}</th>
          <th class="area">{t("customregisterform.area")}</th>
          <th class="addr">{t("req.address")}</th>
          <th class="type">{t("customregisterform.type")}</th>
          <th class="num">{t("customregisterform.scale")}</th>
          <th class="unit">{t("computedform.unit")}</th>
          <th class="acc">{t("customregisterform.access")}</th>
          <th class="labels" title={t("builder.labels_hint")}
            >{t("builder.labels")}</th
          >
          <th class="desc">{t("libraryview.description")}</th>
          <th class="act"></th>
        </tr>
      </thead>
      <tbody>
        {#if win.before}
          <tr class="spacer" aria-hidden="true" style:height="{win.before}px"
            ><td colspan="11"></td></tr
          >
        {/if}
        {#each visible as reg, i (reg)}
          {@const index = win.start + i}
          {@const problems = builder.sorted.rows.get(index) ?? []}
          {@const bits = isBitArea(reg.area)}
          <tr
            class="row"
            data-index={index}
            class:focus={builder.focus === index}
            class:bad={problems.some((p) => p.severity === "error")}
          >
            <td class="name"
              ><input
                class="input"
                aria-label={t("computedform.name")}
                bind:value={reg.name}
                oninput={() => builder.touch()}
                spellcheck="false"
              /></td
            >
            <td class="grp"
              ><input
                class="input"
                aria-label={t("maptable.group")}
                bind:value={reg.group}
                oninput={() => builder.touch()}
              /></td
            >
            <td class="area"
              ><select
                class="input"
                aria-label={t("customregisterform.area")}
                value={reg.area}
                onchange={(e) => setArea(index, e.currentTarget.value as Area)}
              >
                {#each AREAS as a (a.value)}<option value={a.value}
                    >{a.label}</option
                  >{/each}
              </select></td
            >
            <td class="addr"
              ><input
                class="input num"
                type="number"
                aria-label={t("req.address")}
                value={fileAddress(reg, base)}
                aria-invalid={local[index] === t("builder.bad_address")}
                onchange={(e) => setAddress(index, e.currentTarget.value)}
              /></td
            >
            <td class="type">
              {#if bits}<span class="mono dim">bool</span>{:else}<select
                  class="input"
                  aria-label={t("customregisterform.type")}
                  value={reg.type}
                  onchange={(e) =>
                    setType(index, e.currentTarget.value as DataType)}
                >
                  {#each TYPES as ty (ty)}<option value={ty}>{ty}</option
                    >{/each}
                  {#if reg.type === "bool"}<option value="bool">bool</option
                    >{/if}
                </select>
                {#if reg.type === "string"}<input
                    class="input num len"
                    type="number"
                    min="1"
                    aria-label={t("customregisterform.length_registers")}
                    title={t("customregisterform.length_registers")}
                    value={reg.length ?? 1}
                    oninput={(e) => {
                      reg.length = numberOrNull(e.currentTarget.value);
                      builder.touch();
                    }}
                  />{/if}{/if}
            </td>
            <td class="num"
              >{#if !bits}<input
                  class="input num"
                  type="number"
                  step="any"
                  aria-label={t("customregisterform.scale")}
                  value={reg.scale}
                  oninput={(e) => {
                    const n = numberOrNull(e.currentTarget.value);
                    if (n !== null) {
                      reg.scale = n;
                      builder.touch();
                    }
                  }}
                />{/if}</td
            >
            <td class="unit"
              >{#if !bits}<input
                  class="input"
                  aria-label={t("computedform.unit")}
                  bind:value={reg.unit}
                  oninput={() => builder.touch()}
                />{/if}</td
            >
            <td class="acc"
              ><select
                class="input"
                aria-label={t("customregisterform.access")}
                value={reg.access}
                disabled={reg.area === "input" || reg.area === "discrete"}
                onchange={(e) =>
                  setAccess(index, e.currentTarget.value as Access)}
              >
                <option value="R">R</option>
                <option value="RW">RW</option>
                <option value="W">W</option>
              </select></td
            >
            <td class="labels"
              ><input
                class="input"
                aria-label={t("builder.labels")}
                placeholder="0=Off, 1=On"
                value={enumText(reg.enum)}
                aria-invalid={local[index] === t("builder.bad_labels")}
                onchange={(e) => setLabels(index, e.currentTarget.value)}
              /></td
            >
            <td class="desc"
              ><input
                class="input"
                aria-label={t("libraryview.description")}
                bind:value={reg.description}
                oninput={() => builder.touch()}
              /></td
            >
            <td class="act">
              <button
                class="btn icon"
                title={t("builder.row_up")}
                aria-label={t("builder.row_up")}
                disabled={index === 0}
                onclick={() => {
                  builder.move(index, -1);
                  local = {};
                }}>↑</button
              >
              <button
                class="btn icon"
                title={t("builder.row_down")}
                aria-label={t("builder.row_down")}
                disabled={index === builder.registers.length - 1}
                onclick={() => {
                  builder.move(index, 1);
                  local = {};
                }}>↓</button
              >
              <button
                class="btn icon"
                title={t("builder.row_duplicate")}
                aria-label={t("builder.row_duplicate")}
                onclick={() => {
                  builder.duplicate(index);
                  local = {};
                }}><Icon name="plus" /></button
              >
              <button
                class="btn icon"
                title={t("builder.row_delete")}
                aria-label={t("builder.row_delete")}
                onclick={() => {
                  builder.remove(index);
                  local = {};
                }}>×</button
              >
            </td>
          </tr>
          {#if problems.length > 0 || local[index]}
            <tr class="msgs">
              <td colspan="11">
                {#if local[index]}<p class="msg">{local[index]}</p>{/if}
                {#each problems as d, i (i)}
                  <p class="msg" class:warn={d.severity === "warning"}>
                    <span class="mono">{d.pointer}</span>
                    {d.message}
                  </p>
                {/each}
              </td>
            </tr>
          {/if}
        {/each}
        {#if win.after}
          <tr class="spacer" aria-hidden="true" style:height="{win.after}px"
            ><td colspan="11"></td></tr
          >
        {/if}
        {#if builder.registers.length === 0}
          <tr><td colspan="11" class="empty dim">{t("builder.empty")}</td></tr>
        {/if}
      </tbody>
    </table>
  </div>
</div>

<style>
  .builder {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    flex: 1;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .state {
    margin-left: auto;
    font-size: 13px;
  }

  .ok {
    color: var(--ok, var(--text-2));
  }

  .bad {
    color: var(--bad, #c0392b);
  }

  .note {
    margin: 0;
    font-size: 13px;
    color: var(--bad, #c0392b);
  }

  .note.ok {
    color: var(--text-2);
  }

  .path {
    margin: -6px 0 0;
    font-size: 12px;
    word-break: break-all;
  }

  .card {
    background: var(--field);
    border-radius: 10px;
    padding: 10px 14px;
  }

  summary {
    cursor: pointer;
    font-weight: 600;
  }

  h4 {
    margin: 12px 0 6px;
    font-size: 13px;
    color: var(--text-2);
    font-weight: 600;
  }

  .changes {
    margin: 8px 0 0;
    padding-left: 18px;
    font-size: 13px;
    max-height: 160px;
    overflow: auto;
  }

  tr.row.focus td {
    background: var(--accent-soft);
  }

  /* The device header can be taller than the window (optional fields, sources,
     notes): it scrolls inside its card and leaves room for the registers. */
  .device {
    flex-shrink: 0;
    max-height: 50vh;
    overflow: auto;
  }

  .fields {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 12px;
    margin-top: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 120px;
  }

  .field.wide {
    flex: 1 1 320px;
  }

  .regs-head {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .regs-head .btn {
    margin-left: auto;
  }

  tr.spacer td {
    padding: 0;
    border: 0;
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
    table-layout: fixed;
    min-width: 1100px;
  }

  th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--card);
    text-align: left;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    padding: 6px 4px;
  }

  td {
    padding: 3px 3px;
    vertical-align: middle;
  }

  td .input,
  td select {
    width: 100%;
    min-width: 0;
  }

  .name {
    width: 17%;
  }
  .grp {
    width: 9%;
  }
  .area {
    width: 11%;
  }
  .addr {
    width: 82px;
  }
  .type {
    width: 118px;
  }
  .num {
    width: 78px;
  }
  .unit {
    width: 64px;
  }
  .acc {
    width: 66px;
  }
  .labels {
    width: 14%;
  }
  .desc {
    width: auto;
  }
  .act {
    width: 136px;
    white-space: nowrap;
    text-align: right;
  }

  /* The icon button has no text baseline: align all by their middle. */
  .act .btn {
    vertical-align: middle;
  }

  td.type {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .len {
    width: 54px;
  }

  tr.bad .input {
    border-color: var(--bad, #c0392b);
  }

  .msgs td {
    padding: 0 6px 6px;
  }

  .msg {
    margin: 0;
    font-size: 12px;
    color: var(--bad, #c0392b);
  }

  .msg.warn {
    color: var(--text-2);
  }

  .empty {
    text-align: center;
    padding: 24px;
  }

  [aria-invalid="true"] {
    border-color: var(--bad, #c0392b);
  }
  .opt {
    margin-top: 12px;
  }
  .opt > summary {
    cursor: pointer;
    font-weight: 600;
    margin-bottom: 8px;
  }
</style>
