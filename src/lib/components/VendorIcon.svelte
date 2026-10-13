<script lang="ts" module>
  import { api } from "$lib/api";

  // One request per website for the whole session; the backend also caches
  // icons on disk, so this only saves round trips.
  const icons = new Map<string, Promise<string | null>>();

  function iconFor(website: string): Promise<string | null> {
    const key = website.trim().toLowerCase();
    let p = icons.get(key);
    if (!p) {
      p = api.vendorIcon(key).catch(() => null);
      icons.set(key, p);
    }
    return p;
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
