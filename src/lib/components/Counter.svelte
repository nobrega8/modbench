<script lang="ts">
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

<div class="counter {tone}" class:hot={tone !== "ok" && value > 0}>
  <span class="label">{label}</span>
  <div class="body">
    <span class="lcd" aria-label="{label}: {value}"
      ><span class="off">{digits.slice(0, lead)}</span>{digits.slice(
        lead,
      )}</span
    >
    <button
      class="reset"
      title="Reset {label.toLowerCase()}"
      aria-label="Reset {label.toLowerCase()}"
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
  .counter {
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

  /* A sunken readout; errors and timeouts turn red only when they count */
  .lcd {
    display: inline-flex;
    align-items: center;
    height: 34px;
    padding: 0 11px;
    border-radius: 9px;
    background: var(--lcd);
    color: var(--text);
    font-family: var(--mono);
    font-size: 16px;
    font-weight: 600;
    letter-spacing: 0.06em;
    font-variant-numeric: tabular-nums;
  }

  .hot .label,
  .hot .lcd {
    color: rgb(var(--alert-rgb));
  }

  .off {
    opacity: 0.25;
  }

  .reset {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
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
