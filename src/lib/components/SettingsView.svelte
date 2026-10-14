<script lang="ts">
  import { api, errorText } from "$lib/api";
  import { bench } from "$lib/state.svelte";
  import {
    settings,
    TEXT_SCALE_MAX,
    TEXT_SCALE_MIN,
    type Theme,
  } from "$lib/settings.svelte";
  import { forgetAll } from "$lib/profiles";
  import { APT_UPGRADE, updates } from "$lib/updates.svelte";

  const themes: { id: Theme; label: string }[] = [
    { id: "system", label: "Same as system" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
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

  let version = $state("");
  let copied = $state(false);
  let forgotten = $state<number | null>(null);
  let problem = $state<string | null>(null);

  $effect(() => {
    api.appVersion().then((v) => (version = v));
  });

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
  <div class="column">
    <!-- About: the app, its version, and where the project lives -->
    <section class="about">
      <img src="/favicon.png" alt="" width="56" height="56" />
      <div class="who">
        <h1>Modbench</h1>
        <p class="version">
          Version <span class="mono">{version}</span>
          {#if updates.available}
            · <strong>{updates.available.version} is available</strong>
          {:else if updates.message}
            · {updates.message}
          {/if}
        </p>
        <p class="tagline">Modbus TCP/RTU client · MIT licence</p>
      </div>
      <button
        class="btn"
        disabled={updates.checking}
        onclick={() => updates.check(false)}
        >{updates.checking ? "Checking…" : "Check for updates"}</button
      >
      <div class="links">
        <button
          class="btn primary"
          onclick={() => run(() => api.openLink("repo"))}
          >View on GitHub {@render external()}</button
        >
        <button class="btn" onclick={() => run(() => api.openLink("issues"))}
          >Report a problem {@render external()}</button
        >
        <button class="btn" onclick={() => run(() => api.openLink("releases"))}
          >Release notes {@render external()}</button
        >
      </div>
    </section>

    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

    <div class="cols">
      <div class="col">
        <section>
          <h2 class="key">Appearance</h2>
          <div class="group">
            <div class="setting theme-row">
              <span class="name">Theme</span>
              <div class="themes" role="radiogroup" aria-label="Theme">
                {#each themes as t (t.id)}
                  <button
                    class="theme"
                    role="radio"
                    aria-checked={settings.theme === t.id}
                    class:on={settings.theme === t.id}
                    onclick={() => {
                      settings.theme = t.id;
                      changed();
                    }}
                  >
                    <span class="preview {t.id}" aria-hidden="true">
                      <span class="layer light"><i></i><b></b><b></b></span>
                      <span class="layer dark"><i></i><b></b><b></b></span>
                    </span>
                    <span class="label">{t.label}</span>
                  </button>
                {/each}
              </div>
            </div>
            <div class="setting">
              <div class="text">
                <span class="name">Text size</span>
                <span class="hint"
                  >Smaller to see more at once, larger to read from further
                  away. Text and controls scale together.</span
                >
              </div>
              <div class="scale">
                <input
                  type="range"
                  min={TEXT_SCALE_MIN * 100}
                  max={TEXT_SCALE_MAX * 100}
                  step="10"
                  value={Math.round(settings.textScale * 100)}
                  aria-label="Text size"
                  oninput={(e) => {
                    settings.textScale = Number(e.currentTarget.value) / 100;
                    settings.applyTextScale();
                  }}
                  onchange={changed}
                />
                <span class="mono pct"
                  >{Math.round(settings.textScale * 100)}%</span
                >
                <button
                  class="btn"
                  disabled={settings.textScale === 1}
                  onclick={() => {
                    settings.textScale = 1;
                    settings.applyTextScale();
                    changed();
                  }}>Reset</button
                >
              </div>
            </div>
          </div>
        </section>
        <section>
          <h2 class="key">Values</h2>
          <div class="group">
            <label class="setting">
              <div class="text">
                <span class="name">Highlight changed values</span>
                <span class="hint"
                  >A value that changes stays highlighted this long, in the
                  register grid and the device map. Helps to find what moves on
                  an unknown device.</span
                >
              </div>
              <select
                class="input"
                bind:value={settings.highlightSeconds}
                onchange={changed}
              >
                {#each holds as h (h.s)}<option value={h.s}>{h.label}</option
                  >{/each}
              </select>
            </label>
          </div>
        </section>
        <section>
          <h2 class="key">Device library</h2>
          <div class="group">
            <div class="setting">
              <div class="text">
                <span class="name">Library folder</span>
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
                  >Open folder</button
                >
              </div>
            </div>
            <div class="setting">
              <div class="text">
                <span class="name">Shared maps</span>
                <span class="hint"
                  >The <span class="mono">.mbmap</span> format and the maps other
                  users share live in the mbmap-library repository.</span
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
        </section>
        <section>
          <h2 class="key">Updates</h2>
          <div class="group">
            <label class="setting">
              <div class="text">
                <span class="name">Check for updates when Modbench starts</span>
                <span class="hint"
                  >{#if updates.viaApt}Installed with apt: updates install with
                    <span class="mono">{APT_UPGRADE}</span>{:else}You choose
                    when to install.{/if}</span
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
        </section>
      </div>
      <div class="col">
        <section>
          <h2 class="key">Connection</h2>
          <div class="group">
            <label class="setting">
              <div class="text">
                <span class="name">Remember the last connection</span>
                <span class="hint"
                  >Port, baud rate, address, device and timing are filled in
                  again the next time Modbench opens.</span
                >
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
                  >Remember register choices for each device</span
                >
                <span class="hint"
                  >Which registers are read, the byte order and Loop come back
                  when you pick the same map on the same connection and unit ID.
                  Nothing is sent to the device when they are restored.</span
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
                <span class="name">Forget saved device choices</span>
                <span class="hint"
                  >{forgotten === null
                    ? "Every device goes back to reading all registers."
                    : `Forgot ${forgotten} device${forgotten === 1 ? "" : "s"}.`}</span
                >
              </div>
              <button class="btn" onclick={forgetDevices}>Forget</button>
            </div>
            <label class="setting">
              <div class="text">
                <span class="name">Confirm before writing to a device</span>
                <span class="hint"
                  >Asks before every write: write commands, map values and
                  control commands.</span
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
                <span class="name">Reconnect when the connection is lost</span>
                <span class="hint"
                  >While a test runs, a connection that fails (an I/O error, or
                  over TCP three timeouts in a row) is opened again, after 0.5 s
                  and then less often, up to every 10 s.</span
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
            <label class="setting">
              <div class="text">
                <span class="name">Read-only mode</span>
                <span class="hint"
                  >Blocks every write, also on a connection that is already
                  open. Reading is not affected. Use it on a plant in operation.</span
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
                <span class="name">CSV separator</span>
                <span class="hint"
                  >For saved logs, exported values and recordings. Use a
                  semicolon if your spreadsheet expects a decimal comma.</span
                >
              </div>
              <select
                class="input"
                bind:value={settings.csvSeparator}
                onchange={changed}
              >
                <option value=",">Comma ( , )</option>
                <option value=";">Semicolon ( ; )</option>
              </select>
            </label>
            <label class="setting">
              <div class="text">
                <span class="name">Requests kept in the log</span>
                <span class="hint">Older requests are dropped first.</span>
              </div>
              <select
                class="input"
                bind:value={settings.logLimit}
                onchange={changed}
              >
                {#each logLimits as n (n)}<option value={n}
                    >{n.toLocaleString("en")}</option
                  >{/each}
              </select>
            </label>
          </div>
        </section>
      </div>
    </div>
  </div>
</div>

<style>
  .settings {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: -20px -22px;
    padding: 28px 22px 40px;
  }

  .column {
    max-width: 1180px;
    margin: 0 auto;
  }

  /* Two columns of sections; one on a narrow window */
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0 22px;
    align-items: start;
    margin-top: 26px;
  }

  @media (max-width: 900px) {
    .cols {
      grid-template-columns: 1fr;
    }

    .col + .col {
      margin-top: 26px;
    }
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

  /* ── About ── */
  .about {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 6px 18px;
    padding: 22px;
    border-radius: 14px;
    background: var(--subtle);
  }

  .about img {
    border-radius: 13px;
    align-self: start;
  }

  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.021em;
    line-height: 1.15;
  }

  .version {
    margin: 3px 0 0;
    color: var(--text-2);
    font-size: 13px;
  }

  .version strong {
    color: var(--text);
    font-weight: 600;
  }

  .tagline {
    margin: 1px 0 0;
    color: var(--text-3);
    font-size: 13px;
  }

  .about > .btn {
    align-self: start;
  }

  .links {
    grid-column: 2 / -1;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
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
    margin: 10px 2px 0;
    color: var(--bad);
    font-size: 13px;
  }

  /* ── Sections: small-caps eyebrow over one grouped list ── */
  section + section {
    margin-top: 26px;
  }

  h2 {
    margin: 0 0 8px 4px;
  }

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

  select {
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
