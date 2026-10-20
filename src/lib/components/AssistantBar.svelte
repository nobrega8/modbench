<!--
  Shown while an assistant is connected through the GUI control channel (see
  gui-control.svelte.ts): that it is there, one click to pause it, and what it
  changed and when.
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { assistant } from "#lib/gui-control.svelte.ts";
  import { clock } from "#lib/format.ts";

  let open = $state(false);
</script>

{#if assistant.clients > 0 || assistant.paused}
  <section class="bar" class:paused={assistant.paused} role="status">
    <div class="row">
      <strong
        >{assistant.paused
          ? t("assistant.paused")
          : t("assistant.connected")}</strong
      >
      <span class="grow"></span>
      <button class="btn" onclick={() => (open = !open)} aria-expanded={open}
        >{t("assistant.activity")}</button
      >
      <button
        class="btn"
        class:accent={!assistant.paused}
        onclick={() => assistant.setPaused(!assistant.paused)}
        >{assistant.paused
          ? t("assistant.resume")
          : t("assistant.pause")}</button
      >
      <button class="btn" onclick={() => assistant.disconnect()}
        >{t("assistant.disconnect")}</button
      >
    </div>
    {#if open}
      <ul>
        {#each assistant.activity as a (a.time + a.text)}
          <li class:refused={a.refused}>
            <span class="time">{clock(a.time).slice(0, 8)}</span>
            {a.text}
          </li>
        {:else}
          <li class="dim">{t("assistant.no_activity")}</li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .bar {
    flex-shrink: 0;
    padding: 10px 16px;
    border-radius: 14px;
    background: var(--card);
    box-shadow:
      var(--e2),
      inset 0 0 0 2px var(--accent);
  }

  .bar.paused {
    box-shadow: var(--e2);
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .grow {
    flex: 1;
  }

  ul {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
    max-height: 160px;
    overflow: auto;
    font-size: 13px;
    color: var(--text-2);
  }

  li {
    padding: 2px 0;
  }

  li.refused {
    color: var(--bad);
  }

  .time {
    margin-right: 8px;
    font-family: var(--mono);
    color: var(--text-3);
  }

  .dim {
    color: var(--text-3);
  }
</style>
