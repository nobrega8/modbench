<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { changes, summary } from "#lib/release-notes.ts";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { settings } from "#lib/settings.svelte.ts";
  import { APT_UPGRADE, updates } from "#lib/updates.svelte.ts";

  const update = $derived(updates.available);
  let dismissed = $state(false);
  let downloading = $state(false);
  let progress = $state<number | null>(null);
  let failure = $state<string | null>(null);

  $effect(() => {
    if (settings.checkUpdates) updates.check(true);
  });

  async function install() {
    if (!update) return;
    failure = null;
    let total = 0;
    let done = 0;
    downloading = true;
    progress = null;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        else if (event.event === "Progress") {
          done += event.data.chunkLength;
          progress = total ? Math.round((done / total) * 100) : null;
        }
      });
      await relaunch();
    } catch (e) {
      downloading = false;
      failure = String(e);
    }
  }
</script>

{#if update && !dismissed}
  <div class="banner" role="status">
    <strong>Modbench {update.version} is available.</strong>
    {#if updates.viaApt}
      <span>Update with <span class="mono">{APT_UPGRADE}</span></span>
    {:else if failure}
      <span class="bad">The update failed: {failure}</span>
    {:else if downloading}
      <span>Downloading…{progress === null ? "" : ` ${progress}%`}</span>
    {:else if update.body}
      <span class="notes" title={changes(update.body).join("\n")}
        >{summary(update.body)}</span
      >
    {/if}
    <span class="spacer"></span>
    {#if updates.viaApt}
      <button class="btn" onclick={() => (dismissed = true)}
        >{t("action.dismiss")}</button
      >
    {:else}
      <button class="btn accent" disabled={downloading} onclick={install}
        >{t("updatebanner.install_and_restart")}</button
      >
      <button
        class="btn"
        disabled={downloading}
        onclick={() => (dismissed = true)}>{t("updatebanner.later")}</button
      >
    {/if}
  </div>
{/if}

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    border-radius: 14px;
    background: var(--card);
    box-shadow: var(--e2);
  }

  .notes {
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 480px;
  }

  .spacer {
    flex: 1;
  }

  .bad {
    color: var(--bad);
  }
</style>
