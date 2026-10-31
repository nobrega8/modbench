<!--
  Export a map as the configuration of other software: Home Assistant,
  ESPHome, Telegraf, or a CSV tag list. The device's address comes from the
  connection set in step 1, or is left as a placeholder to fill in.
-->
<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { copy } from "#lib/contextmenu.svelte.ts";
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { toasts } from "#lib/toast.svelte.ts";
  import type { ExportFormat, ExportOptions } from "#lib/types.ts";
  import Icon from "./Icon.svelte";

  let {
    path = $bindable(null),
    name = "",
  }: {
    /** The map to export; null closes the dialog. */
    path?: string | null;
    name?: string;
  } = $props();

  const FORMATS: { id: ExportFormat; label: string; hint: Key }[] = [
    { id: "homeassistant", label: "Home Assistant", hint: "export.ha" },
    { id: "esphome", label: "ESPHome", hint: "export.esphome" },
    { id: "telegraf", label: "Telegraf", hint: "export.telegraf" },
    { id: "csv", label: "CSV", hint: "export.csv" },
  ];

  let dialog: HTMLDialogElement | undefined = $state();
  let format = $state<ExportFormat>("homeassistant");
  let useConnection = $state(true);
  let interval = $state(10);
  let busy = $state(false);

  /** The connection of step 1, in a few words. */
  const where = $derived.by(() => {
    const tr = bench.config().transport;
    return "host" in tr ? `${tr.host || "?"}:${tr.port}` : tr.port || "?";
  });
  const connected = $derived(format !== "csv");

  $effect(() => {
    if (!dialog) return;
    if (path && !dialog.open) dialog.showModal();
    else if (!path && dialog.open) dialog.close();
  });

  async function produce() {
    const options: ExportOptions = { interval_s: interval };
    if (useConnection) options.connection = bench.config();
    return api.exportMap(path!, format, options);
  }

  async function saveAs() {
    busy = true;
    try {
      const out = await produce();
      const ext = out.file_name.split(".").pop()!;
      const where = await save({
        defaultPath: out.file_name,
        filters: [{ name: ext.toUpperCase(), extensions: [ext] }],
      });
      if (!where) return;
      await api.saveText(where, out.text);
      toasts.show(t("export.saved", { path: where }));
      path = null;
    } catch (e) {
      toasts.fail(errorText(e));
    } finally {
      busy = false;
    }
  }

  async function copyText() {
    busy = true;
    try {
      await copy((await produce()).text);
      toasts.show(t("export.copied"));
    } catch (e) {
      toasts.fail(errorText(e));
    } finally {
      busy = false;
    }
  }
</script>

<dialog
  bind:this={dialog}
  onclose={() => (path = null)}
  aria-labelledby="export-title"
>
  <header>
    <div class="heading">
      <h2 id="export-title">{t("export.title")}</h2>
      <p>{t("export.hint", { name })}</p>
    </div>
    <button
      class="btn icon ghost"
      title={t("mapexplorer.close")}
      onclick={() => (path = null)}><Icon name="close" size={16} /></button
    >
  </header>

  <div class="body">
    <div class="formats" role="radiogroup">
      {#each FORMATS as f (f.id)}
        <label class="format" class:on={format === f.id}>
          <input type="radio" value={f.id} bind:group={format} />
          <span class="name">{f.label}</span>
          <span class="dim">{t(f.hint)}</span>
        </label>
      {/each}
    </div>

    <label class="check" class:off={!connected}>
      <input
        type="checkbox"
        bind:checked={useConnection}
        disabled={!connected}
      />
      {t("export.use_connection", { where })}
    </label>
    <label class="check" class:off={!connected}>
      {t("export.interval")}
      <input
        class="input num"
        type="number"
        min="1"
        bind:value={interval}
        disabled={!connected}
      />
    </label>
    <p class="dim note">{t("export.note")}</p>
  </div>

  <footer>
    <span class="spacer"></span>
    <button class="btn" disabled={busy} onclick={copyText}
      >{t("export.copy")}</button
    >
    <button class="btn accent" disabled={busy} onclick={saveAs}
      >{t("export.save")}</button
    >
  </footer>
</dialog>

<style>
  dialog {
    width: min(560px, calc(100vw - 48px));
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

  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 0 20px;
  }

  .formats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .format {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-radius: 10px;
    box-shadow: inset 0 0 0 1px var(--border);
    cursor: pointer;
    font-size: 12px;
  }

  .format.on {
    box-shadow: inset 0 0 0 2px var(--text);
  }

  .format:has(input:focus-visible) {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }

  .format input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .format .name {
    font-size: 13px;
    font-weight: 600;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .check.off {
    opacity: 0.5;
  }

  .check .num {
    width: 80px;
  }

  .note {
    margin: 0;
    font-size: 12px;
  }

  footer {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 20px;
  }

  .spacer {
    flex: 1;
  }
</style>
