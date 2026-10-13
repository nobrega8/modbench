<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { errorText } from "$lib/api";
  import { FUNCTION_NAME } from "$lib/format";
  import { bench } from "$lib/state.svelte";
  import type { ByteOrder } from "$lib/types";
  import IntInput from "./IntInput.svelte";
  import StepCard from "./StepCard.svelte";

  const orders: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const locked = $derived(bench.running);
  const maxQuantity = $derived(
    bench.fc <= 2 ? 2000 : bench.fc === 15 ? 1968 : bench.fc === 16 ? 123 : 125,
  );
  const single = $derived(bench.fc === 5 || bench.fc === 6);
  let openError = $state<string | null>(null);
  const libraryOnly = $derived(
    bench.library.filter(
      (e) => !e.error && !bench.maps.some((m) => m.path === e.path),
    ),
  );

  async function chooseMap() {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Modbus map", extensions: ["mbmap"] }],
    });
    if (typeof path !== "string") return;
    openError = null;
    try {
      await bench.openMap(path);
    } catch (e) {
      openError = errorText(e);
    }
  }

  function onMapSelect(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value;
    if (v.startsWith("lib:")) {
      bench.openMap(v.slice(4)).catch((err) => (openError = errorText(err)));
    } else if (v === "open") {
      (e.currentTarget as HTMLSelectElement).value = String(
        bench.activeMap ?? "",
      );
      chooseMap();
    } else if (v !== "") {
      bench.selectMap(Number(v));
    }
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

    <fieldset class="field source" disabled={locked}>
      <span>Source</span>
      <div class="radios">
        <label class="check"
          ><input
            type="radio"
            bind:group={bench.source}
            value="manual"
            onchange={() => (bench.tab = "registers")}
          /> Manual request</label
        >
        <label class="check"
          ><input
            type="radio"
            bind:group={bench.source}
            value="map"
            onchange={() => (bench.tab = "map")}
          /> Device map</label
        >
      </div>
    </fieldset>

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
      <label class="field">
        <span>Map</span>
        <select
          class="input map"
          value={bench.activeMap === null ? "" : String(bench.activeMap)}
          onchange={onMapSelect}
          disabled={locked}
        >
          {#if bench.activeMap === null}<option value="">Choose a map</option
            >{/if}
          {#if bench.maps.length}
            <optgroup label="Open">
              {#each bench.maps as m, i (m.path)}
                <option value={String(i)}
                  >{m.map.meta.vendor} · {m.map.meta.model} ({m.map.registers
                    .length})</option
                >
              {/each}
            </optgroup>
          {/if}
          {#if libraryOnly.length}
            <optgroup label="Library">
              {#each libraryOnly as e (e.path)}
                <option value="lib:{e.path}"
                  >{e.vendor} · {e.model} ({e.registers})</option
                >
              {/each}
            </optgroup>
          {/if}
          <option value="open">Open map file…</option>
        </select>
      </label>
      <button class="btn" onclick={chooseMap} disabled={locked}>Open…</button>
      <label class="field" title="Read every value with another byte order">
        <span>Byte order</span>
        <select
          class="input"
          bind:value={bench.orderOverride}
          onchange={() => bench.refreshPolling()}
        >
          <option value={null}
            >From map{bench.map
              ? ` (${bench.map.map.meta.byte_order ?? "ABCD"})`
              : ""}</option
          >
          {#each orders as o (o)}<option value={o}>{o}</option>{/each}
        </select>
      </label>
    {/if}

    <div class="options">
      <label class="check"
        ><input type="checkbox" bind:checked={bench.loop} disabled={locked} /> Loop
        command</label
      >
    </div>
  </div>
  {#if openError}<p class="error">Could not open the map: {openError}</p>{/if}
</StepCard>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px 14px;
    align-items: flex-end;
  }

  .source {
    border: 0;
    margin: 0;
    padding: 0;
    min-width: 0;
  }

  .radios {
    display: flex;
    gap: 14px;
    height: 30px;
    align-items: center;
  }

  .command {
    width: 250px;
  }

  .map {
    width: 300px;
  }

  .options {
    margin-left: auto;
    height: 30px;
    display: flex;
    align-items: center;
  }

  .error {
    margin: 8px 0 0;
    color: var(--bad);
  }
</style>
