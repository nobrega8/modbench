// The map under construction in the map builder's window: its header, its
// registers, where it was saved, and what the validator says about it.

import { open, save } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "./api";
import {
  blankMeta,
  blankRegister,
  mapFileName,
  sortDiagnostics,
} from "./builder";
import { wordCount } from "./custom-registers";
import { t } from "./i18n.svelte.ts";
import { MAP_EXTENSIONS } from "./types";
import type { Diagnostic, Meta, Register } from "./types";

/** This page is the map builder's own window (see `builder_open_window`). */
export const IN_BUILDER_WINDOW =
  typeof location !== "undefined" &&
  new URLSearchParams(location.search).get("window") === "builder";

/** How long to wait after the last edit before the map is checked. */
const CHECK_DELAY_MS = 300;

class MapBuilder {
  meta = $state<Meta>(blankMeta());
  registers = $state<Register[]>([]);
  /** Where the map was opened from or last saved, if anywhere. */
  path = $state<string | null>(null);
  /** Edited since it was opened or saved. */
  dirty = $state(false);
  diagnostics = $state.raw<Diagnostic[]>([]);
  checking = $state(false);
  message = $state<{ ok: boolean; text: string } | null>(null);

  private seq = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;

  readonly sorted = $derived(sortDiagnostics(this.diagnostics));
  readonly errors = $derived(
    this.diagnostics.filter((d) => d.severity === "error").length,
  );
  readonly warnings = $derived(
    this.diagnostics.filter((d) => d.severity === "warning").length,
  );

  /** Something was edited: remember it and check the map soon. */
  touch() {
    this.dirty = true;
    this.message = null;
    this.schedule();
  }

  private schedule() {
    clearTimeout(this.timer);
    this.checking = true;
    this.timer = setTimeout(() => void this.check(), CHECK_DELAY_MS);
  }

  /** Runs the validator on the map as it is now. */
  async check() {
    const seq = ++this.seq;
    this.checking = true;
    try {
      const found = await api.validateMap(
        $state.snapshot({ meta: this.meta, registers: this.registers }),
      );
      // A later edit already started a newer check.
      if (seq === this.seq) this.diagnostics = found;
    } catch (e) {
      if (seq === this.seq) {
        this.diagnostics = [];
        this.message = { ok: false, text: errorText(e) };
      }
    } finally {
      if (seq === this.seq) this.checking = false;
    }
  }

  addRegister(after?: number) {
    const base = after === undefined ? undefined : this.registers[after];
    const reg = blankRegister(
      base
        ? {
            group: base.group,
            area: base.area,
            type: base.type,
            access: base.access,
            // The next free address after the one above.
            address: Math.min(
              0xffff,
              base.address + wordCount(base.type, base.length),
            ),
          }
        : {},
    );
    const at = after === undefined ? this.registers.length : after + 1;
    this.registers.splice(at, 0, reg);
    this.touch();
    return at;
  }

  duplicate(index: number) {
    const copy = $state.snapshot(this.registers[index]);
    this.registers.splice(index + 1, 0, copy);
    this.touch();
  }

  remove(index: number) {
    this.registers.splice(index, 1);
    this.touch();
  }

  move(index: number, by: -1 | 1) {
    const to = index + by;
    if (to < 0 || to >= this.registers.length) return;
    const [r] = this.registers.splice(index, 1);
    this.registers.splice(to, 0, r);
    this.touch();
  }

  /** Starts over with an empty map. */
  reset() {
    this.meta = blankMeta();
    this.registers = [];
    this.path = null;
    this.dirty = false;
    this.diagnostics = [];
    this.message = null;
    this.schedule();
  }

  /** Replaces the map with one opened from a file. */
  async openFile() {
    const picked = await open({
      multiple: false,
      filters: [
        { name: t("libraryview.modbus_map"), extensions: MAP_EXTENSIONS },
      ],
    });
    if (typeof picked !== "string") return;
    try {
      const parsed = await api.openMap(picked);
      if (!parsed.map) {
        this.diagnostics = parsed.diagnostics;
        this.message = { ok: false, text: t("builder.cannot_open") };
        return;
      }
      this.meta = parsed.map.meta;
      this.registers = parsed.map.registers;
      this.path = picked;
      this.dirty = false;
      this.message = null;
      this.diagnostics = parsed.diagnostics;
      this.schedule();
    } catch (e) {
      this.message = { ok: false, text: errorText(e) };
    }
  }

  /** Asks where to save, and writes the map as a version 2 file. */
  async saveAs() {
    const where = await save({
      defaultPath: this.path ?? mapFileName(this.meta),
      filters: [
        { name: t("libraryview.modbus_map"), extensions: MAP_EXTENSIONS },
      ],
    });
    if (!where) return;
    try {
      await api.saveMap(
        where,
        $state.snapshot({ meta: this.meta, registers: this.registers }),
      );
      this.path = where;
      this.dirty = false;
      this.message = { ok: true, text: t("builder.saved", { path: where }) };
    } catch (e) {
      this.message = {
        ok: false,
        text: t("builder.not_saved", { error: errorText(e) }),
      };
    }
  }
}

export const builder = new MapBuilder();
