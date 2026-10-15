<!--
  A field for a wire address (0-based). It also takes Modicon (400101) and
  IEC 61131-3 (%MW100) addresses, which name their area: `onchange` gets
  that area too, so the field next to it (function, area) can follow.
-->
<script lang="ts">
  import { parseAddress } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import type { Area } from "#lib/types.ts";

  let {
    value,
    onchange,
    areas,
    disabled = false,
    label,
    width = "96px",
  }: {
    value: number;
    onchange: (address: number, area: Area | null) => void;
    /** Areas the field can take; an address in another one is refused. */
    areas?: Area[];
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
    const p = parseAddress(text, settings.addressFormat);
    if (
      p === null ||
      p.address < 0 ||
      p.address > 0xffff ||
      (p.area !== null && areas && !areas.includes(p.area))
    ) {
      invalid = true;
      return;
    }
    invalid = false;
    text = String(p.address);
    if (p.address !== value || p.area !== null) onchange(p.address, p.area);
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
  title={invalid
    ? `Enter 0–65535 (decimal, 0x… or …h), or 400101 / %MW100${areas ? ` for ${areas.join(" or ")}` : ""}`
    : "Address on the wire (0-based). 400101 or %MW100 also set the area."}
/>

<style>
  .invalid {
    box-shadow: 0 0 0 2px rgb(var(--alert-rgb) / 0.55);
  }
</style>
