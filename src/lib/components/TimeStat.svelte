<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import {
    average,
    ms,
    suggestedTimeout,
    type ResponseTimes,
  } from "#lib/stats.ts";

  let {
    times,
    onreset,
    timeout,
    locked = false,
    onuse,
  }: {
    times: ResponseTimes;
    onreset: () => void;
    /** Timeout set in step 1, to offer a better one. */
    timeout?: number;
    /** Step 1 cannot change now (a test is running). */
    locked?: boolean;
    onuse?: (ms: number) => void;
  } = $props();

  const suggested = $derived(onuse ? suggestedTimeout(times) : null);

  const cells = $derived(
    times.count
      ? [
          { label: t("time.last"), value: ms(times.last) },
          { label: t("time.min"), value: ms(times.min) },
          { label: t("time.avg"), value: ms(average(times)) },
          { label: t("time.max"), value: ms(times.max) },
        ]
      : [
          { label: t("time.last"), value: "—" },
          { label: t("time.min"), value: "—" },
          { label: t("time.avg"), value: "—" },
          { label: t("time.max"), value: "—" },
        ],
  );
</script>

<div class="stat" title={t("time.hint")}>
  <span class="label">{t("time.title")}</span>
  <div class="body">
    <span class="lcd">
      {#each cells as c (c.label)}
        <span class="cell" aria-label="{c.label}: {c.value} ms"
          ><span class="k">{c.label}</span>{c.value}</span
        >
      {/each}
    </span>
    <button
      class="reset"
      title={t("time.reset")}
      aria-label={t("time.reset")}
      onclick={onreset}
    >
      <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"
        ><path
          d="M2.5 8a5.5 5.5 0 1 0 1.6-3.9M2.5 2.5v3h3"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        /></svg
      >
    </button>
    {#if suggested !== null}
      <button
        class="use"
        disabled={locked || suggested === timeout}
        title={t("time.suggested", {
          ms: ms(times.max),
          note:
            suggested === timeout
              ? t("time.used")
              : locked
                ? t("time.locked")
                : t("time.click"),
        })}
        onclick={() => onuse?.(suggested)}
        >{t("time.use", { ms: suggested })}</button
      >
    {/if}
  </div>
</div>

<style>
  .stat {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .label {
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .body {
    display: flex;
    gap: 4px;
  }

  /* Same sunken readout as the counters, split into four small values */
  .lcd {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    height: 30px;
    padding: 0 11px;
    border-radius: 9px;
    background: var(--lcd);
    color: var(--text);
    font-family: var(--mono);
    font-size: 14px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .cell {
    display: inline-flex;
    align-items: baseline;
    gap: 4px;
    min-width: 52px;
  }

  .k {
    font-family: var(--ui);
    font-size: 10px;
    font-weight: 500;
    color: var(--text-3);
  }

  .reset {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 9px;
    background: rgb(var(--fg-rgb) / 0.06);
    color: var(--text-3);
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      color 0.14s var(--ease);
  }

  .use {
    height: 30px;
    padding: 0 10px;
    border: 0;
    border-radius: 9px;
    background: rgb(var(--fg-rgb) / 0.06);
    color: var(--text-2);
    font: inherit;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    cursor: pointer;
    transition:
      background-color 0.14s var(--ease),
      color 0.14s var(--ease);
  }

  .use:hover:not(:disabled) {
    color: var(--text);
    background: rgb(var(--fg-rgb) / 0.11);
  }

  .use:disabled {
    color: var(--text-3);
    cursor: default;
  }

  .reset:hover {
    color: var(--text);
    background: rgb(var(--fg-rgb) / 0.11);
  }
</style>
