<script lang="ts">
  import "../app.css";
  import { settings } from "$lib/settings.svelte";

  $effect(() => settings.applyTheme());

  let { children } = $props();
</script>

{@render children()}
