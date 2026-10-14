<script lang="ts">
  import "../app.css";
  import { settings } from "$lib/settings.svelte";

  $effect(() => settings.applyTheme());
  $effect(() => {
    settings.applyTextScale();
  });

  let { children } = $props();
</script>

{@render children()}
