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

  let { vendor, website, size = 22 }: Props = $props();

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

  /* Favicons are drawn for any background: give them a white tile */
  .icon:has(img) {
    background: #fff;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.06);
  }

  img {
    width: 75%;
    height: 75%;
    object-fit: contain;
  }
</style>
