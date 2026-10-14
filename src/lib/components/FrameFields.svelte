<script lang="ts">
  import type { Decoded } from "$lib/frames";

  let { decoded, compact = false }: { decoded: Decoded; compact?: boolean } =
    $props();
</script>

<div class="frame" class:compact>
  <p class="summary">
    <strong>{decoded.summary}</strong>
    {#if decoded.error}<span class="bad">{decoded.error}</span>{/if}
  </p>
  <table>
    <tbody>
      {#each decoded.fields as f, i (i)}
        <tr class:bad={f.bad}>
          <td class="label">{f.label}</td>
          <td class="bytes mono">{f.bytes}</td>
          <td class="meaning">{f.meaning}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .frame {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .summary {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    margin: 0;
  }

  table {
    border-collapse: collapse;
  }

  td {
    padding: 4px 14px 4px 0;
    border-top: 1px solid var(--border);
    vertical-align: top;
  }

  .compact td {
    padding-top: 2px;
    padding-bottom: 2px;
  }

  .label {
    color: var(--text-2);
    white-space: nowrap;
  }

  .bytes {
    font-size: 12px;
    white-space: nowrap;
  }

  .meaning {
    word-break: break-word;
  }

  .bad,
  tr.bad .meaning {
    color: var(--bad);
  }
</style>
