<script lang="ts">
  import { t, type Key } from "#lib/i18n.svelte.ts";
  import { viewIn } from "#lib/motion.ts";
  import Icon from "./Icon.svelte";
  import { netScanner } from "#lib/netscan.svelte.ts";
  import { sim } from "#lib/simulator.svelte.ts";
  import { gateway } from "#lib/gateway.svelte.ts";
  import { bench, type ToolId } from "#lib/state.svelte.ts";
  import AddressScanTool from "./tools/AddressScanTool.svelte";
  import AlarmsTool from "./tools/AlarmsTool.svelte";
  import DataDecoderTool from "./tools/DataDecoderTool.svelte";
  import DeviceIdTool from "./tools/DeviceIdTool.svelte";
  import FrameDecoderTool from "./tools/FrameDecoderTool.svelte";
  import NetworkScanTool from "./tools/NetworkScanTool.svelte";
  import SendRequestTool from "./tools/SendRequestTool.svelte";
  import SequenceTool from "./tools/SequenceTool.svelte";
  import SimulatorTool from "./tools/SimulatorTool.svelte";
  import GatewayTool from "./tools/GatewayTool.svelte";
  import StatsTool from "./tools/StatsTool.svelte";
  import TrendPlayerTool from "./tools/TrendPlayerTool.svelte";

  /** Shows only this tool, without the list (the simulator's window). */
  let { only }: { only?: ToolId } = $props();
  const tool = $derived(only ?? bench.tool);

  const sections: {
    title: Key;
    tools: { id: ToolId }[];
  }[] = [
    {
      title: "tools.device",
      tools: [
        { id: "identify" },
        { id: "addresses" },
        { id: "network" },
        { id: "simulator" },
        { id: "gateway" },
      ],
    },
    {
      title: "tools.data",
      tools: [{ id: "decoder" }, { id: "frames" }, { id: "player" }],
    },
    {
      title: "tools.test",
      tools: [
        { id: "request" },
        { id: "stats" },
        { id: "alarms" },
        { id: "sequence" },
      ],
    },
  ];

  /** A dot next to tools that have something going on. */
  function busy(id: ToolId): boolean {
    if (id === "simulator") return sim.running !== null;
    if (id === "gateway") return gateway.running !== null;
    if (id === "addresses")
      return bench.scanning && bench.addressProgress !== null;
    if (id === "alarms") return bench.alarms.length > 0;
    if (id === "network") return netScanner.running;
    return false;
  }
</script>

<div class="tools" class:alone={only}>
  {#if !only}
    <nav aria-label={t("nav.tools")}>
      {#each sections as s (s.title)}
        <h2 class="key">{t(s.title)}</h2>
        {#each s.tools as tool (tool.id)}
          <button
            title={t(`tools.${tool.id}.hint` as Key)}
            class:on={bench.tool === tool.id}
            aria-current={bench.tool === tool.id ? "page" : undefined}
            onclick={() => (bench.tool = tool.id)}
          >
            <Icon name={tool.id} size={16} />
            <span class="name"
              >{t(`tools.${tool.id}` as Key)}{#if busy(tool.id)}<span
                  class="dot"
                  aria-hidden="true"
                ></span>{/if}</span
            >
          </button>
        {/each}
      {/each}
    </nav>
  {/if}

  {#key tool}
    <div class="panel" in:viewIn>
      {#if tool === "simulator"}
        <SimulatorTool />
      {:else if tool === "gateway"}
        <GatewayTool />
      {:else if tool === "identify"}
        <DeviceIdTool />
      {:else if tool === "addresses"}
        <AddressScanTool />
      {:else if tool === "network"}
        <NetworkScanTool />
      {:else if tool === "request"}
        <SendRequestTool />
      {:else if tool === "decoder"}
        <DataDecoderTool />
      {:else if tool === "frames"}
        <FrameDecoderTool />
      {:else if tool === "stats"}
        <StatsTool />
      {:else if tool === "alarms"}
        <AlarmsTool />
      {:else if tool === "sequence"}
        <SequenceTool />
      {:else}
        <TrendPlayerTool />
      {/if}
    </div>
  {/key}
</div>

<style>
  .tools {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 184px 1fr;
    gap: 20px;
  }

  .tools.alone {
    grid-template-columns: 1fr;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
  }

  h2 {
    margin: 14px 0 6px 10px;
  }

  h2:first-child {
    margin-top: 0;
  }

  nav button {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 6px 10px;
    color: var(--text-2);
    border: 0;
    border-radius: 9px;
    background: none;
    text-align: left;
    cursor: pointer;
    transition: background-color 0.14s var(--ease);
  }

  nav button:hover {
    background: rgb(var(--fg-rgb) / 0.05);
  }

  nav button.on {
    background: var(--subtle);
    color: var(--text);
  }

  .name {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 500;
    color: var(--text);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text);
  }

  .panel {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow: auto;
  }

  /* Shared by the tools: a title over a short explanation */
  .panel :global(.tool-head) {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .panel :global(.tool-head h1) {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: -0.011em;
  }

  .panel :global(.tool-head p) {
    margin: 0;
    color: var(--text-2);
    max-width: 70ch;
  }

  .panel :global(.row) {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .panel :global(.dim) {
    color: var(--text-3);
  }

  .panel :global(.bad) {
    color: var(--bad);
  }

  .panel :global(.empty) {
    color: var(--text-2);
    margin: 0;
  }

  /* A plain data table */
  .panel :global(table.data) {
    border-collapse: collapse;
    width: 100%;
  }

  .panel :global(table.data th) {
    text-align: left;
    padding: 6px 10px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }

  .panel :global(table.data td) {
    padding: 6px 10px;
    border-top: 1px solid var(--border);
    vertical-align: top;
  }

  .panel :global(table.data td.num),
  .panel :global(table.data th.num) {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .panel :global(.bar) {
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    overflow: hidden;
  }

  .panel :global(.bar > div) {
    height: 100%;
    background: var(--tx);
    transition: width 0.2s var(--ease);
  }
</style>
