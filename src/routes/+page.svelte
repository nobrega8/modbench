<!--
  One page, three windows: the app itself, the simulator popped out into a
  window of its own (opened with ?window=simulator), or the map builder
  (?window=builder).
-->
<script lang="ts">
  import MainApp from "#lib/components/MainApp.svelte";
  import BuilderWindow from "#lib/components/BuilderWindow.svelte";
  import SimWindow from "#lib/components/SimWindow.svelte";
  import { IN_BUILDER_WINDOW } from "#lib/builder.svelte.ts";
  import { IN_SIM_WINDOW } from "#lib/simulator.svelte.ts";
</script>

{#if IN_BUILDER_WINDOW}
  <BuilderWindow />
{:else if IN_SIM_WINDOW}
  <SimWindow />
{:else}
  <MainApp />
{/if}
