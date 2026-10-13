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

<div class="counter {tone}">
  <span class="label">{label}</span>
  <div class="body">
    <span class="lcd" aria-label="{label}: {value}"
      ><span class="off">{digits.slice(0, lead)}</span>{digits.slice(
        lead,
      )}</span
    >
    <button class="reset" title="Reset {label.toLowerCase()}" onclick={onreset}
      >Reset</button
    >
  </div>
</div>

<style>
  .counter {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .label {
    font-size: 12px;
    font-weight: 600;
  }

  .ok .label {
    color: var(--ok);
  }
  .bad .label {
    color: var(--bad);
  }
  .warn .label {
    color: var(--warn);
  }

  .body {
    display: flex;
    gap: 4px;
  }

  .lcd {
    display: inline-flex;
    align-items: center;
    height: 30px;
    padding: 0 9px;
    border-radius: 4px;
    background: var(--lcd);
    font-family: var(--mono);
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.08em;
    font-variant-numeric: tabular-nums;
  }

  .ok .lcd {
    color: #4ee66a;
  }
  .bad .lcd {
    color: #ff5a4a;
  }
  .warn .lcd {
    color: #ffd23f;
  }

  .off {
    opacity: 0.22;
  }

  .reset {
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    background: var(--card);
    font-size: 12px;
    color: var(--text-2);
    cursor: pointer;
  }

  .reset:hover {
    color: var(--text);
    background: var(--subtle);
  }
</style>
