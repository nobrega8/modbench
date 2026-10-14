<script lang="ts">
  import { ADDRESS_FORMATS } from "$lib/format";
  import { settings } from "$lib/settings.svelte";
</script>

<label
  class="inline"
  title="How addresses are shown. Fields where you type an address always take the address sent on the wire (0-based)."
>
  <span>Addresses</span>
  <select
    class="input"
    bind:value={settings.addressFormat}
    onchange={() => settings.save()}
  >
    {#each ADDRESS_FORMATS as f (f.id)}<option value={f.id}>{f.label}</option
      >{/each}
  </select>
</label>

<style>
  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
</style>
