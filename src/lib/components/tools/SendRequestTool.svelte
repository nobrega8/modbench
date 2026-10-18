<script module lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  /** A request kept in the list, to send again. */
  interface Saved {
    name: string;
    raw: boolean;
    fc: number;
    data: number[];
    broadcast: boolean;
  }
  const LIST_KEY = "modbench.requests";

  function loadList(): Saved[] {
    try {
      return JSON.parse(localStorage.getItem(LIST_KEY) ?? "[]") as Saved[];
    } catch {
      return [];
    }
  }

  // Kept while other tools are shown.
  let mode = $state<"function" | "raw">("function");
  let fc = $state(22);
  let otherFc = $state(65);
  let values = $state<Record<string, string>>({});
  let otherHex = $state("");
  let rawHex = $state("");
  let waitMs = $state(500);
  let broadcast = $state(false);
  let list = $state<Saved[]>(loadList());
</script>

<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { errorText } from "#lib/api.ts";
  import { decodeFrame, parseHex, type Framing } from "#lib/frames.ts";
  import { FUNCTIONS, buildData, hexBytes, onlyReads } from "#lib/requests.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { slidePill } from "#lib/motion.ts";
  import IntInput from "../IntInput.svelte";
  import Select from "../Select.svelte";

  const OTHER = 0;
  const spec = $derived(FUNCTIONS.find((f) => f.fc === fc) ?? null);
  const framing = $derived<Framing>(
    bench.transport === "tcp" || bench.transport === "udp"
      ? "tcp"
      : bench.transport === "rtu" && bench.framing === "ascii"
        ? "ascii"
        : "rtu",
  );

  let busy = $state(false);
  let error = $state<string | null>(null);
  let answer = $state.raw<{
    out: number[];
    in: number[];
    note?: string;
  } | null>(null);
  let looping = $state(false);
  let loopMs = $state(1000);

  /** The request as it would be sent, or the reason it cannot be built. */
  const built = $derived.by((): { fc: number; data: number[] } | string => {
    try {
      if (mode === "raw") {
        const bytes = parseHex(rawHex);
        if (!bytes?.length) return t("sendrequesttool.type_the_bytes_in");
        return { fc: -1, data: bytes };
      }
      if (fc === OTHER) {
        const data = otherHex.trim() ? parseHex(otherHex) : [];
        if (data === null) return t("sendrequesttool.data_hex_bytes_e");
        return { fc: otherFc, data };
      }
      return { fc, data: buildData(spec!, valuesFor(spec!.fc)) };
    } catch (e) {
      return errorText(e);
    }
  });

  function valuesFor(code: number): Record<string, string> {
    const s = FUNCTIONS.find((f) => f.fc === code);
    const out: Record<string, string> = {};
    for (const f of s?.fields ?? [])
      out[f.key] = values[`${code}:${f.key}`] ?? f.initial;
    return out;
  }

  function setValue(key: string, v: string) {
    values = { ...values, [`${fc}:${key}`]: v };
  }

  /** Whether sending may write, and so needs the write rules. */
  function writes(
    r: { fc: number; data: number[] },
    raw: boolean,
    bc: boolean,
  ) {
    return raw || bc || !onlyReads(r.fc, r.data);
  }

  async function send(
    r: { fc: number; data: number[] },
    raw: boolean,
    bc: boolean,
  ) {
    error = null;
    if (writes(r, raw, bc)) {
      if (settings.readOnly) {
        error = t("sendrequesttool.read_only_mode_is");
        return false;
      }
      if (
        settings.confirmWrites &&
        !(await ask(
          raw
            ? t("sendrequesttool.send_count_raw_bytes", {
                count: r.data.length,
              })
            : t("sendrequesttool.send_function_fc_which", { fc: r.fc }),
          { title: t("tools.request"), kind: "warning" },
        ))
      )
        return false;
    }
    busy = true;
    try {
      if (raw) {
        const back = await bench.sendRaw(r.data, waitMs);
        answer = {
          out: r.data,
          in: back,
          note: back.length
            ? undefined
            : t("sendrequesttool.no_answer_within_ms", { ms: waitMs }),
        };
      } else {
        const ex = await bench.sendPdu(r.fc, r.data, bc);
        answer = ex
          ? {
              out: ex.out,
              in: ex.in,
              note: bc ? t("sendrequesttool.sent_to_every_device") : undefined,
            }
          : { out: [], in: [], note: t("sendrequesttool.sent2") };
      }
      return true;
    } catch (e) {
      error = errorText(e);
      return false;
    } finally {
      busy = false;
    }
  }

  function sendNow() {
    if (typeof built === "string") return;
    send(built, mode === "raw", broadcast && mode !== "raw");
  }

  function saveToList() {
    if (typeof built === "string") return;
    const name =
      mode === "raw"
        ? `Raw · ${hexBytes(built.data.slice(0, 8))}${built.data.length > 8 ? "…" : ""}`
        : `FC${String(built.fc).padStart(2, "0")} · ${hexBytes(built.data.slice(0, 6))}${built.data.length > 6 ? "…" : ""}`;
    list = [
      ...list,
      { name, raw: mode === "raw", fc: built.fc, data: built.data, broadcast },
    ];
    persist();
  }

  function persist() {
    try {
      localStorage.setItem(LIST_KEY, JSON.stringify(list));
    } catch {
      // Storage unavailable: the list lasts for this session.
    }
  }

  async function sendAll() {
    for (const s of list) {
      if (!(await send({ fc: s.fc, data: s.data }, s.raw, s.broadcast))) {
        looping = false;
        return;
      }
    }
  }

  async function loop() {
    looping = true;
    while (looping) {
      await sendAll();
      if (!looping) break;
      await new Promise((r) => setTimeout(r, Math.max(50, loopMs)));
    }
  }

  const decodedOut = $derived(
    answer?.out.length && mode !== "raw"
      ? decodeFrame(answer.out, framing, "request")
      : null,
  );
  const decodedIn = $derived(
    answer?.in.length ? decodeFrame(answer.in, framing, "response") : null,
  );
</script>

<header class="tool-head">
  <h1>{t("tools.request")}</h1>
  <p>
    {t("sendrequesttool.any_function_code_or", {
      where: bench.connectionText,
      unit: bench.unitId,
    })}
  </p>
</header>

<div class="row">
  <div
    class="segmented"
    role="tablist"
    aria-label={t("step.request")}
    use:slidePill
  >
    <button
      role="tab"
      aria-selected={mode === "function"}
      class:on={mode === "function"}
      onclick={() => (mode = "function")}>{t("logview.function")}</button
    >
    <button
      role="tab"
      aria-selected={mode === "raw"}
      class:on={mode === "raw"}
      onclick={() => (mode = "raw")}>{t("sendrequesttool.raw_bytes")}</button
    >
  </div>
  {#if mode === "function"}
    <label class="field">
      <span>{t("logview.function")}</span>
      <Select
        bind:value={fc}
        label={t("logview.function")}
        width="300px"
        options={[
          ...FUNCTIONS.map((f) => ({
            value: f.fc,
            label: `${String(f.fc).padStart(2, "0")} ${f.name}`,
          })),
          { value: OTHER, label: t("sendrequesttool.other_function_code") },
        ]}
      />
    </label>
    <label class="check" title={t("sendrequesttool.unit_0_every_device")}>
      <input type="checkbox" bind:checked={broadcast} />
      {t("sendrequesttool.broadcast_unit_0")}
    </label>
  {/if}
</div>

{#if mode === "function"}
  <div class="row">
    {#if fc === OTHER}
      <label class="field">
        <span>{t("sendrequesttool.function_code")}</span>
        <IntInput
          value={otherFc}
          min={1}
          max={127}
          width="80px"
          label={t("sendrequesttool.function_code")}
          onchange={(n) => (otherFc = n)}
        />
      </label>
      <label class="field grow">
        <span>{t("sendrequesttool.data_hex")}</span>
        <input
          class="input mono"
          bind:value={otherHex}
          spellcheck="false"
          placeholder="00 01"
        />
      </label>
    {:else if spec}
      {#each spec.fields as f (f.key)}
        <label
          class="field"
          class:grow={f.kind === "words" || f.kind === "hex"}
          title={f.hint}
        >
          <span>{f.label}</span>
          <input
            class="input mono"
            class:short={f.kind === "word" || f.kind === "byte"}
            value={valuesFor(fc)[f.key]}
            spellcheck="false"
            oninput={(e) =>
              setValue(f.key, (e.currentTarget as HTMLInputElement).value)}
          />
        </label>
      {/each}
      {#if !spec.fields.length}<span class="dim"
          >{t("sendrequesttool.no_parameters")}</span
        >{/if}
    {/if}
  </div>
{:else}
  <div class="row">
    <label class="field grow">
      <span>{t("sendrequesttool.bytes_hex_sent_without")}</span>
      <textarea
        class="input mono"
        rows="2"
        bind:value={rawHex}
        spellcheck="false"
        placeholder="00 01 00 00 00 06 01 03 00 00 00 0A"></textarea>
    </label>
    <label class="field" title={t("sendrequesttool.how_long_to_collect")}>
      <span>{t("scanpanel.wait_ms")}</span>
      <IntInput
        value={waitMs}
        min={20}
        max={10000}
        width="80px"
        label={t("sendrequesttool.wait")}
        onchange={(n) => (waitMs = n)}
      />
    </label>
  </div>
{/if}

<div class="row">
  {#if typeof built === "string"}
    <span class:bad={mode !== "raw" || rawHex.trim() !== ""} class="dim"
      >{built}</span
    >
  {:else}
    <span class="mono preview"
      >{mode === "raw"
        ? hexBytes(built.data)
        : `PDU ${hexBytes([built.fc, ...built.data])}`}</span
    >
    {#if writes(built, mode === "raw", broadcast && mode !== "raw")}
      <span class="tag">{t("sendrequesttool.may_write")}</span>
    {/if}
  {/if}
  <span class="spacer"></span>
  <button class="btn" disabled={typeof built === "string"} onclick={saveToList}
    >{t("sendrequesttool.add_to_list")}</button
  >
  <button
    class="btn accent"
    disabled={busy || bench.running || typeof built === "string"}
    title={bench.running ? t("addressscantool.stop_the_test_on") : undefined}
    onclick={sendNow}
    >{busy ? t("sendrequesttool.sending") : t("sendrequesttool.send")}</button
  >
</div>

{#if error}<p class="bad" role="alert">{error}</p>{/if}

{#if answer}
  <section class="answer">
    {#if answer.note}<p class="dim">{answer.note}</p>{/if}
    {#if answer.out.length}
      <h2 class="key">{t("sendrequesttool.sent")}</h2>
      <p class="mono">{hexBytes(answer.out)}</p>
      {#if decodedOut}<p class="dim">{decodedOut.summary}</p>{/if}
    {/if}
    {#if answer.in.length}
      <h2 class="key">{t("deviceidtool.answer")}</h2>
      <p class="mono">{hexBytes(answer.in)}</p>
      {#if decodedIn}
        <p>{decodedIn.summary}</p>
        {#if decodedIn.error}<p class="bad">{decodedIn.error}</p>{/if}
        <table class="data">
          <tbody>
            {#each decodedIn.fields as f, i (i)}
              <tr class:bad={f.bad}>
                <td>{f.label}</td>
                <td class="mono">{f.bytes}</td>
                <td>{f.meaning}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else if !answer.note}
      <p class="dim">{t("sendrequesttool.no_answer")}</p>
    {/if}
  </section>
{/if}

{#if list.length}
  <section class="list">
    <div class="row">
      <h2 class="key">{t("registergrid.list")}</h2>
      <span class="spacer"></span>
      <button
        class="btn small"
        disabled={busy || looping || bench.running}
        onclick={sendAll}>{t("sendrequesttool.send_all")}</button
      >
      {#if looping}
        <button class="btn small" onclick={() => (looping = false)}
          >{t("test.stop")}</button
        >
      {:else}
        <button
          class="btn small"
          disabled={busy || bench.running}
          onclick={loop}>{t("sendrequesttool.loop")}</button
        >
      {/if}
      <label class="inline"
        >{t("req.every")}
        <IntInput
          value={loopMs}
          min={50}
          max={600000}
          width="80px"
          label={t("sendrequesttool.loop_interval")}
          onchange={(n) => (loopMs = n)}
        /> ms</label
      >
    </div>
    <ul>
      {#each list as s, i (i)}
        <li>
          <span class="mono">{s.name}</span>
          {#if s.broadcast}<span class="tag"
              >{t("sendrequesttool.broadcast")}</span
            >{/if}
          <span class="spacer"></span>
          <button
            class="btn small"
            disabled={busy || bench.running}
            onclick={() => send({ fc: s.fc, data: s.data }, s.raw, s.broadcast)}
            >{t("sendrequesttool.send")}</button
          >
          <button
            class="btn small ghost"
            onclick={() => {
              list = list.filter((_, k) => k !== i);
              persist();
            }}>{t("maptable.remove")}</button
          >
        </li>
      {/each}
    </ul>
  </section>
{/if}

<style>
  p {
    margin: 0;
  }

  .grow {
    flex: 1;
    min-width: 240px;
  }

  .grow input,
  .grow textarea {
    width: 100%;
  }

  .short {
    width: 90px;
  }

  textarea {
    resize: vertical;
    font-family: var(--mono);
  }

  .check,
  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    color: var(--text-2);
  }

  .preview {
    font-size: 12px;
    color: var(--text-2);
  }

  .tag {
    padding: 1px 6px;
    border-radius: 4px;
    background: var(--subtle);
    font-size: 11px;
    color: var(--text-2);
  }

  .spacer {
    flex: 1;
  }

  .answer,
  .list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  h2 {
    margin: 6px 0 0;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 5px 0;
    border-top: 1px solid var(--border);
  }

  .btn.small {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
  }
</style>
