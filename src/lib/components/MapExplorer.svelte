<!--
  A read-only look at a library map: what it says about the device and its
  registers, without opening it on the Main page. The registers scroll; the
  map's header stays beside them.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { api, errorText } from "#lib/api.ts";
  import { AREA_TAG, formatAddress } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { extraRows, headerRows, mapStatusOf } from "#lib/map-header.ts";
  import type { DeviceMap } from "#lib/types.ts";
  import Icon from "./Icon.svelte";
  import MapStatus, { mapStatus } from "./MapStatus.svelte";

  interface Props {
    /** The map file to show, or null when closed. */
    path: string | null;
  }

  let { path = $bindable() }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let map = $state.raw<DeviceMap | null>(null);
  let problem = $state<string | null>(null);
  let search = $state("");

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
    try {
      const parsed = await api.openMap(p);
      if (parsed.map) map = parsed.map;
      else
        problem =
          parsed.diagnostics.find((d) => d.severity === "error")?.message ??
          "The map cannot be read.";
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
      m.unit_id !== null ? `Unit ${m.unit_id}` : null,
      m.port !== null ? `Port ${m.port}` : null,
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

  const STATUS_TEXT: Record<string, string> = {
    verified: "Verified on a device",
    official: "Official: from the manufacturer's documentation",
    unofficial: "Unofficial: not checked against the documentation",
  };
  const status = $derived(map ? mapStatus(mapStatusOf(map.meta)) : null);

  const shown = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const regs = map?.registers ?? [];
    if (!q) return regs;
    return regs.filter(
      (r) =>
        r.name.toLowerCase().includes(q) ||
        r.group.toLowerCase().includes(q) ||
        r.description.toLowerCase().includes(q),
    );
  });
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
    <input
      class="input search"
      type="search"
      placeholder={t("mapexplorer.search_registers")}
      bind:value={search}
      aria-label={t("mapexplorer.search_registers")}
    />
    <button
      class="btn icon ghost"
      title={t("mapexplorer.close")}
      onclick={() => (path = null)}><Icon name="close" size={16} /></button
    >
  </header>

  <div class="body">
    <div class="list">
      {#if problem}
        <p class="note bad">{problem}</p>
      {:else if !map}
        <p class="note">{t("mapexplorer.loading")}</p>
      {:else}
        <table class="data">
          <thead>
            <tr>
              <th>{t("req.address")}</th>
              <th>{t("computedform.name")}</th>
              <th>{t("customregisterform.type")}</th>
              <th>{t("computedform.unit")}</th>
              <th>{t("customregisterform.access")}</th>
            </tr>
          </thead>
          <tbody>
            {#each shown as r, i (i)}
              <tr title={r.description || undefined}>
                <td class="mono"
                  ><span class="tag">{AREA_TAG[r.area]}</span>{formatAddress(
                    r.address,
                    settings.addressFormat,
                    r.area,
                  )}</td
                >
                <td
                  >{r.name}{#if r.group}<span class="group">{r.group}</span
                    >{/if}</td
                >
                <td class="mono">{r.type}</td>
                <td>{r.unit}</td>
                <td>{r.access}</td>
              </tr>
            {:else}
              <tr
                ><td colspan="5" class="note"
                  >{t("mapexplorer.no_registers_match")}</td
                ></tr
              >
            {/each}
          </tbody>
        </table>
      {/if}
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
    border-radius: 14px;
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
    border-radius: 6px;
    background: rgb(var(--fg-rgb) / 0.06);
    color: var(--text-2);
    font-size: 12px;
  }

  .search {
    width: 200px;
    flex: none;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .list {
    flex: 1;
    min-width: 0;
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
