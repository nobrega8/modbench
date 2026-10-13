<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  let version = $state("");

  $effect(() => {
    invoke<string>("app_version").then((v) => (version = v));
  });
</script>

<main class="shell">
  <aside class="library">
    <h2>Biblioteca</h2>
    <p class="muted">Sem devices ainda.</p>
  </aside>

  <section class="workspace">
    <header class="toolbar">
      <strong>Modbench</strong>
      {#if version}<span class="muted">v{version}</span>{/if}
    </header>
    <div class="empty">
      <p>
        Abre um ficheiro <code>.mbmap</code> ou escolhe um device da biblioteca.
      </p>
    </div>
  </section>
</main>

<style>
  :global(:root) {
    --bg: #f6f7f9;
    --panel: #ffffff;
    --border: #dfe3e8;
    --text: #1d232b;
    --muted: #6b7684;
    font-family: Inter, "Segoe UI", system-ui, sans-serif;
    font-size: 14px;
    color: var(--text);
    background: var(--bg);
  }

  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --bg: #14171c;
      --panel: #1b1f26;
      --border: #2a303a;
      --text: #e4e8ee;
      --muted: #8b95a3;
    }
  }

  :global(body) {
    margin: 0;
  }

  .shell {
    display: grid;
    grid-template-columns: 240px 1fr;
    height: 100vh;
  }

  .library {
    background: var(--panel);
    border-right: 1px solid var(--border);
    padding: 12px 16px;
  }

  .library h2 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
    margin: 4px 0 12px;
  }

  .workspace {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .toolbar {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }

  .empty {
    flex: 1;
    display: grid;
    place-items: center;
    color: var(--muted);
  }

  .muted {
    color: var(--muted);
  }
</style>
