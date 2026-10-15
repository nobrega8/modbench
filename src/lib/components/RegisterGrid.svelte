<script lang="ts">
  import Select from "./Select.svelte";
  import { formatAddress, hex } from "#lib/format.ts";
  import { settings } from "#lib/settings.svelte.ts";
  import AddressFormat from "./AddressFormat.svelte";
  import { bench } from "#lib/state.svelte.ts";
  import type { ByteOrder } from "#lib/types.ts";

  type Format =
    | "unsigned"
    | "signed"
    | "hex"
    | "binary"
    | "ascii"
    | "int32"
    | "uint32"
    | "float32"
    | "bits";
  const formats: { id: Format; label: string }[] = [
    { id: "unsigned", label: "Unsigned" },
    { id: "signed", label: "Signed" },
    { id: "hex", label: "Hexadecimal" },
    { id: "binary", label: "Binary" },
    { id: "ascii", label: "ASCII" },
    { id: "int32", label: "32-bit signed" },
    { id: "uint32", label: "32-bit unsigned" },
    { id: "float32", label: "32-bit float" },
    { id: "bits", label: "Bits" },
  ];
  const orders: ByteOrder[] = ["ABCD", "CDAB", "BADC", "DCBA"];
  const PER_ROW = 10;

  let format = $state<Format>("unsigned");
  let order = $state<ByteOrder>("ABCD");

  const wide = $derived(
    format === "int32" || format === "uint32" || format === "float32",
  );
  const read = $derived(bench.read);
  const bitView = $derived(
    format === "bits" && read?.data.kind === "registers",
  );
  /** Bits of holding registers can be switched one at a time. */
  const writable = $derived(read?.area === "holding" && !settings.readOnly);
  let bitError = $state<string | null>(null);
  let writing = $state<string | null>(null);

  async function flip(address: number, bit: number, on: boolean) {
    bitError = null;
    writing = `${address}:${bit}`;
    try {
      await bench.writeBit(address, bit, on);
    } catch (e) {
      bitError = e instanceof Error ? e.message : String(e);
    } finally {
      writing = null;
    }
  }

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
    const fresh = (i: number) => bench.fresh(`g:${i}`);
    const out: Cell[] = [];
    if (read.data.kind === "bits") {
      read.data.values.forEach((b, i) =>
        out.push({
          offset: i,
          text: b ? "1" : "0",
          span: 1,
          changed: fresh(i),
        }),
      );
      return out;
    }
    const v = read.data.values;
    if (wide) {
      for (let i = 0; i + 1 < v.length; i += 2) {
        out.push({
          offset: i,
          text: pair(v[i], v[i + 1]),
          span: 2,
          changed: fresh(i) || fresh(i + 1),
        });
      }
      return out;
    }
    v.forEach((w, i) =>
      out.push({
        offset: i,
        text: word(w),
        span: 1,
        changed: fresh(i),
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
      <Select
        bind:value={format}
        label="Show as"
        options={formats.map((f) => ({ value: f.id, label: f.label }))}
      />
    </label>
    {#if wide}
      <label class="inline">
        <span>Byte order</span>
        <Select
          bind:value={order}
          label="Byte order"
          options={orders.map((o) => ({ value: o, label: o }))}
        />
      </label>
    {/if}
  {/if}
  <AddressFormat />
  <span class="spacer"></span>
  <button class="btn" disabled={!read} onclick={() => bench.copyReadToWrite()}
    >Copy data to write values</button
  >
</div>

{#if bitView && read?.data.kind === "registers"}
  {@const r = read}
  <p class="bit-hint">
    {writable
      ? "Click a bit to switch it: the register is read, the bit changed and the register written back."
      : read.area === "holding"
        ? "Read-only mode is on: bits cannot be switched."
        : "Input registers cannot be written."}
    {#if bitError}<span class="bad">{bitError}</span>{/if}
  </p>
  <div class="grid-wrap">
    <div class="bits">
      <span></span>
      {#each Array(16) as _, k (k)}<span class="colhead">{15 - k}</span>{/each}
      <span class="colhead">Hex</span>
      {#each read.data.values as w, i (i)}
        <span class="rowhead">{addr(r.address + i)}</span>
        {#each Array(16) as _, k (k)}
          {@const bit = 15 - k}
          {@const on = ((w >> bit) & 1) === 1}
          <button
            class="bit"
            class:on
            class:nibble={bit % 4 === 0 && bit > 0}
            class:busy={writing === `${r.address + i}:${bit}`}
            disabled={!writable || writing !== null}
            title="Bit {bit} of {addr(r.address + i)}: {on ? 1 : 0}"
            aria-label="Bit {bit} of address {addr(r.address + i)}, {on
              ? 'on'
              : 'off'}"
            onclick={() => flip(r.address + i, bit, !on)}>{on ? 1 : 0}</button
          >
        {/each}
        <span class="cell hexcell">{hex(w)}</span>
      {/each}
    </div>
  </div>
{:else if !read}
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

  /* Lit at once when the value changes, then fades back */
  .cell {
    transition: background-color 0.6s var(--ease);
  }

  .bit-hint {
    margin: 0 0 8px;
    color: var(--text-2);
    font-size: 12px;
  }

  .bit-hint .bad {
    margin-left: 8px;
    color: var(--bad);
  }

  .bits {
    display: grid;
    grid-template-columns: 58px repeat(16, 26px) 64px;
    gap: 3px;
    align-items: center;
  }

  .bit {
    height: 24px;
    padding: 0;
    border: 0;
    border-radius: 3px;
    background: var(--cell);
    font-family: var(--mono);
    font-size: 12px;
    color: var(--text-3);
    cursor: pointer;
  }

  .bit.nibble {
    margin-right: 5px;
  }

  .bit.on {
    background: var(--accent);
    color: var(--on-accent);
  }

  .bit:disabled {
    cursor: default;
  }

  .bit.busy {
    opacity: 0.5;
  }

  .bit:hover:not(:disabled) {
    box-shadow: inset 0 0 0 1px var(--text-3);
  }

  .hexcell {
    margin-left: 6px;
  }

  .cell.changed {
    background: var(--changed);
    transition: none;
  }
</style>
