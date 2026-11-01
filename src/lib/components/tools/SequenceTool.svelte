<script module lang="ts">
  import type { Sequence, StepResult } from "#lib/types.ts";

  // Kept while other tools are shown.
  let sequence = $state<Sequence>({ name: "", steps: [] });
  let results = $state.raw<StepResult[]>([]);
  let path = $state<string | null>(null);
</script>

<!--
  Test sequences for commissioning: steps on the open map's registers (write
  a value, wait, expect a value within a time, read one to record it), run
  against the device of step 1, with a result per step and an HTML report.
-->
<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { ask, open as openDialog, save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { t } from "#lib/i18n.svelte.ts";
  import {
    SEQUENCE_EXTENSIONS,
    newStep,
    parseSequence,
    stepProblem,
    stepRegister,
    writes,
    type StepKind,
  } from "#lib/sequence.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { toasts } from "#lib/toast.svelte.ts";
  import type { ReportContext, SequenceStep } from "#lib/types.ts";
  import Icon from "../Icon.svelte";
  import Select from "../Select.svelte";

  let running = $state(false);
  let context = $state.raw<ReportContext | null>(null);

  const map = $derived(bench.map?.map ?? null);
  const names = $derived(map?.registers.map((r) => r.name) ?? []);
  const KINDS = $derived<{ value: StepKind; label: string }[]>([
    { value: "write", label: t("sequence.write") },
    { value: "wait", label: t("sequence.wait") },
    { value: "expect", label: t("sequence.expect") },
    { value: "read", label: t("sequence.read") },
  ]);

  function add() {
    const last = sequence.steps.at(-1);
    const register = (last && stepRegister(last)) || names[0] || "";
    sequence.steps.push(newStep("expect", register));
    results = [];
  }

  function setKind(i: number, kind: StepKind) {
    const register = stepRegister(sequence.steps[i]) || names[0] || "";
    sequence.steps[i] = newStep(kind, register);
    results = [];
  }

  function move(i: number, by: -1 | 1) {
    const to = i + by;
    if (to < 0 || to >= sequence.steps.length) return;
    const [s] = sequence.steps.splice(i, 1);
    sequence.steps.splice(to, 0, s);
    results = [];
  }

  function remove(i: number) {
    sequence.steps.splice(i, 1);
    results = [];
  }

  /** Equal to a value, or in a range: the expect step's two forms. */
  function setMode(s: SequenceStep, mode: "equals" | "range") {
    if (s.do !== "expect") return;
    if (mode === "equals") {
      s.equals = s.min ?? 0;
      delete s.min;
      delete s.max;
    } else {
      s.min = s.equals ?? 0;
      s.max = s.equals ?? 0;
      delete s.equals;
      delete s.tolerance;
    }
  }

  const num = (v: string, fallback = 0) =>
    v.trim() === "" || !Number.isFinite(+v) ? fallback : +v;

  async function openFile() {
    const picked = await openDialog({
      multiple: false,
      filters: [{ name: t("sequence.file"), extensions: SEQUENCE_EXTENSIONS }],
    });
    if (typeof picked !== "string") return;
    try {
      sequence = parseSequence(await api.readText(picked));
      path = picked;
      results = [];
    } catch (e) {
      toasts.fail(`${picked}: ${errorText(e)}`);
    }
  }

  async function saveFile() {
    const where = await save({
      defaultPath: path ?? "sequence.mbtest.json",
      filters: [{ name: t("sequence.file"), extensions: SEQUENCE_EXTENSIONS }],
    });
    if (!where) return;
    try {
      await api.saveText(
        where,
        JSON.stringify($state.snapshot(sequence), null, 2) + "\n",
      );
      path = where;
      toasts.show(t("export.saved", { path: where }));
    } catch (e) {
      toasts.fail(errorText(e));
    }
  }

  async function run() {
    if (writes(sequence) && !(await ask(t("sequence.confirm_writes")))) return;
    running = true;
    results = [];
    const stop = await listen<StepResult>("sequence-step", (e) => {
      results = [...results, e.payload];
    });
    try {
      const done = await bench.runSequence($state.snapshot(sequence));
      results = done.results;
      context = done.context;
    } catch (e) {
      toasts.fail(errorText(e));
    } finally {
      stop();
      running = false;
    }
  }

  async function saveReport() {
    if (!context) return;
    const where = await save({
      defaultPath: "sequence-report.html",
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!where) return;
    try {
      const html = await api.sequenceHtml(
        $state.snapshot(sequence),
        results,
        context,
      );
      await api.saveText(where, html);
      toasts.show(t("export.saved", { path: where }));
    } catch (e) {
      toasts.fail(errorText(e));
    }
  }

  const resultOf = (i: number) => results.find((r) => r.index === i);
  const passed = $derived(
    results.length > 0 &&
      results.every((r) => r.status === "pass" || r.status === "done"),
  );
</script>

<header class="tool-head">
  <h1>{t("tools.sequence")}</h1>
  <p>{t("sequence.intro")}</p>
</header>

<div class="row">
  <label class="field name">
    <span>{t("computedform.name")}</span>
    <input
      class="input"
      bind:value={sequence.name}
      placeholder={t("sequence.untitled")}
    />
  </label>
  <button class="btn" onclick={openFile}
    ><Icon name="folder" />{t("sequence.open")}</button
  >
  <button class="btn" disabled={!sequence.steps.length} onclick={saveFile}
    >{t("builder.save")}</button
  >
  <span class="spacer"></span>
  <button
    class="btn accent"
    disabled={running || !map || !sequence.steps.length || bench.running}
    title={!map
      ? t("state.choosemap")
      : bench.running
        ? t("check.stop_first")
        : ""}
    onclick={run}>{running ? t("sequence.running") : t("sequence.run")}</button
  >
  <button class="btn" disabled={!context || running} onclick={saveReport}
    >{t("sequence.report")}</button
  >
</div>
{#if !map}<p class="dim">{t("sequence.no_map")}</p>{/if}
{#if results.length && !running}
  <p class:ok={passed} class:bad={!passed} role="status">
    {passed ? t("sequence.passed") : t("sequence.failed")}
  </p>
{/if}

<datalist id="sequence-registers">
  {#each names as n (n)}<option value={n}></option>{/each}
</datalist>

<table class="data steps">
  <thead>
    <tr>
      <th class="n">#</th>
      <th>{t("sequence.step")}</th>
      <th>{t("sequence.register")}</th>
      <th>{t("sequence.parameters")}</th>
      <th>{t("logview.result")}</th>
      <th></th>
    </tr>
  </thead>
  <tbody>
    {#each sequence.steps as s, i (s)}
      {@const r = resultOf(i)}
      {@const problem = stepProblem(s, map)}
      <tr class:bad={r?.status === "fail" || r?.status === "error"}>
        <td class="n dim">{i + 1}</td>
        <td>
          <Select
            value={s.do}
            options={KINDS}
            label={t("sequence.step")}
            disabled={running}
            onchange={(k: StepKind) => setKind(i, k)}
          />
        </td>
        <td>
          {#if s.do !== "wait"}
            <input
              class="input reg"
              list="sequence-registers"
              aria-label={t("sequence.register")}
              bind:value={s.register}
              aria-invalid={!!problem}
              title={problem ?? ""}
              disabled={running}
              spellcheck="false"
            />
          {/if}
        </td>
        <td class="params">
          {#if s.do === "write"}
            <label
              >= <input
                class="input num"
                value={s.value}
                aria-label={t("sequence.write")}
                disabled={running}
                onchange={(e) => (s.value = num(e.currentTarget.value))}
              /></label
            >
          {:else if s.do === "wait"}
            <label
              ><input
                class="input num"
                value={s.ms}
                aria-label={t("sequence.wait")}
                disabled={running}
                onchange={(e) =>
                  (s.ms = Math.max(0, num(e.currentTarget.value)))}
              /> ms</label
            >
          {:else if s.do === "expect"}
            <select
              class="input mode"
              aria-label={t("sequence.expect")}
              value={s.equals !== undefined ? "equals" : "range"}
              disabled={running}
              onchange={(e) =>
                setMode(s, e.currentTarget.value as "equals" | "range")}
            >
              <option value="equals">=</option>
              <option value="range">{t("sequence.range")}</option>
            </select>
            {#if s.equals !== undefined}
              <input
                class="input num"
                value={s.equals}
                aria-label={t("sequence.expect")}
                disabled={running}
                onchange={(e) => (s.equals = num(e.currentTarget.value))}
              />
              <label
                >± <input
                  class="input num"
                  value={s.tolerance ?? 0}
                  aria-label="±"
                  disabled={running}
                  onchange={(e) =>
                    (s.tolerance = Math.abs(num(e.currentTarget.value)))}
                /></label
              >
            {:else}
              <input
                class="input num"
                value={s.min ?? ""}
                aria-label="min"
                placeholder="min"
                disabled={running}
                onchange={(e) =>
                  (s.min = e.currentTarget.value.trim()
                    ? num(e.currentTarget.value)
                    : undefined)}
              />
              …
              <input
                class="input num"
                value={s.max ?? ""}
                aria-label="max"
                placeholder="max"
                disabled={running}
                onchange={(e) =>
                  (s.max = e.currentTarget.value.trim()
                    ? num(e.currentTarget.value)
                    : undefined)}
              />
            {/if}
            <label
              >{t("sequence.within")}
              <input
                class="input num"
                value={s.within_ms}
                aria-label={t("sequence.within")}
                disabled={running}
                onchange={(e) =>
                  (s.within_ms = Math.max(0, num(e.currentTarget.value)))}
              /> ms</label
            >
          {/if}
        </td>
        <td class="result">
          {#if r}
            <span
              class:ok={r.status === "pass" || r.status === "done"}
              class:bad={r.status === "fail" || r.status === "error"}
              >{t(`sequence.status.${r.status}`)}</span
            >
            {#if r.value}<span class="mono">{r.value}</span>{/if}
            {#if r.message}<span class="dim">{r.message}</span>{/if}
          {:else if problem}
            <span class="bad">{problem}</span>
          {/if}
        </td>
        <td class="act">
          <button
            class="btn icon ghost"
            title={t("builder.row_up")}
            aria-label={t("builder.row_up")}
            disabled={running || i === 0}
            onclick={() => move(i, -1)}><Icon name="up" size={14} /></button
          >
          <button
            class="btn icon ghost"
            title={t("builder.row_down")}
            aria-label={t("builder.row_down")}
            disabled={running || i === sequence.steps.length - 1}
            onclick={() => move(i, 1)}><Icon name="down" size={14} /></button
          >
          <button
            class="btn icon ghost"
            title={t("builder.row_delete")}
            aria-label={t("builder.row_delete")}
            disabled={running}
            onclick={() => remove(i)}><Icon name="trash" size={14} /></button
          >
        </td>
      </tr>
    {:else}
      <tr><td colspan="6" class="dim empty">{t("sequence.empty")}</td></tr>
    {/each}
  </tbody>
</table>
<div class="row">
  <button class="btn" disabled={running} onclick={add}
    ><Icon name="plus" />{t("sequence.add")}</button
  >
</div>

<style>
  p {
    margin: 0;
  }

  .spacer {
    flex: 1;
  }

  .name .input {
    width: 260px;
  }

  .steps {
    width: 100%;
  }

  .n {
    width: 28px;
    text-align: right;
  }

  .reg {
    width: 180px;
  }

  .params {
    white-space: nowrap;
  }

  .params .num {
    width: 72px;
  }

  .params label {
    margin-left: 6px;
  }

  .mode {
    width: auto;
  }

  .result {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    min-width: 160px;
    font-size: 12px;
  }

  .act {
    white-space: nowrap;
    text-align: right;
  }

  .ok {
    color: var(--ok, var(--text-2));
  }

  .bad {
    color: var(--bad);
  }

  .empty {
    padding: 14px 8px;
  }
</style>
