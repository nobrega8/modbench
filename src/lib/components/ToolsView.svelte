<script lang="ts">
  import { viewIn } from "$lib/motion";
  import { sim } from "$lib/simulator.svelte";
  import { bench, type ToolId } from "$lib/state.svelte";
  import AddressScanTool from "./tools/AddressScanTool.svelte";
  import AlarmsTool from "./tools/AlarmsTool.svelte";
  import DataDecoderTool from "./tools/DataDecoderTool.svelte";
  import DeviceIdTool from "./tools/DeviceIdTool.svelte";
  import FrameDecoderTool from "./tools/FrameDecoderTool.svelte";
  import SimulatorTool from "./tools/SimulatorTool.svelte";
  import StatsTool from "./tools/StatsTool.svelte";
  import TrendPlayerTool from "./tools/TrendPlayerTool.svelte";

  const sections: {
    title: string;
    tools: { id: ToolId; name: string; hint: string }[];
  }[] = [
    {
      title: "Device",
      tools: [
        {
          id: "identify",
          name: "Identify",
          hint: "Ask the device who it is (FC43)",
        },
        {
          id: "addresses",
          name: "Find addresses",
          hint: "Which registers answer",
        },
        {
          id: "simulator",
          name: "Simulator",
          hint: "A Modbus TCP device on this computer",
        },
      ],
    },
    {
      title: "Data",
      tools: [
        {
          id: "decoder",
          name: "Data decoder",
          hint: "Hex as numbers, floats and text",
        },
        {
          id: "frames",
          name: "Frame decoder",
          hint: "A frame, field by field",
        },
        {
          id: "player",
          name: "Trend player",
          hint: "Play back a recording",
        },
      ],
    },
    {
      title: "Test",
      tools: [
        {
          id: "stats",
          name: "Statistics",
          hint: "Failures by cause and function",
        },
        {
          id: "alarms",
          name: "Alarms",
          hint: "Limits on map registers",
        },
      ],
    },
  ];

  /** A dot next to tools that have something going on. */
  function busy(id: ToolId): boolean {
    if (id === "simulator") return sim.running !== null;
    if (id === "addresses")
      return bench.scanning && bench.addressProgress !== null;
    if (id === "alarms") return bench.alarms.length > 0;
    return false;
  }
</script>

<div class="tools">
  <nav aria-label="Tools">
    {#each sections as s (s.title)}
      <h2 class="key">{s.title}</h2>
      {#each s.tools as t (t.id)}
        <button
          class:on={bench.tool === t.id}
          aria-current={bench.tool === t.id ? "page" : undefined}
          onclick={() => (bench.tool = t.id)}
        >
          <span class="name"
            >{t.name}{#if busy(t.id)}<span class="dot" aria-hidden="true"
              ></span>{/if}</span
          >
          <span class="hint">{t.hint}</span>
        </button>
      {/each}
    {/each}
  </nav>

  {#key bench.tool}
    <div class="panel" in:viewIn>
      {#if bench.tool === "simulator"}
        <SimulatorTool />
      {:else if bench.tool === "identify"}
        <DeviceIdTool />
      {:else if bench.tool === "addresses"}
        <AddressScanTool />
      {:else if bench.tool === "decoder"}
        <DataDecoderTool />
      {:else if bench.tool === "frames"}
        <FrameDecoderTool />
      {:else if bench.tool === "stats"}
        <StatsTool />
      {:else if bench.tool === "alarms"}
        <AlarmsTool />
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
    grid-template-columns: 220px 1fr;
    gap: 22px;
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
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
    padding: 7px 10px;
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
  }

  .name {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 500;
  }

  .hint {
    font-size: 12px;
    color: var(--text-3);
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
    gap: 14px;
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
    background: var(--accent);
    transition: width 0.2s var(--ease);
  }
</style>
