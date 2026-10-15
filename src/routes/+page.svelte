<!--
  One page, two windows: the app itself, or the simulator popped out into a
  window of its own (opened with ?window=simulator).
-->
<script lang="ts">
  import MainApp from "$lib/components/MainApp.svelte";
  import SimWindow from "$lib/components/SimWindow.svelte";
  import { IN_SIM_WINDOW } from "$lib/simulator.svelte";
</script>

{#if IN_SIM_WINDOW}
  <SimWindow />
{:else}
  <MainApp />
{/if}
