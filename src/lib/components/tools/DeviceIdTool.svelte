<script module lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import type {
    Area,
    DeviceIdentity,
    FunctionCheck,
    ReadLimit,
    Support,
  } from "#lib/types.ts";

  // Kept while other tools are shown.
  let result = $state.raw<DeviceIdentity | null>(null);
  let checks = $state.raw<FunctionCheck[] | null>(null);
  let address = $state(0);
  let writes = $state(false);
  let limitArea = $state<Area>("holding");
  let limit = $state.raw<{
    area: Area;
    address: number;
    read: ReadLimit;
  } | null>(null);
  let level = $state(1);
</script>

<script lang="ts">
  import { errorText } from "#lib/api.ts";
  import { AREA_NAME, formatAddress, hex } from "#lib/format.ts";
  import { slidePill } from "#lib/motion.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import AddressInput from "../AddressInput.svelte";
  import Select from "../Select.svelte";

  const levels = $derived([
    { id: 1, label: t("deviceidtool.basic") },
    { id: 2, label: t("deviceidtool.regular") },
    { id: 3, label: t("deviceidtool.extended") },
  ]);

  let busy = $state(false);
  let error = $state<string | null>(null);
  let checking = $state(false);
  let checkError = $state<string | null>(null);
  let measuring = $state(false);
  let limitError = $state<string | null>(null);
  const areas: Area[] = ["holding", "input", "coil", "discrete"];

  const answers: Record<Support, string> = $derived({
    supported: t("deviceidtool.supported"),
    not_supported: t("deviceidtool.not_supported"),
    refused: t("deviceidtool.supported_refused_this_request"),
    no_answer: t("deviceidtool.no_answer"),
    skipped: t("deviceidtool.not_tried"),
  });
  // Functions the standard defines for serial lines only
  const serialOnly = new Set([7, 8, 11, 17]);

  const cannotAsk = $derived(
    busy ||
      checking ||
      measuring ||
      bench.scanning ||
      (bench.transport === "rtu" && !bench.serialPort),
  );

  const where = $derived(
    bench.transport === "rtu"
      ? t("deviceidtool.port_unit_unit", {
          port: bench.serialPort || t("scanpanel.the_serial_port"),
          unit: bench.unitId,
        })
      : t("deviceidtool.host_port_unit_unit", {
          host: bench.host,
          port: bench.tcpPort,
          unit: bench.unitId,
        }),
  );

  /** What the conformity byte says the device supports. */
  function conformity(c: number): string {
    const kind =
      [
        "",
        t("deviceidtool.basic2"),
        t("deviceidtool.regular2"),
        t("deviceidtool.extended2"),
      ][c & 0x7f] ?? t("deviceidtool.unknown");
    return t("deviceidtool.kind_objects_access", {
      kind,
      access:
        c & 0x80
          ? t("deviceidtool.stream_and_single_object")
          : t("deviceidtool.stream_access_only"),
    });
  }

  async function check() {
    checking = true;
    checkError = null;
    try {
      const found = await bench.checkFunctions(
        address,
        writes && !settings.readOnly,
      );
      if (found) checks = found;
    } catch (e) {
      checks = null;
      checkError = errorText(e);
    } finally {
      checking = false;
    }
  }

  async function measure() {
    measuring = true;
    limitError = null;
    try {
      limit = {
        area: limitArea,
        address,
        read: await bench.largestRead(limitArea, address),
      };
    } catch (e) {
      limit = null;
      limitError = errorText(e);
    } finally {
      measuring = false;
    }
  }

  async function read() {
    busy = true;
    error = null;
    try {
      result = await bench.readDeviceId(level);
    } catch (e) {
      result = null;
      const text = errorText(e);
      error = text.includes("0x01")
        ? t("deviceidtool.the_device_does_not")
        : text;
    } finally {
      busy = false;
    }
  }
</script>

<header class="tool-head">
  <h1>{t("deviceidtool.identify_the_device")}</h1>
  <p>{t("deviceidtool.vendor_product_and_revision", { where })}</p>
</header>

<div class="row">
  <div class="field">
    <span>{t("deviceidtool.objects")}</span>
    <div
      class="segmented"
      role="radiogroup"
      aria-label={t("deviceidtool.objects")}
      use:slidePill
    >
      {#each levels as l (l.id)}
        <button
          role="radio"
          aria-checked={level === l.id}
          class:on={level === l.id}
          onclick={() => (level = l.id)}>{l.label}</button
        >
      {/each}
    </div>
  </div>
  <button class="btn accent" disabled={cannotAsk} onclick={read}
    >{busy
      ? t("deviceidtool.asking")
      : t("deviceidtool.read_identification")}</button
  >
</div>

{#if error}
  <p class="bad" role="alert">{error}</p>
{:else if result}
  {#if result.objects.length}
    <table class="data">
      <thead>
        <tr
          ><th>ID</th><th>{t("deviceidtool.object")}</th><th
            >{t("maptable.value")}</th
          ></tr
        >
      </thead>
      <tbody>
        {#each result.objects as o (o.id)}
          <tr>
            <td class="mono dim">0x{hex(o.id, 2)}</td>
            <td>{o.name}</td>
            <td class="mono value">{o.value || "—"}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="empty">{t("deviceidtool.the_device_answered_without")}</p>
  {/if}
  <p class="dim">
    {t("deviceidtool.conformity_0x_hex_text", {
      hex: hex(result.conformity, 2),
      text: conformity(result.conformity),
    })}
  </p>
{:else}
  <p class="empty">{t("deviceidtool.devices_without_fc43_answer")}</p>
{/if}

<section class="functions">
  <header class="tool-head">
    <h2>{t("deviceidtool.function_codes")}</h2>
    <p>
      {t("deviceidtool.illegal_function_not_supported")}
    </p>
  </header>
  <div class="row">
    <label class="field">
      <span>{t("req.address")}</span>
      <AddressInput
        value={address}
        width="90px"
        label={t("deviceidtool.address_to_try")}
        disabled={cannotAsk}
        onchange={(n) => (address = n)}
      />
    </label>
    <label
      class="check"
      title={settings.readOnly
        ? t("deviceidtool.read_only_mode_is")
        : t("deviceidtool.writes_back_the_value")}
    >
      <input
        type="checkbox"
        bind:checked={writes}
        disabled={cannotAsk || settings.readOnly}
      />
      {t("deviceidtool.also_try_write_functions")}{#if settings.readOnly}<span
          class="dim">{t("deviceidtool.off_in_read_only")}</span
        >{/if}
    </label>
    <button class="btn" disabled={cannotAsk} onclick={check}
      >{checking
        ? t("settingsview.checking")
        : t("deviceidtool.check_function_codes")}</button
    >
  </div>

  {#if checkError}
    <p class="bad" role="alert">{checkError}</p>
  {:else if checks}
    <table class="data">
      <thead>
        <tr
          ><th class="num">FC</th><th>{t("logview.function")}</th><th
            >{t("deviceidtool.answer")}</th
          ></tr
        >
      </thead>
      <tbody>
        {#each checks as c (c.function)}
          <tr>
            <td class="num mono">{c.function}</td>
            <td
              >{c.name}{#if serialOnly.has(c.function)}<span class="dim"
                  >{t("deviceidtool.serial_line_only")}</span
                >{/if}</td
            >
            <td class="answer {c.support}"
              >{answers[c.support]}{#if c.detail}<span class="dim"
                  >{` · ${c.detail}`}</span
                >{/if}</td
            >
          </tr>
        {/each}
      </tbody>
    </table>
    {#if bench.transport !== "rtu"}
      <p class="dim">{t("deviceidtool.serial_only_functions_often")}</p>
    {/if}
  {/if}

  <header class="tool-head">
    <h2>{t("deviceidtool.largest_read")}</h2>
    <p>
      {t("deviceidtool.most_registers_or_bits")}
    </p>
  </header>
  <div class="row">
    <label class="field">
      <span>{t("customregisterform.area")}</span>
      <Select
        bind:value={limitArea}
        label={t("customregisterform.area")}
        disabled={cannotAsk}
        options={areas.map((a) => ({ value: a, label: AREA_NAME[a] + "s" }))}
      />
    </label>
    <button class="btn" disabled={cannotAsk} onclick={measure}
      >{measuring
        ? t("deviceidtool.reading")
        : t("deviceidtool.find_largest_read")}</button
    >
  </div>
  {#if limitError}
    <p class="bad" role="alert">{limitError}</p>
  {:else if limit}
    <p>
      From {AREA_NAME[limit.area].toLowerCase()}
      {formatAddress(limit.address, settings.addressFormat, limit.area)}:
      <strong>{limit.read.largest}</strong>
      {limit.area === "coil" || limit.area === "discrete"
        ? "bits"
        : "registers"}
      in one read{#if limit.read.limit}<span class="dim"
          >{`. One more: ${limit.read.limit}`}</span
        >{:else}, the most the protocol allows{/if}.
    </p>
  {/if}
</section>

<style>
  .functions {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 12px;
  }

  .functions h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }

  .answer.supported,
  .answer.refused {
    color: var(--ok);
  }

  .answer.not_supported,
  .answer.skipped {
    color: var(--text-2);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
  }

  .answer.no_answer {
    color: var(--bad);
  }

  .value {
    word-break: break-all;
  }

  p {
    margin: 0;
  }
</style>
