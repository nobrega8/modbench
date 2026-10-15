<script lang="ts">
  import type { Snippet } from "svelte";
  import { smoothHeight } from "#lib/motion.ts";

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
  {#if grow}
    <div class="body">
      {@render children()}
    </div>
  {:else}
    <div class="clip" use:smoothHeight>
      <div class="body">
        {@render children()}
      </div>
    </div>
  {/if}
</section>

<style>
  .step {
    display: grid;
    grid-template-columns: 80px 1fr;
    background: var(--card);
    border-radius: 14px;
    box-shadow: var(--e2);
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
    padding: 12px 6px;
  }

  .num {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
    font-size: 13px;
    font-variant-numeric: tabular-nums;
  }

  .title {
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: center;
  }

  .clip {
    min-width: 0;
  }

  .body {
    padding: 10px 16px 12px 4px;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
</style>
