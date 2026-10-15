<script lang="ts" module>
  import { api } from "#lib/api.ts";

  // One request per website for the whole session; the backend also caches
  // icons on disk, so this only saves round trips.
  const icons = new Map<
    string,
    { promise: Promise<string | null>; retryAt: number }
  >();
  const retryMissingAfter = 60 * 60 * 1000;

  function iconFor(website: string): Promise<string | null> {
    const key = website.trim().toLowerCase();
    let cached = icons.get(key);
    if (cached?.retryAt && cached.retryAt <= Date.now()) {
      icons.delete(key);
      cached = undefined;
    }
    if (!cached) {
      const entry: { promise: Promise<string | null>; retryAt: number } = {
        promise: Promise.resolve(null),
        retryAt: 0,
      };
      entry.promise = api
        .vendorIcon(key)
        .catch(() => null)
        .then((icon) => {
          if (!icon) entry.retryAt = Date.now() + retryMissingAfter;
          return icon;
        });
      icons.set(key, entry);
      cached = entry;
    }
    return cached.promise;
  }
</script>

<script lang="ts">
  interface Props {
    vendor: string;
    website: string | null;
    size?: number;
  }

  let { vendor, website, size = 20 }: Props = $props();

  let src = $state<string | null>(null);
  let broken = $state(false);

  $effect(() => {
    src = null;
    broken = false;
    if (!website) return;
    let live = true;
    iconFor(website).then((url) => {
      if (live) src = url;
    });
    return () => {
      live = false;
    };
  });

  const letter = $derived(vendor.trim().charAt(0).toUpperCase() || "?");
</script>

<span
  class="icon"
  style:width="{size}px"
  style:height="{size}px"
  style:font-size="{Math.round(size * 0.5)}px"
  aria-hidden="true"
>
  {#if src && !broken}
    <img {src} alt="" onerror={() => (broken = true)} />
  {:else}
    {letter}
  {/if}
</span>

<style>
  .icon {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 6px;
    background: var(--subtle);
    color: var(--text-3);
    font-weight: 600;
    line-height: 1;
    overflow: hidden;
    vertical-align: middle;
  }

  /* A favicon stands on its own, with no tile behind it */
  .icon:has(img) {
    background: none;
    border-radius: 0;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
