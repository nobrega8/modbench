<!--
  The x_status of a map as a coloured icon. Shows nothing for a map without
  a status or with one this version does not know.
-->
<script lang="ts" module>
  const STATUS = {
    verified: "Verified: tested on a real device.",
    unverified:
      "Unverified: written from an official source (the manufacturer's manual or protocol document), not tested on a device.",
    unofficial:
      "Unofficial: copied from an unofficial source (another catalogue, a forum, a third-party integration), not tested.",
  } as const;
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  let { status, size = 15 }: { status?: string | null; size?: number } =
    $props();

  const known = $derived(
    status && Object.hasOwn(STATUS, status)
      ? (status as keyof typeof STATUS)
      : null,
  );
</script>

{#if known}
  <span
    class="status {known}"
    role="img"
    aria-label={STATUS[known]}
    title={STATUS[known]}><Icon name={known} {size} /></span
  >
{/if}

<style>
  .status {
    display: inline-flex;
    flex: none;
  }
  .verified {
    color: var(--status-verified);
  }
  .unverified {
    color: var(--status-unverified);
  }
  .unofficial {
    color: var(--status-unofficial);
  }
</style>
