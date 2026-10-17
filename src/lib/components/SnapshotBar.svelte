<!--
  Snapshot controls for the Device map and Read registers tabs: take one,
  save or open one, compare it with the live values or with a second
  snapshot, and show only what differs.
-->
<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, errorText } from "#lib/api.ts";
  import { clock } from "#lib/format.ts";
  import { contextMenu } from "#lib/contextmenu.svelte.ts";
  import {
    SNAPSHOT_EXTENSION,
    parseSnapshot,
    snapshotFile,
    type Snapshot,
  } from "#lib/snapshots.ts";
  import { bench } from "#lib/state.svelte.ts";

  let {
    differences,
    canTake,
    kind,
  }: {
    /** Values that differ now, or null without a snapshot. */
    differences: number | null;
    /** There are values to keep. */
    canTake: boolean;
    /** What this tab compares: a map's registers or a manual read. */
    kind: "map" | "read";
  } = $props();

  let note = $state<{ ok: boolean; text: string } | null>(null);

  const fits = (s: Snapshot) => (kind === "map" ? !!s.map : !!s.read);

  async function saveSnapshot() {
    const s = bench.snapshot;
    if (!s) return;
    note = null;
    const path = await save({
      defaultPath: `snapshot.${SNAPSHOT_EXTENSION}`,
      filters: [{ name: "Snapshot", extensions: ["json"] }],
    });
    if (!path) return;
    try {
      await api.saveText(path, snapshotFile(s));
      note = { ok: true, text: "Saved." };
    } catch (e) {
      note = { ok: false, text: `Not saved: ${errorText(e)}` };
    }
  }

  /** Opens a snapshot file: as the snapshot, or as what it is compared with. */
  async function openSnapshot(second: boolean) {
    note = null;
    const path = await open({
      filters: [{ name: "Snapshot", extensions: ["json"] }],
    });
    if (typeof path !== "string") return;
    try {
      const s = parseSnapshot(await api.readText(path));
      if ("error" in s) throw new Error(s.error);
      if (!fits(s))
        throw new Error(
          kind === "map"
            ? "This snapshot is of a manual read, not of a device map."
            : "This snapshot is of a device map, not of a manual read.",
        );
      s.label = path.split(/[\\/]/).pop() ?? s.label;
      if (second) bench.compareWith = s;
      else {
        bench.snapshot = s;
        bench.compareWith = null;
      }
    } catch (e) {
      note = { ok: false, text: errorText(e) };
    }
  }

  function menu(e: MouseEvent) {
    const has = !!bench.snapshot && fits(bench.snapshot);
    contextMenu.show(e, [
      {
        label: has ? "Take a new snapshot" : "Take snapshot",
        disabled: !canTake,
        action: () => bench.takeSnapshot(),
      },
      { label: "Open snapshot…", action: () => openSnapshot(false) },
      has && { label: "Save snapshot…", action: saveSnapshot },
      null,
      has && {
        label: "Compare with the live values",
        disabled: !bench.compareWith,
        action: () => (bench.compareWith = null),
      },
      has && {
        label: "Compare with a saved snapshot…",
        action: () => openSnapshot(true),
      },
      null,
      has && { label: "Clear snapshot", action: () => bench.clearSnapshot() },
    ]);
  }
</script>

<button
  class="btn small"
  aria-haspopup="menu"
  title="Keep the values shown now, to compare with later values or with another snapshot"
  onclick={menu}>Snapshot…</button
>
{#if bench.snapshot && fits(bench.snapshot) && differences !== null}
  <span class="snap" role="status">
    <span class="mono"
      >{bench.snapshot.label || clock(bench.snapshot.taken_ms)}</span
    >
    vs {bench.compareWith ? bench.compareWith.label : "live"} ·
    <strong class:differ={differences > 0}
      >{differences === 0
        ? "no differences"
        : `${differences} differ${differences === 1 ? "s" : ""}`}</strong
    >
  </span>
  <label class="only">
    <input type="checkbox" bind:checked={bench.onlyDifferences} />
    Only differences
  </label>
{/if}
{#if note}<span class="note" class:bad={!note.ok}>{note.text}</span>{/if}

<style>
  .snap {
    font-size: 12px;
    color: var(--text-2);
  }

  .differ {
    color: var(--text);
  }

  .only {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--text-2);
  }

  .note {
    font-size: 12px;
    color: var(--text-3);
  }

  .note.bad {
    color: var(--bad);
  }
</style>
