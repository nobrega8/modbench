<script lang="ts">
  import "../app.css";
  import ContextMenu from "#lib/components/ContextMenu.svelte";
  import { settings } from "#lib/settings.svelte.ts";

  $effect(() => settings.applyTheme());
  $effect(() => {
    settings.applyTextScale();
  });

  let { children } = $props();
</script>

{@render children()}
<ContextMenu />
