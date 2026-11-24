<!--
  A read-only look at a library map: what it says about the device and its
  registers, without opening it on the Main page. The registers scroll; the
  map's header stays beside them.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { api, errorText } from "#lib/api.ts";
  import { AREA_NAME, AREA_TAG, formatAddress } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { revealIn } from "#lib/motion.ts";
  import { extraRows, headerRows, mapStatusOf } from "#lib/map-header.ts";
  import type { Area, DeviceMap, Register } from "#lib/types.ts";
  import Icon from "./Icon.svelte";
  import MapStatus, { mapStatus } from "./MapStatus.svelte";
  import Skeleton from "./Skeleton.svelte";

  interface Props {
    /** The map file to show, or null when closed. */
    path: string | null;
  }

  let { path = $bindable() }: Props = $props();

  /** Registers drawn at first; more on request, to keep big maps light. */
  const PAGE = 150;

  let dialog: HTMLDialogElement | undefined = $state();
  let map = $state.raw<DeviceMap | null>(null);
  let problem = $state<string | null>(null);
  let search = $state("");
  let area = $state<Area | "">("");
  let access = $state<"" | "R" | "W">("");
  let group = $state("");
  let limit = $state(PAGE);
  /** Registers whose details are open, by their place in the map. */
  let open = $state<ReadonlySet<number>>(new Set());

  $effect(() => {
    if (!dialog) return;
    if (path && !dialog.open) {
      dialog.showModal();
      load(path);
    } else if (!path && dialog.open) dialog.close();
  });

  async function load(p: string) {
    map = null;
    problem = null;
    search = "";
    area = "";
    access = "";
    group = "";
    limit = PAGE;
    open = new Set();
    try {
      const parsed = await api.openMap(p);
      if (parsed.map) map = parsed.map;
      else
        problem =
          parsed.diagnostics.find((d) => d.severity === "error")?.message ??
          t("mapexplorer.the_map_cannot_be");
    } catch (e) {
      problem = errorText(e);
    }
  }

  /** Connection defaults the map sets, as short facts. */
  const facts = $derived.by(() => {
    const m = map?.meta;
    if (!m) return [];
    const serial =
      m.baudrate !== null
        ? `${m.baudrate} ${m.data_bits ?? 8}${m.parity ?? "N"}${m.stop_bits ?? 1}`
        : null;
    return [
      m.protocol?.toUpperCase().replace("+", " + "),
      m.unit_id !== null
        ? t("mapexplorer.unit_unit", { unit: m.unit_id })
        : null,
      m.port !== null ? t("mapexplorer.port_port", { port: m.port }) : null,
      serial,
      m.byte_order,
      m.firmware ? `FW ${m.firmware}` : null,
    ].filter((f): f is string => !!f);
  });

  /** The header of the map for the side panel; the description is already
      under the title. */
  const header = $derived(
    map
      ? headerRows(map.meta, map.registers.length).filter(
          ([label]) => label !== "Description",
        )
      : [],
  );

  const STATUS_TEXT: Record<string, string> = $derived({
    verified: t("mapexplorer.verified_on_a_device"),
    official: t("mapexplorer.official_from_the_manufacturer"),
    unofficial: t("mapexplorer.unofficial_not_checked_against"),
  });
  const status = $derived(map ? mapStatus(mapStatusOf(map.meta)) : null);

  /** Lower case and without accents, so "pressao" finds "Pressão". */
  const fold = (x: string) =>
    x.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

  const areas = $derived(
    (["holding", "input", "coil", "discrete"] as const).filter((a) =>
      map?.registers.some((r) => r.area === a),
    ),
  );
  const groups = $derived(
    [
      ...new Set((map?.registers ?? []).map((r) => r.group).filter(Boolean)),
    ].sort((a, b) => a.localeCompare(b, undefined, { numeric: true })),
  );

  /** The registers that match the search and the filters, in the order of
      the map. A number (or 0x…) in the search also finds that address. */
  const shown = $derived.by(() => {
    const query = search.trim();
    const num = /^(\d+|0x[0-9a-f]+)$/i.test(query) ? Number(query) : null;
    const words =
      num != null && /^0x/i.test(query)
        ? []
        : fold(query).split(/\s+/).filter(Boolean);
    const out: { r: Register; i: number }[] = [];
    (map?.registers ?? []).forEach((r, i) => {
      if (area && r.area !== area) return;
      if (access === "R" && r.access !== "R") return;
      if (access === "W" && r.access === "R") return;
      if (group && r.group !== group) return;
      if (query) {
        const hay = fold(
          `${r.name} ${r.group} ${r.description} ${r.unit} ${r.address}`,
        );
        if (
          !(num != null && r.address === num) &&
          !words.every((w) => hay.includes(w))
        )
          return;
      }
      out.push({ r, i });
    });
    return out;
  });

  // Any change of what is asked starts the list over, closed.
  $effect(() => {
    search;
    area;
    access;
    group;
    limit = PAGE;
    open = new Set();
  });

  function toggle(i: number) {
    const next = new Set(open);
    if (!next.delete(i)) next.add(i);
    open = next;
  }

  const typeText = (r: Register) =>
    r.length && r.length > 1 ? `${r.type}[${r.length}]` : r.type;

  function scaleText(r: Register): string {
    const parts: string[] = [];
    if (r.scale_factor) parts.push(`×10^${r.scale_factor}`);
    if (r.scale !== 1) parts.push(`×${r.scale}`);
    if (r.offset)
      parts.push(`${r.offset < 0 ? "−" : "+"}${Math.abs(r.offset)}`);
    return parts.join(" ");
  }

  /** What the map says about one register, as label/value pairs. */
  function details(r: Register): [string, string][] {
    const rows: [string, string | null][] = [
      [t("customregisterform.type"), typeText(r)],
      [t("customregisterform.access"), r.access],
      [t("computedform.unit"), r.unit],
      [t("customregisterform.scale"), scaleText(r)],
      [
        t("mapexplorer.write_limits"),
        r.write_min != null || r.write_max != null
          ? `${r.write_min ?? "—"} … ${r.write_max ?? "—"}${r.unit ? ` ${r.unit}` : ""}`
          : null,
      ],
      [
        t("hdr.firmware"),
        [
          r.firmware_min && `≥ ${r.firmware_min}`,
          r.firmware_max && `≤ ${r.firmware_max}`,
        ]
          .filter(Boolean)
          .join(" "),
      ],
      [t("maptable.group"), r.group],
      [t("libraryview.description"), r.description],
      [t("mapexplorer.comment"), r.comment ?? null],
    ];
    return rows.filter((row): row is [string, string] => !!row[1]);
  }
</script>

<dialog
  bind:this={dialog}
  onclose={() => (path = null)}
  aria-labelledby="explore-title"
>
  <header>
    <div class="heading">
      <h2 id="explore-title">
        {map ? `${map.meta.vendor} ${map.meta.model}` : "Map"}
      </h2>
      {#if map?.meta.description}<p>{map.meta.description}</p>{/if}
      {#if facts.length}
        <div class="facts">
          {#each facts as f (f)}<span>{f}</span>{/each}
        </div>
      {/if}
    </div>
    <button
      class="btn icon ghost"
      title={t("mapexplorer.close")}
      onclick={() => (path = null)}><Icon name="close" size={16} /></button
    >
  </header>

  <div class="body">
    <div class="main">
      {#if map}
        <div class="toolbar">
          <input
            class="input search"
            type="search"
            placeholder={t("mapexplorer.search_registers")}
            bind:value={search}
            aria-label={t("mapexplorer.search_registers")}
          />
          {#if areas.length > 1}
            <select class="input" bind:value={area}>
              <option value="">{t("mapexplorer.all_areas")}</option>
              {#each areas as a (a)}
                <option value={a}>{AREA_TAG[a]} · {AREA_NAME[a]}</option>
              {/each}
            </select>
          {/if}
          <select class="input" bind:value={access}>
            <option value="">{t("mapexplorer.any_access")}</option>
            <option value="R">{t("mapexplorer.read_only")}</option>
            <option value="W">{t("mapexplorer.writable")}</option>
          </select>
          {#if groups.length > 1}
            <select class="input" bind:value={group}>
              <option value="">{t("mapexplorer.all_groups")}</option>
              {#each groups as g (g)}<option value={g}>{g}</option>{/each}
            </select>
          {/if}
          <span class="count" aria-live="polite">
            {t("mappicker.registers_count", {
              count: shown.length,
            })}{shown.length !== map.registers.length
              ? ` / ${map.registers.length}`
              : ""}
          </span>
        </div>
      {/if}

      <div class="list">
        {#if problem}
          <p class="note bad">{problem}</p>
        {:else if !map}
          <Skeleton rows={8} />
        {:else}
          <table class="data" in:revealIn>
            <thead>
              <tr>
                <th>{t("req.address")}</th>
                <th>{t("computedform.name")}</th>
                <th>{t("customregisterform.type")}</th>
                <th>{t("customregisterform.access")}</th>
                <th>{t("computedform.unit")}</th>
                <th>{t("libraryview.description")}</th>
              </tr>
            </thead>
            <tbody>
              {#each shown.slice(0, limit) as { r, i } (i)}
                {@const isOpen = open.has(i)}
                <tr class:open={isOpen}>
                  <td class="mono"
                    ><span class="tag">{AREA_TAG[r.area]}</span>{formatAddress(
                      r.address,
                      settings.addressFormat,
                      r.area,
                    )}</td
                  >
                  <td class="name">
                    <button
                      class="reg-name"
                      aria-expanded={isOpen}
                      onclick={() => toggle(i)}
                      >{r.name}{#if r.group}<span class="group">{r.group}</span
                        >{/if}{#if r.enum.length || r.enum_ref}<span
                          class="pill">enum</span
                        >{/if}{#if r.parts?.length}<span class="pill"
                          >parts</span
                        >{/if}</button
                    >
                  </td>
                  <td class="mono">{typeText(r)}</td>
                  <td class="mono">{r.access}</td>
                  <td>{r.unit}</td>
                  <td class="desc" title={r.description || undefined}
                    >{r.description}</td
                  >
                </tr>
                {#if isOpen}
                  <tr class="detail">
                    <td colspan="6">
                      <dl class="facts">
                        {#each details(r) as [label, value] (label)}
                          <dt>{label}</dt>
                          <dd>{value}</dd>
                        {/each}
                      </dl>
                      {#if r.parts?.length}
                        <h4>{t("mapexplorer.parts")}</h4>
                        <table class="data inner">
                          <tbody>
                            {#each r.parts as p (p.offset + p.name + (p.bit ?? ""))}
                              {@const first = r.address + p.offset}
                              {@const bitField =
                                p.bit !== undefined && p.bit !== null}
                              <tr>
                                <td>{p.name}</td>
                                <td class="mono"
                                  >{bitField
                                    ? (p.bits ?? 1) > 1
                                      ? `bits ${p.bit}–${(p.bit ?? 0) + (p.bits ?? 1) - 1}`
                                      : `bit ${p.bit}`
                                    : p.length > 1
                                      ? `${first}–${first + p.length - 1}`
                                      : first}</td
                                >
                                <td class="mono"
                                  >{bitField
                                    ? (p.enum ?? [])
                                        .map(([v, l]) => `${v}=${l}`)
                                        .join(", ")
                                    : p.type}</td
                                >
                                <td>{p.description ?? ""}</td>
                              </tr>
                            {/each}
                          </tbody>
                        </table>
                      {/if}
                      {#if r.enum.length}
                        <h4>
                          {t("mapexplorer.values")}{r.enum_ref
                            ? ` · ${r.enum_ref}`
                            : ""}
                        </h4>
                        <table class="data inner">
                          <tbody>
                            {#each r.enum as [value, label] (value)}
                              <tr
                                ><td class="mono">{value}</td><td>{label}</td
                                ></tr
                              >
                            {/each}
                          </tbody>
                        </table>
                      {/if}
                    </td>
                  </tr>
                {/if}
              {:else}
                <tr
                  ><td colspan="6" class="note"
                    >{t("mapexplorer.no_registers_match")}</td
                  ></tr
                >
              {/each}
            </tbody>
          </table>
          {#if shown.length > limit}
            <button class="btn more" onclick={() => (limit += PAGE)}
              >{t("mapexplorer.show_more", {
                count: shown.length - limit,
              })}</button
            >
          {/if}
        {/if}
      </div>
    </div>

    {#if map}
      {@const m = map.meta}
      <aside aria-label={t("mapexplorer.map_properties")}>
        {#if status}
          <p class="status">
            <MapStatus {status} />{STATUS_TEXT[status] ?? status}
          </p>
        {/if}
        <dl>
          {#each header as [label, value] (label)}
            <dt>{label}</dt>
            <dd>{value}</dd>
          {/each}
          {#if m.enums.length}
            <dt>{t("mapexplorer.translation_tables")}</dt>
            <dd>{m.enums.map(([name]) => name).join(", ")}</dd>
          {/if}
          {#if m.sentinels.length}
            <dt>{t("mapexplorer.sentinels")}</dt>
            <dd>
              {m.sentinels
                .map(
                  ([type, values]) =>
                    `${type}: ${values.map(([, label]) => label).join(", ")}`,
                )
                .join("; ")}
            </dd>
          {/if}
          {#each extraRows(m) as [key, value] (key)}
            <dt class="mono">{key}</dt>
            <dd>{value}</dd>
          {/each}
        </dl>

        {#if m.sources?.length}
          <h3>{t("mapexplorer.sources")}</h3>
          <ul>
            {#each m.sources as src, i (i)}
              <li>
                <span class="title">{src.title}</span>
                {#if src.version || src.date}
                  <span class="sub"
                    >{[src.version, src.date].filter(Boolean).join(" · ")}</span
                  >
                {/if}
                {#if src.url}<span class="url">{src.url}</span>{/if}
                {#if src.note}<span class="sub">{src.note}</span>{/if}
                {#if src.accessed || src.sha256}
                  <span class="sub"
                    >{[
                      src.accessed &&
                        t("mapexplorer.accessed", { date: src.accessed }),
                      src.sha256 && `SHA-256 ${src.sha256.slice(0, 12)}…`,
                    ]
                      .filter(Boolean)
                      .join(" · ")}</span
                  >
                {/if}
              </li>
            {/each}
          </ul>
        {/if}

        {#if m.notes?.length}
          <h3>{t("mapexplorer.notes")}</h3>
          <ul>
            {#each m.notes as note, i (i)}<li>{note}</li>{/each}
          </ul>
        {/if}
      </aside>
    {/if}
  </div>
</dialog>

<style>
  dialog {
    width: min(1120px, calc(100vw - 48px));
    height: min(640px, calc(100vh - 80px));
    padding: 0;
    border: 0;
    border-radius: var(--r-xl);
    background: var(--card);
    color: var(--text);
    box-shadow: var(--e3);
    flex-direction: column;
    overflow: hidden;
  }

  dialog[open] {
    display: flex;
    animation: rise 0.22s var(--ease);
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.32);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px) scale(0.99);
    }
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 16px 16px 10px 20px;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  header p {
    margin: 2px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }

  .facts {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }

  .facts span {
    padding: 1px 7px;
    border-radius: var(--r-sm);
    background: rgb(var(--fg-rgb) / 0.06);
    color: var(--text-2);
    font-size: 12px;
  }

  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 0 12px 8px 20px;
  }

  .search {
    flex: 1 1 180px;
    min-width: 140px;
  }

  .toolbar select {
    flex: none;
    max-width: 170px;
  }

  .count {
    margin-left: auto;
    color: var(--text-3);
    font-size: 12px;
    white-space: nowrap;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 12px 12px;
  }

  /* Beside the table and outside its scroll: always in view. */
  aside {
    flex: none;
    width: 300px;
    overflow-y: auto;
    padding: 2px 18px 16px 16px;
    border-left: 1px solid var(--border);
    font-size: 12.5px;
    user-select: text;
  }

  aside .status {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 0 10px;
    color: var(--text-2);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 5px 12px;
    margin: 0;
  }

  dt {
    color: var(--text-3);
    white-space: nowrap;
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  h3 {
    margin: 16px 0 6px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  aside ul {
    margin: 0;
    padding-left: 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  aside li span {
    display: block;
  }

  .sub {
    color: var(--text-2);
  }

  .url {
    color: var(--text-2);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 13px;
  }

  th {
    position: sticky;
    top: 0;
    background: var(--card);
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: left;
    padding: 6px 8px;
  }

  td {
    padding: 5px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
  }

  .reg-name {
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .reg-name::before {
    content: "›";
    display: inline-block;
    width: 12px;
    color: var(--text-3);
    transition: transform 0.15s var(--ease);
  }

  .reg-name[aria-expanded="true"]::before {
    transform: rotate(90deg);
  }

  tr:not(.detail):hover,
  tr.open {
    background: var(--accent-soft);
  }

  .pill {
    margin-left: 6px;
    padding: 0 5px;
    border-radius: var(--r-sm);
    background: rgb(var(--fg-rgb) / 0.06);
    color: var(--text-3);
    font-size: 10.5px;
  }

  .desc {
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-2);
  }

  .detail td {
    padding: 8px 8px 14px 26px;
    white-space: normal;
    user-select: text;
  }

  .facts {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 4px 16px;
    margin: 0;
    font-size: 12.5px;
  }

  .detail h4 {
    margin: 12px 0 4px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .inner {
    width: auto;
    font-size: 12.5px;
  }

  .inner td {
    padding: 2px 16px 2px 0;
    border: 0;
  }

  .more {
    display: block;
    margin: 12px auto 0;
  }

  .tag {
    margin-right: 6px;
    color: var(--text-3);
    font-size: 11px;
  }

  .group {
    margin-left: 8px;
    color: var(--text-3);
    font-size: 12px;
  }

  .note {
    color: var(--text-2);
    padding: 12px 8px;
  }

  .bad {
    color: var(--bad);
  }
</style>
