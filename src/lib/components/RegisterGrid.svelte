<script lang="ts">
  import { formatAddress, hex } from "$lib/format";
  import { settings } from "$lib/settings.svelte";
  import AddressFormat from "./AddressFormat.svelte";
  import { bench } from "$lib/state.svelte";
  import type { ByteOrder } from "$lib/types";

  type Format =
    | "unsigned"
    | "signed"
    | "hex"
    | "binary"
    | "ascii"
    | "int32"
    | "uint32"
    | "float32";
  const formats: { id: Format; label: string }[] = [
    { id: "unsigned", label: "Unsigned" },
    { id: "signed", label: "Signed" },
    { id: "hex", label: "Hexadecimal" },
    { id: "binary", label: "Binary" },
    { id: "ascii", label: "ASCII" },
    { id: "int32", label: "32-bit signed" },
    { id: "uint32", label: "32-bit unsigned" },
    { id: "float32", label: "32-bit float" },
  ];
  const orders: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const PER_ROW = 10;

  let format = $state<Format>("unsigned");
  let order = $state<ByteOrder>("ABCD");

  const wide = $derived(
    format === "int32" || format === "uint32" || format === "float32",
  );
  const read = $derived(bench.read);

  function addr(n: number) {
    return formatAddress(n, settings.addressFormat, true);
  }

  function word(w: number): string {
    switch (format) {
      case "signed":
        return String(w > 0x7fff ? w - 0x10000 : w);
      case "hex":
        return hex(w);
      case "binary":
        return w
          .toString(2)
          .padStart(16, "0")
          .replace(/(.{4})(?!$)/g, "$1 ");
      case "ascii":
        return [w >> 8, w & 0xff]
          .map((c) => (c >= 32 && c < 127 ? String.fromCharCode(c) : "·"))
          .join("");
      default:
        return String(w);
    }
  }

  function pair(a: number, b: number): string {
    let regs = [a, b];
    if (order === "CDAB" || order === "DCBA") regs = regs.reverse();
    const swap = order === "BADC" || order === "DCBA";
    const view = new DataView(new ArrayBuffer(4));
    regs.forEach((w, k) =>
      view.setUint16(k * 2, swap ? ((w & 0xff) << 8) | (w >> 8) : w),
    );
    if (format === "int32") return String(view.getInt32(0));
    if (format === "uint32") return String(view.getUint32(0));
    return String(Number(view.getFloat32(0).toPrecision(7)));
  }

  interface Cell {
    offset: number;
    text: string;
    span: number;
    changed: boolean;
  }

  /** Cells in display order: one per register, or one per pair for 32-bit. */
  const cells = $derived.by((): Cell[] => {
    if (!read) return [];
    const prev = bench.previous;
    const out: Cell[] = [];
    if (read.data.kind === "bits") {
      const p = prev?.kind === "bits" ? prev.values : null;
      read.data.values.forEach((b, i) =>
        out.push({
          offset: i,
          text: b ? "1" : "0",
          span: 1,
          changed: !!p && p[i] !== b,
        }),
      );
      return out;
    }
    const v = read.data.values;
    const p = prev?.kind === "registers" ? prev.values : null;
    if (wide) {
      for (let i = 0; i + 1 < v.length; i += 2) {
        out.push({
          offset: i,
          text: pair(v[i], v[i + 1]),
          span: 2,
          changed: !!p && (p[i] !== v[i] || p[i + 1] !== v[i + 1]),
        });
      }
      return out;
    }
    v.forEach((w, i) =>
      out.push({
        offset: i,
        text: word(w),
        span: 1,
        changed: !!p && p[i] !== w,
      }),
    );
    return out;
  });

  const rows = $derived.by(() => {
    const out: Cell[][] = [];
    for (const c of cells) (out[Math.floor(c.offset / PER_ROW)] ??= []).push(c);
    return out;
  });
</script>

<div class="toolbar">
  {#if read?.data.kind !== "bits"}
    <label class="inline">
      <span>Show as</span>
      <select class="input" bind:value={format}>
        {#each formats as f (f.id)}<option value={f.id}>{f.label}</option
          >{/each}
      </select>
    </label>
    {#if wide}
      <label class="inline">
        <span>Byte order</span>
        <select class="input" bind:value={order}>
          {#each orders as o (o)}<option value={o}>{o}</option>{/each}
        </select>
      </label>
    {/if}
  {/if}
  <AddressFormat />
  <span class="spacer"></span>
  <button class="btn" disabled={!read} onclick={() => bench.copyReadToWrite()}
    >Copy data to write values</button
  >
</div>

{#if !read}
  <p class="empty">
    Values read from the device appear here. Choose a read command in step 2 and
    press Start.
  </p>
{:else}
  {@const r = read}
  <div class="grid-wrap">
    <div class="grid">
      <span></span>
      {#each Array(PER_ROW) as _, i (i)}<span class="colhead">+{i}</span>{/each}
      {#each rows as row, ri (ri)}
        <span class="rowhead">{addr(r.address + ri * PER_ROW)}</span>
        {#each row as c (c.offset)}
          <span
            class="cell"
            class:changed={c.changed}
            style:grid-column="span {c.span}"
            title="Address {addr(r.address + c.offset)}">{c.text}</span
          >
        {/each}
        {#if row.length * row[0].span < PER_ROW}
          <span style:grid-column="span {PER_ROW - row.length * row[0].span}"
          ></span>
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
    padding-bottom: 10px;
  }

  .inline {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
  }

  .spacer {
    flex: 1;
  }

  .empty {
    color: var(--text-2);
    margin: 8px 0;
  }

  .grid-wrap {
    overflow: auto;
    min-height: 0;
    flex: 1;
  }

  .grid {
    display: grid;
    grid-template-columns: 58px repeat(10, minmax(64px, 1fr));
    gap: 3px;
  }

  .colhead {
    font-size: 11px;
    color: var(--text-3);
    text-align: center;
  }

  .rowhead {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding-right: 6px;
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-2);
  }

  .cell {
    padding: 4px 6px;
    background: var(--cell);
    border-radius: 3px;
    font-family: var(--mono);
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }

  .cell.changed {
    background: var(--changed);
  }
</style>
