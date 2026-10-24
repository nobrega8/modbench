<script lang="ts">
  // A readout whose digits roll to their new value, like an odometer, instead
  // of being redrawn. Other characters (a point, a dash) are drawn as they
  // are. Columns are matched from the right, so the units keep rolling in
  // place when the number grows a digit. After EasyUI's Animated Number.
  let {
    text,
    dim = 0,
  }: {
    text: string;
    /** How many leading characters are drawn faded (padding zeros). */
    dim?: number;
  } = $props();

  const chars = $derived(
    [...text].map((c, i) => ({
      c,
      key: text.length - i,
      digit: c >= "0" && c <= "9" ? Number(c) : null,
      off: i < dim,
    })),
  );
</script>

<span class="roll" aria-hidden="true">
  {#each chars as ch (ch.key)}
    {#if ch.digit === null}
      <span class:off={ch.off}>{ch.c}</span>
    {:else}
      <span class="col" class:off={ch.off}
        ><span class="strip" style:transform="translateY({-ch.digit * 10}%)"
          >{#each { length: 10 } as _, n (n)}<span>{n}</span>{/each}</span
        ></span
      >
    {/if}
  {/each}
</span>

<style>
  .roll {
    display: inline-flex;
    font-variant-numeric: tabular-nums;
  }

  .col {
    display: inline-block;
    height: 1.25em;
    line-height: 1.25em;
    overflow: hidden;
  }

  .strip {
    display: flex;
    flex-direction: column;
    transition: transform 0.38s var(--ease);
  }

  .off {
    opacity: 0.25;
  }

  @media (prefers-reduced-motion: reduce) {
    .strip {
      transition: none;
    }
  }
</style>
