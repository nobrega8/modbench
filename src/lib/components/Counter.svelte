<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import DigitRoll from "./DigitRoll.svelte";
  let {
    label,
    value,
    tone,
    onreset,
  }: {
    label: string;
    value: number;
    tone: "ok" | "bad" | "warn";
    onreset: () => void;
  } = $props();

  const digits = $derived(String(Math.min(value, 9_999_999)).padStart(7, "0"));
  const lead = $derived(value === 0 ? 6 : 7 - String(value).length);
</script>

<div class="counter {tone}" class:hot={value > 0}>
  <span class="label">{label}</span>
  <div class="body">
    <span class="lcd" aria-label="{label}: {value}"
      ><DigitRoll text={digits} dim={lead} /></span
    >
    <button
      class="reset"
      title={t("counter.reset", { label })}
      aria-label={t("counter.reset", { label })}
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
  </div>
</div>

<style>
  /* Each counter carries its colour: green valid, red errors, amber timeouts */
  .counter {
    display: flex;
    flex-direction: column;
    gap: 5px;
    --tint: var(--fg-rgb);
  }

  .ok {
    --tint: 22 150 82;
  }

  .bad {
    --tint: var(--alert-rgb);
  }

  .warn {
    --tint: 206 146 0;
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

  /* A sunken readout, tinted with the counter's colour; the digits take the
     colour once it counts */
  .lcd {
    display: inline-flex;
    align-items: center;
    height: 30px;
    padding: 0 11px;
    border-radius: 9px;
    background: rgb(var(--tint) / 0.12);
    color: var(--text);
    font-family: var(--mono);
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.06em;
    font-variant-numeric: tabular-nums;
  }

  .hot .lcd {
    color: rgb(var(--tint));
  }

  .hot.bad .label,
  .hot.warn .label {
    color: rgb(var(--tint));
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

  .reset:hover {
    color: var(--text);
    background: rgb(var(--fg-rgb) / 0.11);
  }
</style>
