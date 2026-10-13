<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    step,
    title,
    children,
    grow = false,
  }: {
    step: number;
    title: string;
    children: Snippet;
    grow?: boolean;
  } = $props();
</script>

<section class="step" class:grow aria-label="Step {step}: {title}">
  <header class="rail">
    <span class="num">{step}</span>
    <span class="title">{title}</span>
  </header>
  <div class="body">
    {@render children()}
  </div>
</section>

<style>
  .step {
    display: grid;
    grid-template-columns: 76px 1fr;
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.04);
    min-height: 0;
  }

  .step.grow {
    flex: 1;
  }

  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-start;
    gap: 6px;
    padding: 14px 6px;
    border-right: 1px solid var(--border);
    background: var(--subtle);
    border-radius: 8px 0 0 8px;
  }

  .num {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
    font-size: 15px;
  }

  .title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    text-align: center;
  }

  .body {
    padding: 12px 14px;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
