<script lang="ts">
  import { FUNCTION_NAME } from "$lib/format";
  import { fieldIn, slidePill } from "$lib/motion";
  import { bench } from "$lib/state.svelte";
  import type { ByteOrder } from "$lib/types";
  import IntInput from "./IntInput.svelte";
  import MapPicker from "./MapPicker.svelte";
  import ScanPanel from "./ScanPanel.svelte";
  import StepCard from "./StepCard.svelte";
  import Select from "./Select.svelte";

  const orders: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const locked = $derived(bench.running);
  const maxQuantity = $derived(
    bench.fc <= 2 ? 2000 : bench.fc === 15 ? 1968 : bench.fc === 16 ? 123 : 125,
  );
  const single = $derived(bench.fc === 5 || bench.fc === 6);
  let scanOpen = $state(false);

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
    <button
      class="btn"
      title={bench.transport === "rtu"
        ? "Find which unit IDs answer, or the serial settings of a device"
        : "Find which unit IDs answer on this connection"}
      disabled={locked}
      onclick={() => (scanOpen = !scanOpen)}>Find…</button
    >

    <div class="field">
      <span>Source</span>
      <div
        class="segmented"
        role="radiogroup"
        aria-label="Source"
        use:slidePill
      >
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
      <label in:fieldIn class="field">
        <span>Command</span>
        <Select
          bind:value={bench.fc}
          width="250px"
          label="Command"
          disabled={locked}
          options={Object.entries(FUNCTION_NAME).map(([code, name]) => ({
            value: Number(code),
            label: `${name} (${code.padStart(2, "0")})`,
          }))}
          onchange={onFunction}
        />
      </label>
      <label in:fieldIn class="field">
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
      <label in:fieldIn class="field">
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
      <div in:fieldIn class="field">
        <span>Map</span>
        <MapPicker disabled={locked} />
      </div>
      <label
        in:fieldIn
        class="field"
        title="Read every value with another byte order"
      >
        <span>Byte order</span>
        <Select
          value={bench.orderOverride ?? ""}
          label="Byte order"
          options={[
            {
              value: "",
              label: `From map${bench.map ? ` (${bench.map.map.meta.byte_order ?? "ABCD"})` : ""}`,
            },
            ...orders.map((o) => ({ value: o, label: o })),
          ]}
          onchange={(v) =>
            bench.setOrderOverride((v || null) as ByteOrder | null)}
        />
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
      {#if bench.loop}
        <label
          class="every"
          in:fieldIn
          title="Time from the start of one cycle to the start of the next. 0 repeats as soon as Delay allows. Can be changed while running."
        >
          every
          <input
            class="input num"
            type="number"
            min="0"
            step="100"
            value={bench.loopMs}
            onchange={(e) => {
              bench.setLoopMs(Number(e.currentTarget.value));
              e.currentTarget.value = String(bench.loopMs);
            }}
            aria-label="Time between loops, in milliseconds"
          />
          ms
        </label>
      {/if}
    </div>
  </div>
  {#if scanOpen}<ScanPanel onclose={() => (scanOpen = false)} />{/if}
</StepCard>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .options {
    margin-left: auto;
    height: 30px;
    display: flex;
    align-items: center;
  }

  .loop {
    gap: 9px;
  }

  .every {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: 12px;
    color: var(--text-2);
  }

  .every input {
    width: 84px;
  }
</style>
