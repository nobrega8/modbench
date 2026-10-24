<script lang="ts">
  // Placeholder rows shown while a list loads, shaped like the rows that will
  // replace them, with a slow light sweep. After EasyUI's Skeleton.
  import { t } from "#lib/i18n.svelte.ts";

  let {
    rows = 6,
    icon = false,
    label = t("mapexplorer.loading"),
  }: {
    rows?: number;
    /** Rows start with a square icon (a vendor logo). */
    icon?: boolean;
    /** Read out instead of the placeholders. */
    label?: string;
  } = $props();

  // Varied widths, so the rows read as text rather than as a grid.
  const WIDTHS = [62, 44, 71, 53, 38, 66, 49, 58];
</script>

<div class="skeleton" role="status" aria-busy="true" aria-label={label}>
  {#each { length: rows } as _, i (i)}
    <div class="row" aria-hidden="true">
      {#if icon}<span class="bone icon"></span>{/if}
      <span class="bone" style:width="{WIDTHS[i % WIDTHS.length]}%"></span>
      <span class="bone end"></span>
    </div>
  {/each}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px;
    height: 48px;
  }

  .row + .row {
    box-shadow: inset 0 1px 0 var(--border);
  }

  .bone {
    position: relative;
    overflow: hidden;
    height: 12px;
    border-radius: 6px;
    background: rgb(var(--fg-rgb) / 0.06);
  }

  .icon {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
  }

  .end {
    width: 56px;
    margin-left: auto;
  }

  .bone::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgb(var(--fg-rgb) / 0.06),
      transparent
    );
    transform: translateX(-100%);
    animation: sweep 1.8s ease-in-out infinite;
  }

  @keyframes sweep {
    to {
      transform: translateX(100%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .bone::after {
      animation: none;
    }
  }
</style>
