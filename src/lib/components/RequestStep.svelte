<script lang="ts">
  import { FUNCTION_NAME } from "$lib/format";
  import { bench } from "$lib/state.svelte";
  import type { ByteOrder } from "$lib/types";
  import IntInput from "./IntInput.svelte";
  import MapPicker from "./MapPicker.svelte";
  import StepCard from "./StepCard.svelte";

  const orders: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const locked = $derived(bench.running);
  const maxQuantity = $derived(
    bench.fc <= 2 ? 2000 : bench.fc === 15 ? 1968 : bench.fc === 16 ? 123 : 125,
  );
  const single = $derived(bench.fc === 5 || bench.fc === 6);

  function setSource(source: "manual" | "map") {
    if (bench.source === source) return;
    bench.source = source;
    bench.tab = source === "map" ? "map" : "registers";
  }

  function onFunction() {
    if (single) bench.setQuantity(1);
    else if (bench.quantity > maxQuantity) bench.setQuantity(maxQuantity);
  }
</script>

<StepCard step={2} title="Request">
  <div class="row">
    <label class="field">
      <span>Device</span>
      <IntInput
        value={bench.unitId}
        min={0}
        max={255}
        width="72px"
        label="Device (unit ID)"
        disabled={locked}
        onchange={(n) => (bench.unitId = n)}
      />
    </label>

    <div class="field">
      <span>Source</span>
      <div class="segmented" role="radiogroup" aria-label="Source">
        <button
          role="radio"
          aria-checked={bench.source === "manual"}
          class:on={bench.source === "manual"}
          disabled={locked}
          onclick={() => setSource("manual")}>Manual request</button
        >
        <button
          role="radio"
          aria-checked={bench.source === "map"}
          class:on={bench.source === "map"}
          disabled={locked}
          onclick={() => setSource("map")}>Device map</button
        >
      </div>
    </div>

    {#if bench.source === "manual"}
      <label class="field">
        <span>Command</span>
        <select
          class="input command"
          bind:value={bench.fc}
          onchange={onFunction}
          disabled={locked}
        >
          {#each Object.entries(FUNCTION_NAME) as [code, name] (code)}
            <option value={Number(code)}
              >{name} ({code.padStart(2, "0")})</option
            >
          {/each}
        </select>
      </label>
      <label class="field">
        <span
          >{bench.fc <= 2 || bench.fc === 5 || bench.fc === 15
            ? "Address"
            : "Register"}</span
        >
        <IntInput
          value={bench.address}
          min={0}
          max={65535}
          label="Start address"
          disabled={locked}
          onchange={(n) => (bench.address = n)}
        />
      </label>
      <label class="field">
        <span>Quantity</span>
        <IntInput
          value={bench.quantity}
          min={1}
          max={maxQuantity}
          width="80px"
          label="Quantity"
          disabled={locked || single}
          onchange={(n) => bench.setQuantity(n)}
        />
      </label>
    {:else}
      <div class="field">
        <span>Map</span>
        <MapPicker disabled={locked} />
      </div>
      <label class="field" title="Read every value with another byte order">
        <span>Byte order</span>
        <select
          class="input"
          value={bench.orderOverride ?? ""}
          onchange={(e) =>
            bench.setOrderOverride(
              (e.currentTarget.value || null) as ByteOrder | null,
            )}
        >
          <option value=""
            >From map{bench.map
              ? ` (${bench.map.map.meta.byte_order ?? "ABCD"})`
              : ""}</option
          >
          {#each orders as o (o)}<option value={o}>{o}</option>{/each}
        </select>
      </label>
    {/if}

    <div class="options">
      <label class="check loop"
        ><input
          type="checkbox"
          class="switch"
          role="switch"
          checked={bench.loop}
          onchange={(e) => bench.setLoop(e.currentTarget.checked)}
          disabled={locked}
        /> Loop command</label
      >
    </div>
  </div>
</StepCard>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .command {
    width: 250px;
  }

  .options {
    margin-left: auto;
    height: 34px;
    display: flex;
    align-items: center;
  }

  .loop {
    gap: 9px;
  }
</style>
