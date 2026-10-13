<script lang="ts">
  import { parseInteger } from "$lib/format";

  let {
    value,
    min,
    max,
    onchange,
    disabled = false,
    label,
    width = "96px",
  }: {
    value: number;
    min: number;
    max: number;
    onchange: (n: number) => void;
    disabled?: boolean;
    label?: string;
    width?: string;
  } = $props();

  let text = $state("");
  let invalid = $state(false);

  // Show the current value whenever it changes from outside.
  $effect(() => {
    text = String(value);
    invalid = false;
  });

  function commit() {
    const n = parseInteger(text);
    if (n === null || n < min || n > max) {
      invalid = true;
      return;
    }
    invalid = false;
    if (n !== value) onchange(n);
    else text = String(n);
  }
</script>

<input
  class="input num"
  class:invalid
  style:width
  bind:value={text}
  onchange={commit}
  onkeydown={(e) => e.key === "Enter" && commit()}
  {disabled}
  spellcheck="false"
  aria-label={label}
  aria-invalid={invalid}
  title={invalid ? `Enter ${min}–${max} (decimal, 0x… or …h)` : undefined}
/>

<style>
  .invalid {
    box-shadow: 0 0 0 2px rgb(var(--alert-rgb) / 0.55);
  }
</style>
