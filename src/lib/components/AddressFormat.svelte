<script lang="ts">
  import { ADDRESS_FORMATS } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import Select from "./Select.svelte";
</script>

<label
  class="inline"
  title="How addresses are shown. Fields where you type an address take the address sent on the wire (0-based), or a Modicon (400101) or IEC 61131-3 (%MW100) address, which also sets the area."
>
  <span>Addresses</span>
  <Select
    bind:value={settings.addressFormat}
    options={ADDRESS_FORMATS.map((f) => ({ value: f.id, label: f.label }))}
    label="Addresses"
    onchange={() => settings.save()}
  />
</label>

<style>
  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }
</style>
