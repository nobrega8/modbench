<!--
  What the map says about one register: where it is, how it is encoded, its
  limits, reference and labels, and its description and comment. Shown where
  a generator is made for it, so the generator fits the register.
-->
<script lang="ts">
  import { AREA_NAME, formatAddress, limitHint } from "#lib/format.ts";
  import { referenceValue } from "#lib/generators.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import type { Register } from "#lib/types.ts";

  let { register: r }: { register: Register } = $props();

  const LABELS_SHOWN = 8;

  const encoding = $derived(
    [
      r.type,
      r.bit !== null
        ? r.bits
          ? `bits ${r.bit}–${r.bit + r.bits - 1}`
          : `bit ${r.bit}`
        : "",
      r.scale !== 1 ? `× ${r.scale}` : "",
      r.offset ? `${r.offset > 0 ? "+" : "−"} ${Math.abs(r.offset)}` : "",
    ]
      .filter(Boolean)
      .join(" "),
  );
  const reference = $derived(referenceValue(r));
  const limits = $derived(limitHint(r));
  const unit = $derived(r.unit ? ` ${r.unit}` : "");
</script>

<div class="info">
  <div class="title">
    <strong>{r.name}</strong>
    {#if r.group}<span class="dim">{r.group}</span>{/if}
  </div>
  <dl>
    <dt>Where</dt>
    <dd>
      {AREA_NAME[r.area]}
      <span class="mono"
        >{formatAddress(r.address, settings.addressFormat, r.area)}</span
      >
    </dd>
    <dt>Type</dt>
    <dd class="mono">{encoding}{unit && ` → ${r.unit}`}</dd>
    {#if limits}
      <dt>Limits</dt>
      <dd>{limits}{unit}</dd>
    {/if}
    {#if r.reference}
      <dt>Reference</dt>
      <dd>
        {reference !== null && reference !== Number(r.reference)
          ? `${Number(reference.toPrecision(6))}${unit} (raw ${r.reference})`
          : `${r.reference}${unit}`}
      </dd>
    {/if}
    {#if r.enum.length}
      <dt>Values</dt>
      <dd>
        {r.enum
          .slice(0, LABELS_SHOWN)
          .map(([v, l]) => `${v} ${l}`)
          .join(" · ")}{r.enum.length > LABELS_SHOWN
          ? ` · +${r.enum.length - LABELS_SHOWN} more`
          : ""}
      </dd>
    {/if}
  </dl>
  {#if r.description}<p>{r.description}</p>{/if}
  {#if r.comment}<p class="comment">{r.comment}</p>{/if}
</div>

<style>
  .info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    font-size: 13px;
  }

  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .dim {
    color: var(--text-3);
    font-size: 12px;
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 12px;
    margin: 0;
  }

  dt {
    color: var(--text-3);
    font-size: 12px;
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  p {
    margin: 0;
    color: var(--text-2);
    max-width: 60ch;
  }

  .comment {
    font-style: italic;
  }
</style>
