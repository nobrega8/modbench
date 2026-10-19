<!--
  The list-shaped parts of a map's header, in the builder's optional
  parameters: the documents it was made from, its named state labels, the
  values a data type uses to say "no data", and the keys of the file this
  version does not know (`x_…`).
-->
<script lang="ts">
  import { t } from "#lib/i18n.svelte.ts";
  import { enumText, parseEnum } from "#lib/builder.ts";
  import type { DataType, Meta, Source } from "#lib/types.ts";

  let {
    meta,
    ontouch,
  }: {
    meta: Meta;
    ontouch: () => void;
  } = $props();

  const TYPES: DataType[] = [
    "uint16",
    "int16",
    "uint32",
    "int32",
    "uint64",
    "int64",
  ];

  /** Rows whose labels text is not valid, by `enum:N` or `sentinel:N`. */
  let bad = $state<Record<string, boolean>>({});

  const textOrNull = (s: string) => (s.trim() === "" ? null : s);

  const sources = $derived(meta.sources ?? []);

  function addSource() {
    meta.sources = [
      ...sources,
      { title: "", version: null, date: null, url: null, note: null },
    ];
    ontouch();
  }

  function removeSource(i: number) {
    meta.sources = sources.filter((_, k) => k !== i);
    ontouch();
  }

  function setSource(i: number, key: keyof Source, text: string) {
    const s = meta.sources![i];
    if (key === "title") s.title = text;
    else s[key] = textOrNull(text);
    ontouch();
  }

  function setLabels(kind: "enum" | "sentinel", i: number, text: string) {
    const pairs = parseEnum(text);
    bad[`${kind}:${i}`] = pairs === null;
    if (pairs === null) return;
    if (kind === "enum") meta.enums[i][1] = pairs;
    else meta.sentinels[i][1] = pairs;
    ontouch();
  }

  function addEnum() {
    meta.enums = [...meta.enums, ["", []]];
    ontouch();
  }

  function removeEnum(i: number) {
    meta.enums = meta.enums.filter((_, k) => k !== i);
    bad = {};
    ontouch();
  }

  function addSentinel() {
    const used = new Set(meta.sentinels.map(([ty]) => ty));
    const type = TYPES.find((ty) => !used.has(ty));
    if (!type) return;
    meta.sentinels = [...meta.sentinels, [type, []]];
    ontouch();
  }

  function removeSentinel(i: number) {
    meta.sentinels = meta.sentinels.filter((_, k) => k !== i);
    bad = {};
    ontouch();
  }

  function addExtra() {
    meta.extra = [...meta.extra, ["x_", ""]];
    ontouch();
  }

  function removeExtra(i: number) {
    meta.extra = meta.extra.filter((_, k) => k !== i);
    ontouch();
  }
</script>

<div class="lists">
  <h4>{t("builder.sources")}</h4>
  {#each sources as s, i (i)}
    <div class="item">
      <input
        class="input grow"
        placeholder={t("builder.src_title")}
        aria-label={t("builder.src_title")}
        value={s.title}
        oninput={(e) => setSource(i, "title", e.currentTarget.value)}
      />
      <input
        class="input narrow"
        placeholder={t("builder.src_version")}
        aria-label={t("builder.src_version")}
        value={s.version ?? ""}
        oninput={(e) => setSource(i, "version", e.currentTarget.value)}
      />
      <input
        class="input narrow"
        placeholder="YYYY-MM-DD"
        aria-label={t("builder.src_date")}
        value={s.date ?? ""}
        oninput={(e) => setSource(i, "date", e.currentTarget.value)}
      />
      <input
        class="input grow"
        placeholder="URL"
        aria-label="URL"
        value={s.url ?? ""}
        oninput={(e) => setSource(i, "url", e.currentTarget.value)}
      />
      <input
        class="input grow"
        placeholder={t("builder.src_note")}
        aria-label={t("builder.src_note")}
        value={s.note ?? ""}
        oninput={(e) => setSource(i, "note", e.currentTarget.value)}
      />
      <button
        class="btn ghost small"
        aria-label={t("maptable.remove")}
        onclick={() => removeSource(i)}>×</button
      >
    </div>
  {/each}
  <button class="btn ghost small" onclick={addSource}>{t("builder.add")}</button
  >

  <h4>{t("builder.enums")}</h4>
  {#each meta.enums as e, i (i)}
    <div class="item">
      <input
        class="input narrow"
        placeholder={t("builder.enum_name")}
        aria-label={t("builder.enum_name")}
        bind:value={e[0]}
        oninput={ontouch}
        spellcheck="false"
      />
      <input
        class="input grow"
        placeholder="0=Off, 1=On"
        aria-label={t("builder.labels")}
        aria-invalid={bad[`enum:${i}`]}
        value={enumText(e[1])}
        oninput={(ev) => setLabels("enum", i, ev.currentTarget.value)}
      />
      <button
        class="btn ghost small"
        aria-label={t("maptable.remove")}
        onclick={() => removeEnum(i)}>×</button
      >
    </div>
  {/each}
  <button class="btn ghost small" onclick={addEnum}>{t("builder.add")}</button>

  <h4>{t("builder.sentinels")}</h4>
  {#each meta.sentinels as s, i (i)}
    <div class="item">
      <select
        class="input narrow"
        aria-label={t("customregisterform.type")}
        value={s[0]}
        onchange={(ev) => {
          s[0] = ev.currentTarget.value as DataType;
          ontouch();
        }}
      >
        {#each TYPES as ty (ty)}<option value={ty}>{ty}</option>{/each}
      </select>
      <input
        class="input grow"
        placeholder="0xFFFF=n/a"
        aria-label={t("builder.labels")}
        aria-invalid={bad[`sentinel:${i}`]}
        value={enumText(s[1])}
        oninput={(ev) => setLabels("sentinel", i, ev.currentTarget.value)}
      />
      <button
        class="btn ghost small"
        aria-label={t("maptable.remove")}
        onclick={() => removeSentinel(i)}>×</button
      >
    </div>
  {/each}
  <button
    class="btn ghost small"
    disabled={meta.sentinels.length >= TYPES.length}
    onclick={addSentinel}>{t("builder.add")}</button
  >

  <h4>{t("builder.extra_keys")}</h4>
  {#each meta.extra as x, i (i)}
    <div class="item">
      <input
        class="input narrow mono"
        aria-label={t("builder.extra_key")}
        bind:value={x[0]}
        oninput={ontouch}
        spellcheck="false"
      />
      <input
        class="input grow"
        aria-label={t("builder.extra_value")}
        bind:value={x[1]}
        oninput={ontouch}
      />
      <button
        class="btn ghost small"
        aria-label={t("maptable.remove")}
        onclick={() => removeExtra(i)}>×</button
      >
    </div>
  {/each}
  <button class="btn ghost small" onclick={addExtra}>{t("builder.add")}</button>
</div>

<style>
  .lists {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    margin-top: 12px;
  }

  h4 {
    margin: 8px 0 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }

  .item {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-self: stretch;
  }

  .grow {
    flex: 1 1 200px;
  }

  .narrow {
    flex: 0 1 140px;
  }

  .mono {
    font-family: var(--mono);
  }

  input[aria-invalid="true"] {
    outline: 1px solid var(--danger, #c0392b);
  }
</style>
