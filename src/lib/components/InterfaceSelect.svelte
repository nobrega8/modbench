<script lang="ts">
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
    { value: "", label: "Automatic" },
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
            label: `${bench.localAddress} (not found)`,
          },
        ]
      : []),
  ]);
</script>

<label
  class="field"
  title="The network interface to send from. Automatic leaves it to the system's routing; choose one on a computer with several networks (Ethernet, Wi-Fi, VPN)."
>
  <span>Send from</span>
  <Select
    bind:value={bench.localAddress}
    {options}
    {disabled}
    label="Network interface"
    width="190px"
  />
</label>
