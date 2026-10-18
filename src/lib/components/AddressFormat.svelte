<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { ADDRESS_FORMATS } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import Select from "./Select.svelte";
</script>

<label class="inline" title={t("addressformat.how_addresses_are_shown")}>
  <span>{t("addressformat.addresses")}</span>
  <Select
    bind:value={settings.addressFormat}
    options={ADDRESS_FORMATS.map((f) => ({ value: f.id, label: f.label }))}
    label={t("addressformat.addresses")}
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
