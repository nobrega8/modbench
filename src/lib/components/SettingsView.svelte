<script lang="ts">
  import { api } from "$lib/api";
  import { bench } from "$lib/state.svelte";
  import { settings, type Theme } from "$lib/settings.svelte";
  import { updates } from "$lib/updates.svelte";

  const themes: { id: Theme; label: string }[] = [
    { id: "system", label: "Same as Windows" },
    { id: "light", label: "Light" },
    { id: "dark", label: "Dark" },
  ];
  const logLimits = [200, 1000, 5000, 20000];

  let version = $state("");
  let copied = $state(false);

  $effect(() => {
    api.appVersion().then((v) => (version = v));
  });

  function changed() {
    settings.save();
    settings.applyTheme();
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

<div class="settings">
  <section>
    <h2>Appearance</h2>
    <div class="setting">
      <div class="text">
        <span class="name">Theme</span>
      </div>
      <div class="segmented" role="radiogroup" aria-label="Theme">
        {#each themes as t (t.id)}
          <button
            role="radio"
            aria-checked={settings.theme === t.id}
            class:on={settings.theme === t.id}
            onclick={() => {
              settings.theme = t.id;
              changed();
            }}>{t.label}</button
          >
        {/each}
      </div>
    </div>
  </section>

  <section>
    <h2>Connection</h2>
    <label class="setting">
      <div class="text">
        <span class="name">Remember the last connection</span>
        <span class="hint"
          >Port, baud rate, address, device and timing are filled in again the
          next time Modbench opens.</span
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
  </section>

  <section>
    <h2>Writing</h2>
    <label class="setting">
      <div class="text">
        <span class="name">Confirm before writing to a device</span>
        <span class="hint"
          >Asks before every write: write commands, map values and control
          commands.</span
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
  </section>

  <section>
    <h2>Log</h2>
    <label class="setting">
      <div class="text">
        <span class="name">Requests kept in the log</span>
        <span class="hint">Older requests are dropped first.</span>
      </div>
      <select class="input" bind:value={settings.logLimit} onchange={changed}>
        {#each logLimits as n (n)}<option value={n}
            >{n.toLocaleString("en")}</option
          >{/each}
      </select>
    </label>
  </section>

  <section>
    <h2>Device library</h2>
    <div class="setting">
      <div class="text">
        <span class="name">Library folder</span>
        <span class="hint path">{bench.libraryFolder || "—"}</span>
      </div>
      <button class="btn" onclick={copyFolder} disabled={!bench.libraryFolder}
        >{copied ? "Copied" : "Copy path"}</button
      >
    </div>
  </section>

  <section>
    <h2>Updates</h2>
    <label class="setting">
      <div class="text">
        <span class="name">Check for updates when Modbench starts</span>
        <span class="hint">You choose when to install.</span>
      </div>
      <input
        type="checkbox"
        class="switch"
        role="switch"
        bind:checked={settings.checkUpdates}
        onchange={changed}
      />
    </label>
    <div class="setting">
      <div class="text">
        <span class="name">Modbench {version}</span>
        {#if updates.message}<span class="hint">{updates.message}</span>{/if}
      </div>
      <button
        class="btn"
        disabled={updates.checking}
        onclick={() => updates.check(false)}
        >{updates.checking ? "Checking…" : "Check now"}</button
      >
    </div>
  </section>
</div>

<style>
  .settings {
    overflow: auto;
    flex: 1;
    min-height: 0;
    max-width: 760px;
  }

  section + section {
    margin-top: 20px;
  }

  h2 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
  }

  .setting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 12px 14px;
    background: var(--subtle);
    border: 1px solid var(--border);
    border-radius: 6px;
  }

  .setting + .setting {
    margin-top: 4px;
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

  .segmented {
    display: flex;
    flex: none;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    overflow: hidden;
  }

  .segmented button {
    border: 0;
    background: var(--card);
    padding: 5px 12px;
    cursor: pointer;
    color: var(--text-2);
  }

  .segmented button + button {
    border-left: 1px solid var(--border-strong);
  }

  .segmented button.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  select {
    flex: none;
  }
</style>
