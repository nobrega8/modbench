<script module lang="ts">
  import type { Direction, Framing } from "#lib/frames.ts";

  // Kept while other tools are shown.
  let input = $state("01 03 00 00 00 0A C5 CD");
  let framing = $state<Framing>("rtu");
  let direction = $state<Direction>("request");
</script>

<script lang="ts">
  import { decodeFrame, parseHex } from "#lib/frames.ts";
  import { slidePill } from "#lib/motion.ts";
  import FrameFields from "../FrameFields.svelte";

  const framings: { id: Framing; label: string }[] = [
    { id: "rtu", label: "RTU" },
    { id: "tcp", label: "TCP" },
    { id: "ascii", label: "ASCII" },
  ];

  const bytes = $derived(parseHex(input));
  const decoded = $derived(
    bytes?.length ? decodeFrame(bytes, framing, direction) : null,
  );

  // An ASCII frame pasted as text is ASCII, whatever was chosen.
  $effect(() => {
    if (input.trim().startsWith(":")) framing = "ascii";
  });
</script>

<header class="tool-head">
  <h1>Frame decoder</h1>
  <p>
    Paste a frame from a log, a bus analyser or another tool to read it field by
    field, with its CRC or LRC checked. Frames on the Raw data tab are decoded
    the same way.
  </p>
</header>

<div class="row">
  <label class="field grow">
    <span>Frame</span>
    <input
      class="input num"
      bind:value={input}
      spellcheck="false"
      placeholder="01 03 00 00 00 0A C5 CD"
      aria-invalid={!bytes}
    />
  </label>
  <div class="field">
    <span>Framing</span>
    <div class="segmented" role="radiogroup" aria-label="Framing" use:slidePill>
      {#each framings as f (f.id)}
        <button
          role="radio"
          aria-checked={framing === f.id}
          class:on={framing === f.id}
          onclick={() => (framing = f.id)}>{f.label}</button
        >
      {/each}
    </div>
  </div>
  <div class="field">
    <span>Sent by</span>
    <div class="segmented" role="radiogroup" aria-label="Sent by" use:slidePill>
      <button
        role="radio"
        aria-checked={direction === "request"}
        class:on={direction === "request"}
        onclick={() => (direction = "request")}
        ><span class="from tx" aria-hidden="true"></span>Client</button
      >
      <button
        role="radio"
        aria-checked={direction === "response"}
        class:on={direction === "response"}
        onclick={() => (direction = "response")}
        ><span class="from rx" aria-hidden="true"></span>Device</button
      >
    </div>
  </div>
</div>

{#if !bytes}
  <p class="bad">
    Type the frame as hex bytes, e.g. 01 03 00 00 00 0A C5 CD, or an ASCII frame
    starting with ":".
  </p>
{:else if decoded}
  <FrameFields {decoded} />
{/if}

<style>
  .grow {
    flex: 1;
    min-width: 260px;
  }

  .grow input {
    width: 100%;
  }

  p {
    margin: 0;
  }

  /* The logo's arrows: blue from the client, green from the device */
  .from {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-right: 7px;
    border-radius: 50%;
    vertical-align: 1px;
  }

  .from.tx {
    background: var(--tx);
  }

  .from.rx {
    background: var(--rx);
  }
</style>
