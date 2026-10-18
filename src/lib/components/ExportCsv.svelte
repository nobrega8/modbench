<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";

  /** A button that saves scan results as a CSV file. `make` builds the
   * text when the file is chosen, so a running scan exports what it has. */
  let {
    name,
    make,
    disabled = false,
  }: { name: string; make: () => string; disabled?: boolean } = $props();

  let error = $state<string | null>(null);
  let saved = $state<string | null>(null);

  async function exportCsv() {
    error = null;
    saved = null;
    const path = await save({
      defaultPath: name,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!path) return;
    try {
      await api.saveText(path, make());
      saved = path.split(/[\\/]/).pop() ?? path;
    } catch (e) {
      error = errorText(e);
    }
  }
</script>

<button
  class="btn"
  {disabled}
  title={t("exportcsv.save_the_results_shown")}
  onclick={exportCsv}>{t("exportcsv.export_csv")}</button
>
{#if error}
  <span class="bad" role="alert">{error}</span>
{:else if saved}
  <span class="dim">Saved {saved}</span>
{/if}
