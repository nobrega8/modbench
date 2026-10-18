<!--
  The x_status of a map as a coloured icon. Shows nothing for a map without
  a status or with one this version does not know.
-->
<script lang="ts" module>
  import { t } from "#lib/i18n.svelte.ts";
  const STATUS_KEYS = ["verified", "official", "unofficial"] as const;
  const statusText = (s: (typeof STATUS_KEYS)[number]) =>
    ({
      verified: t("mapstatus.verified_tested_on_a"),
      official: t("mapstatus.official_written_from_an"),
      unofficial: t("mapstatus.unofficial_copied_from_an"),
    })[s];

  /** The x_status of a map, with "unverified" (the older name of
      "official") read as "official". */
  export function mapStatus(status?: string | null): string | null {
    return status === "unverified" ? "official" : (status ?? null);
  }
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  let { status, size = 15 }: { status?: string | null; size?: number } =
    $props();

  const known = $derived.by(() => {
    const s = mapStatus(status);
    return s && (STATUS_KEYS as readonly string[]).includes(s)
      ? (s as (typeof STATUS_KEYS)[number])
      : null;
  });
</script>

{#if known}
  <span
    class="status {known}"
    role="img"
    aria-label={statusText(known)}
    title={statusText(known)}><Icon name={known} {size} /></span
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
  .official {
    color: var(--status-official);
  }
  .unofficial {
    color: var(--status-unofficial);
  }
</style>
