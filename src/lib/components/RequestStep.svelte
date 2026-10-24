<script lang="ts">
  import { t, td } from "#lib/i18n.svelte.ts";
  import { FUNCTION_NAME } from "#lib/format.ts";
  import { fieldIn, slidePill } from "#lib/motion.ts";
  import { bench } from "#lib/state.svelte.ts";
  import type { Area, ByteOrder } from "#lib/types.ts";
  import AddressInput from "./AddressInput.svelte";
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

  const READ_FC: Record<Area, number> = {
    coil: 1,
    discrete: 2,
    holding: 3,
    input: 4,
  };

  /** Follows an address that names its area (400101, %MW100): the same
   * kind of command in that area, or a read where it cannot be written. */
  function setArea(area: Area) {
    const write = [5, 6, 15, 16].includes(bench.fc);
    const fc =
      write && area === "coil"
        ? single
          ? 5
          : 15
        : write && area === "holding"
          ? single
            ? 6
            : 16
          : READ_FC[area];
    if (fc === bench.fc) return;
    bench.fc = fc;
    onFunction();
  }
</script>

<StepCard step={2} title={t("step.request")}>
  <div class="row">
    <label class="field">
      <span>{t("req.device")}</span>
      <IntInput
        value={bench.unitId}
        min={0}
        max={255}
        width="72px"
        label={t("req.unitId")}
        disabled={locked}
        onchange={(n) => (bench.unitId = n)}
      />
    </label>
    <button
      class="btn"
      title={bench.transport === "rtu"
        ? t("req.find.serial")
        : t("req.find.net")}
      disabled={locked}
      onclick={() => (scanOpen = !scanOpen)}>{t("req.find")}</button
    >

    <div class="field">
      <span>{t("req.source")}</span>
      <div
        class="segmented"
        role="radiogroup"
        aria-label={t("req.source")}
        use:slidePill
      >
        <button
          role="radio"
          aria-checked={bench.source === "manual"}
          class:on={bench.source === "manual"}
          disabled={locked}
          onclick={() => setSource("manual")}>{t("req.manual")}</button
        >
        <button
          role="radio"
          aria-checked={bench.source === "map"}
          class:on={bench.source === "map"}
          disabled={locked}
          onclick={() => setSource("map")}>{t("req.map")}</button
        >
      </div>
    </div>

    {#if bench.source === "manual"}
      <label in:fieldIn class="field">
        <span>{t("req.command")}</span>
        <Select
          bind:value={bench.fc}
          width="250px"
          label={t("req.command")}
          disabled={locked}
          options={Object.keys(FUNCTION_NAME).map((code) => ({
            value: Number(code),
            label: `${td(`fc.${code}`)} (${code.padStart(2, "0")})`,
          }))}
          onchange={onFunction}
        />
      </label>
      <label in:fieldIn class="field">
        <span
          >{bench.fc <= 2 || bench.fc === 5 || bench.fc === 15
            ? t("req.address")
            : t("req.register")}</span
        >
        <AddressInput
          value={bench.address}
          label={t("req.startAddress")}
          disabled={locked}
          onchange={(n, area) => {
            bench.address = n;
            if (area) setArea(area);
          }}
        />
      </label>
      <label in:fieldIn class="field">
        <span>{t("req.quantity")}</span>
        <IntInput
          value={bench.quantity}
          min={1}
          max={maxQuantity}
          width="80px"
          label={t("req.quantity")}
          disabled={locked || single}
          onchange={(n) => bench.setQuantity(n)}
        />
      </label>
    {:else}
      <div in:fieldIn class="field">
        <span>{t("req.mapField")}</span>
        <MapPicker disabled={locked} />
      </div>
      <label in:fieldIn class="field" title={t("req.byteOrder.hint")}>
        <span>{t("req.byteOrder")}</span>
        <Select
          value={bench.orderOverride ?? ""}
          label={t("req.byteOrder")}
          options={[
            {
              value: "",
              label: `${t("req.fromMap")}${bench.map ? ` (${bench.map.map.meta.byte_order ?? "ABCD"})` : ""}`,
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
        />
        {t("req.loop")}</label
      >
      <!-- Always laid out, hidden while the loop is off: the switch keeps its
           place instead of jumping when the field comes and goes -->
      <label
        class="every"
        class:off={!bench.loop}
        inert={!bench.loop}
        aria-hidden={!bench.loop}
        title={t("req.every.hint")}
      >
        {t("req.every")}
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
          aria-label={t("req.every.label")}
        />
        {t("req.ms")}
      </label>
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
    transition:
      opacity 0.2s var(--ease),
      transform 0.2s var(--ease);
  }

  .every.off {
    opacity: 0;
    transform: translateX(-4px);
    pointer-events: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .every {
      transition: none;
    }
  }

  .every input {
    width: 84px;
  }
</style>
