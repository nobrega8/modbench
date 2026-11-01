<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { api } from "#lib/api.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { LocalInterface } from "#lib/types.ts";
  import Select from "./Select.svelte";

  /** Which local address TCP and UDP connections (and the network scan)
   * send from: the system's choice, or one of this computer's interfaces. */
  let { disabled = false }: { disabled?: boolean } = $props();

  let list = $state<LocalInterface[]>([]);

  async function refresh() {
    try {
      list = await api.localInterfaces();
    } catch {
      list = [];
    }
  }

  $effect(() => {
    refresh();
  });

  const options = $derived([
    { value: "", label: t("conn.auto") },
    ...list.map((i) => ({
      value: i.address,
      label: `${i.name} · ${i.address}`,
    })),
    // A saved address whose interface is gone stays visible, marked
    ...(bench.localAddress &&
    !list.some((i) => i.address === bench.localAddress)
      ? [
          {
            value: bench.localAddress,
            label: t("conn.notFound", { address: bench.localAddress }),
          },
        ]
      : []),
  ]);
</script>

<label class="field" title={t("conn.interface.hint")}>
  <span>{t("conn.sendFrom")}</span>
  <Select
    bind:value={bench.localAddress}
    {options}
    {disabled}
    label={t("conn.networkInterface")}
    width="170px"
  />
</label>
