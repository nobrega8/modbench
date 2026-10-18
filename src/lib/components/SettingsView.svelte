<script lang="ts" module>
  type SectionId =
    "about" | "updates" | "appearance" | "values" | "connection" | "library";

  /** The section shown, kept while the app runs. */
  let section = $state<SectionId>("about");
</script>

<script lang="ts">
  import {
    LOCALES,
    detectLocale,
    t,
    type Key,
    type Locale,
  } from "#lib/i18n.svelte.ts";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";
  import { viewIn } from "#lib/motion.ts";
  import { api, errorText } from "#lib/api.ts";
  import { bench } from "#lib/state.svelte.ts";
  import { demo } from "#lib/demo.svelte.ts";
  import {
    settings,
    TEXT_SCALE_MAX,
    TEXT_SCALE_MIN,
    type Theme,
  } from "#lib/settings.svelte.ts";
  import { forgetAll } from "#lib/profiles.ts";
  import { APT_UPGRADE, updates } from "#lib/updates.svelte.ts";

  const themes: { id: Theme; label: Key }[] = [
    { id: "system", label: "settings.theme.system" },
    { id: "light", label: "settings.theme.light" },
    { id: "dark", label: "settings.theme.dark" },
  ];
  const logLimits = [200, 1000, 5000, 20000];
  const holds = [
    { s: 0, label: "Off" },
    { s: 1, label: "1 second" },
    { s: 2, label: "2 seconds" },
    { s: 5, label: "5 seconds" },
    { s: 10, label: "10 seconds" },
    { s: 30, label: "30 seconds" },
  ];

  const categories: {
    title: Key;
    sections: { id: SectionId }[];
  }[] = [
    {
      title: "settings.modbench",
      sections: [{ id: "about" }, { id: "updates" }],
    },
    {
      title: "settings.display",
      sections: [{ id: "appearance" }, { id: "values" }],
    },
    {
      title: "settings.devices",
      sections: [{ id: "connection" }, { id: "library" }],
    },
  ];

  /** The language the system asks for, as Modbench supports it. */
  const systemLocale: Locale = detectLocale(navigator.languages);

  let version = $state("");
  let copied = $state(false);
  let forgotten = $state<number | null>(null);
  let problem = $state<string | null>(null);

  $effect(() => {
    api.appVersion().then((v) => (version = v));
  });

  // The text size is applied when the slider is let go: scaling the window
  // while dragging moves the slider itself away from the pointer.
  let scaleDraft = $state<number | null>(null);

  function changed() {
    settings.save();
    settings.applyTheme();
  }

  async function run(action: () => Promise<void>) {
    problem = null;
    try {
      await action();
    } catch (e) {
      problem = errorText(e);
    }
  }

  function forgetDevices() {
    forgotten = forgetAll();
    setTimeout(() => (forgotten = null), 2500);
  }

  async function copyFolder() {
    try {
      await navigator.clipboard.writeText(bench.libraryFolder);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      copied = false;
    }
  }
</script>

{#snippet external()}
  <svg class="ext" viewBox="0 0 12 12" aria-hidden="true"
    ><path d="M4.5 2.5h5v5M9.5 2.5l-7 7" /></svg
  >
{/snippet}

<div class="settings">
  <nav aria-label={t("nav.settings")}>
    {#each categories as c (c.title)}
      <h2 class="key">{t(c.title)}</h2>
      {#each c.sections as s (s.id)}
        <button
          class:on={section === s.id}
          aria-current={section === s.id ? "page" : undefined}
          onclick={() => (section = s.id)}
        >
          <span class="glyph"><Icon name={s.id} size={16} /></span>
          <span class="name">{t(`settings.${s.id}` as Key)}</span>
          <span class="hint">{t(`settings.${s.id}.hint` as Key)}</span>
        </button>
      {/each}
    {/each}
  </nav>

  {#key section}
    <div class="panel" in:viewIn>
      {#if section === "about"}
        <section class="about">
          <img src="/app-icon.png" alt="" width="88" height="88" />
          <h1>Modbench</h1>
          <p class="version">
            {t("settingsview.version")} <span class="mono">{version}</span>
            {#if updates.available}
              · <strong>{updates.available.version} is available</strong>
            {:else if updates.message}
              · {updates.message}
            {/if}
          </p>
          <p class="tagline">
            {t("settingsview.a_modbus_tcp_and")}
          </p>
          <div class="hero-buttons">
            <button
              class="btn primary"
              onclick={() => run(() => api.openLink("repo"))}
              >View on GitHub {@render external()}</button
            >
            <button
              class="btn"
              disabled={updates.checking}
              onclick={() => updates.check(false)}
              >{updates.checking ? "Checking…" : "Check for updates"}</button
            >
          </div>
        </section>

        <h2 class="key">{t("settingsview.help")}</h2>
        <div class="group">
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.user_manual")}</span>
              <span class="hint">{t("settingsview.every_page_and_tool")}</span>
            </div>
            <button
              class="btn"
              onclick={() => run(() => api.openLink("manual"))}
              >Open {@render external()}</button
            >
          </div>
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.release_notes")}</span>
              <span class="hint">{t("settingsview.what_changed_in_each")}</span>
            </div>
            <button
              class="btn"
              onclick={() => run(() => api.openLink("releases"))}
              >Open {@render external()}</button
            >
          </div>
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.report_a_problem")}</span>
              <span class="hint"
                >{t("settingsview.bugs_and_device_requests")}</span
              >
            </div>
            <button
              class="btn"
              onclick={() => run(() => api.openLink("issues"))}
              >Report {@render external()}</button
            >
          </div>
        </div>

        <h2 class="key">{t("settingsview.project")}</h2>
        <div class="group">
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.licence")}</span>
              <span class="hint">{t("settingsview.gpl_3_0_on")}</span>
            </div>
            <button class="btn" onclick={() => run(() => api.openLink("repo"))}
              >Source code {@render external()}</button
            >
          </div>
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.support_this_project")}</span>
              <span class="hint">{t("settingsview.a_coffee_on_ko")}</span>
            </div>
            <button
              class="btn"
              onclick={() => run(() => api.openLink("support"))}
              ><svg class="heart" viewBox="0 0 16 16" aria-hidden="true"
                ><path
                  d="M8 13.5S2 10 2 6a3 3 0 0 1 6-1 3 3 0 0 1 6 1c0 4-6 7.5-6 7.5z"
                /></svg
              >Ko-fi {@render external()}</button
            >
          </div>
        </div>
      {:else if section === "appearance"}
        <header class="head">
          <h1>{t("settings.appearance")}</h1>
        </header>
        <div class="group">
          <div class="setting theme-row">
            <span class="name">{t("settings.theme")}</span>
            <div
              class="themes"
              role="radiogroup"
              aria-label={t("settings.theme")}
            >
              {#each themes as th (th.id)}
                <button
                  class="theme"
                  role="radio"
                  aria-checked={settings.theme === th.id}
                  class:on={settings.theme === th.id}
                  onclick={() => {
                    settings.theme = th.id;
                    changed();
                  }}
                >
                  <span class="preview {th.id}" aria-hidden="true">
                    <span class="layer light"><i></i><b></b><b></b></span>
                    <span class="layer dark"><i></i><b></b><b></b></span>
                  </span>
                  <span class="label">{t(th.label)}</span>
                </button>
              {/each}
            </div>
          </div>
          <div class="setting">
            <div class="text">
              <span class="name">{t("settings.textSize")}</span>
              <span class="hint">{t("settings.textSize.hint")}</span>
            </div>
            <div class="scale">
              <input
                type="range"
                min={TEXT_SCALE_MIN * 100}
                max={TEXT_SCALE_MAX * 100}
                step="10"
                value={Math.round((scaleDraft ?? settings.textScale) * 100)}
                aria-label={t("settings.textSize")}
                oninput={(e) => {
                  scaleDraft = Number(e.currentTarget.value) / 100;
                }}
                onchange={(e) => {
                  settings.textScale = Number(e.currentTarget.value) / 100;
                  scaleDraft = null;
                  settings.applyTextScale();
                  changed();
                }}
              />
              <span class="mono pct"
                >{Math.round((scaleDraft ?? settings.textScale) * 100)}%</span
              >
              <button
                class="btn"
                disabled={settings.textScale === 1}
                onclick={() => {
                  settings.textScale = 1;
                  settings.applyTextScale();
                  changed();
                }}>{t("settings.reset")}</button
              >
            </div>
          </div>
          <div class="setting">
            <div class="text">
              <span class="name">{t("settings.language")}</span>
              <span class="hint">{t("settings.language.hint")}</span>
            </div>
            <Select
              bind:value={settings.language}
              label={t("settings.language")}
              width="230px"
              options={[
                {
                  value: "system",
                  label: t("settings.language.system", {
                    language:
                      LOCALES.find((l) => l.id === systemLocale)?.name ?? "",
                  }),
                },
                ...LOCALES.map((l) => ({ value: l.id, label: l.name })),
              ]}
              onchange={() => {
                settings.applyLanguage();
                changed();
              }}
            />
          </div>
        </div>
      {:else if section === "values"}
        <header class="head">
          <h1>{t("settings.values")}</h1>
        </header>
        <div class="group">
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.highlight_changed_values")}</span
              >
              <span class="hint">{t("settingsview.how_long_a_changed")}</span>
            </div>
            <Select
              bind:value={settings.highlightSeconds}
              label={t("settingsview.highlight_changed_values")}
              options={holds.map((h) => ({ value: h.s, label: h.label }))}
              onchange={changed}
            />
          </label>
        </div>
      {:else if section === "connection"}
        <header class="head">
          <h1>{t("settings.connection")}</h1>
        </header>
        <div class="group">
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.remember_the_last_connection")}</span
              >
              <span class="hint">{t("settingsview.step_1_is_filled")}</span>
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.rememberConnection}
              onchange={changed}
            />
          </label>
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.remember_register_choices_for")}</span
              >
              <span class="hint"
                >{t("settingsview.registers_read_byte_order")}</span
              >
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.rememberDevices}
              onchange={changed}
            />
          </label>
          <div class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.forget_saved_device_choices")}</span
              >
              <span class="hint"
                >{forgotten === null
                  ? "Every device goes back to reading all registers."
                  : `Forgot ${forgotten} device${forgotten === 1 ? "" : "s"}.`}</span
              >
            </div>
            <button class="btn" onclick={forgetDevices}
              >{t("settingsview.forget")}</button
            >
          </div>
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.confirm_before_writing_to")}</span
              >
              <span class="hint"
                >{t("settingsview.ask_before_every_write")}</span
              >
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.confirmWrites}
              onchange={changed}
            />
          </label>
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.reconnect_when_the_connection")}</span
              >
              <span class="hint"
                >{t("settingsview.reopen_a_failed_connection")}</span
              >
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.autoReconnect}
              onchange={changed}
            />
          </label>
          <div class="setting">
            <div class="text">
              <span class="name">{t("demobanner.demo")}</span>
              <span class="hint"
                >{t("settingsview.a_simulated_meter_your")}</span
              >
            </div>
            <button
              class="btn"
              disabled={demo.active || demo.starting}
              onclick={() => {
                bench.page = "main";
                demo.start();
              }}>{t("settingsview.start_demo")}</button
            >
          </div>
          <label class="setting">
            <div class="text">
              <span class="name">{t("settingsview.offer_the_demo_at")}</span>
              <span class="hint">{t("settingsview.offer_the_demo_until")}</span>
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.offerDemo}
              onchange={changed}
            />
          </label>
          <label class="setting">
            <div class="text">
              <span class="name">{t("settingsview.read_only_mode")}</span>
              <span class="hint">{t("settingsview.block_every_write_for")}</span
              >
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.readOnly}
              onchange={changed}
            />
          </label>
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.write_single_registers_with")}</span
              >
              <span class="hint"
                >{t("settingsview.write_single_registers_with2")}</span
              >
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.singleWriteFc16}
              onchange={changed}
            />
          </label>
          <label class="setting">
            <div class="text">
              <span class="name">{t("settingsview.csv_separator")}</span>
              <span class="hint"
                >{t("settingsview.semicolon_for_spreadsheets_with")}</span
              >
            </div>
            <Select
              bind:value={settings.csvSeparator}
              label={t("settingsview.csv_separator")}
              options={[
                { value: ",", label: "Comma ( , )" },
                { value: ";", label: "Semicolon ( ; )" },
              ]}
              onchange={changed}
            />
          </label>
          <label class="setting">
            <div class="text">
              <span class="name">{t("settingsview.requests_kept_in_the")}</span>
              <span class="hint"
                >{t("settingsview.older_requests_are_dropped")}</span
              >
            </div>
            <Select
              bind:value={settings.logLimit}
              label={t("settingsview.requests_kept_in_the")}
              options={logLimits.map((n) => ({
                value: n,
                label: n.toLocaleString("en"),
              }))}
              onchange={changed}
            />
          </label>
        </div>
      {:else if section === "library"}
        <header class="head">
          <h1>{t("settings.library")}</h1>
        </header>
        <div class="group">
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.library_folder")}</span>
              <span class="hint path" title={bench.libraryFolder}
                >{bench.libraryFolder || "—"}</span
              >
            </div>
            <div class="buttons">
              <button
                class="btn"
                onclick={copyFolder}
                disabled={!bench.libraryFolder}
                >{copied ? "Copied" : "Copy path"}</button
              >
              <button
                class="btn"
                onclick={() => run(() => api.libraryOpenFolder())}
                >{t("settingsview.open_folder")}</button
              >
            </div>
          </div>
          <div class="setting">
            <div class="text">
              <span class="name">{t("settingsview.shared_maps")}</span>
              <span class="hint"
                >The <span class="mono">.mbmap</span> format and shared maps.</span
              >
            </div>
            <div class="buttons">
              <button
                class="btn"
                onclick={() => run(() => api.openLink("spec"))}
                >Format {@render external()}</button
              >
              <button
                class="btn"
                onclick={() => run(() => api.openLink("library"))}
                >Repository {@render external()}</button
              >
            </div>
          </div>
        </div>
      {:else if section === "updates"}
        <header class="head">
          <h1>{t("settings.updates")}</h1>
        </header>
        <div class="group">
          <label class="setting">
            <div class="text">
              <span class="name"
                >{t("settingsview.check_for_updates_when")}</span
              >
              <span class="hint"
                >{#if updates.viaApt}Installed with apt: updates install with
                  <span class="mono">{APT_UPGRADE}</span>{:else}You choose when
                  to install.{/if}</span
              >
            </div>
            <input
              type="checkbox"
              class="switch"
              role="switch"
              bind:checked={settings.checkUpdates}
              onchange={changed}
            />
          </label>
        </div>
      {/if}

      {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
    </div>
  {/key}
</div>

<style>
  .settings {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 244px 1fr;
    gap: 22px;
  }

  /* ── The list of sections, as on the Tools page ── */
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: auto;
  }

  nav h2 {
    margin: 14px 0 6px 10px;
  }

  nav h2:first-child {
    margin-top: 0;
  }

  nav button {
    display: grid;
    grid-template-columns: 16px 1fr;
    column-gap: 10px;
    row-gap: 1px;
    align-items: center;
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

  .glyph {
    grid-row: span 2;
    align-self: start;
    margin-top: 2px;
    color: var(--text-2);
  }

  nav button.on .glyph {
    color: var(--text);
  }

  nav .hint {
    grid-column: 2;
    font-size: 12px;
    color: var(--text-3);
  }

  .panel {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
  }

  /* Sections keep their height; the panel scrolls instead */
  .panel > :global(*) {
    flex-shrink: 0;
    max-width: 760px;
  }

  .head h1 {
    font-size: 17px;
    letter-spacing: -0.011em;
  }

  .scale {
    display: flex;
    flex: none;
    align-items: center;
    gap: 10px;
  }

  .scale input {
    width: 140px;
    accent-color: rgb(var(--fg-rgb));
  }

  .pct {
    width: 40px;
    text-align: right;
    color: var(--text-2);
  }

  /* ── About: the app on top, then its links as settings rows ── */
  .about {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 34px 22px 28px;
    border-radius: 14px;
    background: var(--subtle);
  }

  .about img {
    border-radius: 20px;
    margin-bottom: 14px;
  }

  .about h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 600;
    letter-spacing: -0.021em;
    line-height: 1.15;
  }

  .version {
    margin: 6px 0 0;
    color: var(--text-2);
  }

  .version strong {
    color: var(--text);
    font-weight: 600;
  }

  .tagline {
    margin: 4px 0 0;
    color: var(--text-3);
  }

  .hero-buttons {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 8px;
    margin-top: 20px;
  }

  .panel > h2 {
    margin: 10px 0 -6px 4px;
  }

  .heart {
    width: 13px;
    height: 13px;
    fill: #ff5e5b;
  }

  .ext {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
    opacity: 0.7;
  }

  .problem {
    margin: 0 2px;
    color: var(--bad);
    font-size: 13px;
  }

  /* ── A section: one grouped list of settings ── */
  .group {
    border-radius: 12px;
    background: var(--subtle);
    overflow: hidden;
  }

  .setting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 14px 16px;
  }

  /* Inset separators between rows */
  .setting + .setting {
    box-shadow: inset 0 1px 0 var(--border);
  }

  label.setting {
    cursor: pointer;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .name {
    font-weight: 500;
  }

  .hint {
    font-size: 13px;
    color: var(--text-2);
  }

  .path {
    font-family: var(--mono);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .buttons {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .setting :global(.select) {
    flex: none;
  }

  /* ── Theme picker: a tiny window per choice ── */
  .theme-row {
    align-items: flex-start;
  }

  .themes {
    display: flex;
    gap: 14px;
  }

  .theme {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }

  .theme.on {
    color: var(--text);
  }

  .preview {
    position: relative;
    display: block;
    width: 96px;
    height: 62px;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 0 0 1px rgb(var(--fg-rgb) / 0.1);
    transition: box-shadow 0.14s var(--ease);
  }

  .theme:hover .preview {
    box-shadow: 0 0 0 1px rgb(var(--fg-rgb) / 0.28);
  }

  .theme.on .preview {
    box-shadow:
      0 0 0 2px rgb(var(--surface-2-rgb)),
      0 0 0 4px rgb(var(--fg-rgb));
  }

  .theme:focus-visible {
    box-shadow: none;
  }

  .theme:focus-visible:not(.on) .preview {
    box-shadow:
      0 0 0 2px rgb(var(--surface-2-rgb)),
      0 0 0 4px rgb(var(--fg-rgb) / 0.35);
  }

  /* Both palettes are drawn on top of each other; each choice shows one,
     "Same as system" shows light on the left and dark on the right. */
  .layer {
    position: absolute;
    inset: 0;
    padding: 10px 0 0 10px;
  }

  .preview.light .dark,
  .preview.dark .light {
    display: none;
  }

  .preview.system .dark {
    clip-path: inset(0 0 0 50%);
  }

  .layer.light {
    background: rgb(242 242 245);
  }

  .layer.dark {
    background: #000;
  }

  .layer i {
    display: block;
    width: 100px;
    height: 60px;
    border-radius: 6px;
  }

  .layer.light i {
    background: #fff;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.08);
  }

  .layer.dark i {
    background: rgb(32 32 35);
  }

  .layer b {
    position: absolute;
    left: 18px;
    height: 4px;
    border-radius: 2px;
  }

  .layer b:nth-of-type(1) {
    top: 20px;
    width: 36px;
  }

  .layer b:nth-of-type(2) {
    top: 30px;
    width: 58px;
    opacity: 0.45;
  }

  .layer.light b {
    background: rgb(23 23 23);
  }

  .layer.dark b {
    background: rgb(245 245 245);
  }
</style>
