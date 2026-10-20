// The map under construction in the map builder's window: its header, its
// registers, where it was saved, and what the validator says about it.

import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "./api";
import {
  blankMeta,
  blankRegister,
  diffRegisters,
  findRegister,
  mapFileName,
  sortDiagnostics,
} from "./builder";
import { wordCount } from "./custom-registers";
import { t } from "./i18n.svelte.ts";
import { MAP_EXTENSIONS } from "./types";
import type { BuilderTarget, Diagnostic, Meta, Register } from "./types";

export const BUILT_IN_KEY = "x_built_in";

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
  /** The registers as they were opened, to show what changed. */
  original = $state.raw<Register[] | null>(null);
  /** The map is a file of the library: Save writes to it in place. */
  fromLibrary = $state(false);
  /** The register to scroll to and mark, after a map opened at one. */
  focus = $state<number | null>(null);
  message = $state<{ ok: boolean; text: string } | null>(null);

  private seq = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;

  readonly sorted = $derived(sortDiagnostics(this.diagnostics));
  readonly changes = $derived(
    this.original ? diffRegisters(this.original, this.registers) : null,
  );
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
    this.original = null;
    this.fromLibrary = false;
    this.focus = null;
    this.dirty = false;
    this.diagnostics = [];
    this.message = null;
    this.schedule();
  }

  /** True when it is fine to replace the map: nothing to lose, or the user agrees. */
  private async canReplace(): Promise<boolean> {
    if (!this.dirty) return true;
    return ask(t("builder.discard"), { kind: "warning" });
  }

  /** Starts over, unless that would lose changes the user wants to keep. */
  async startNew() {
    if (await this.canReplace()) this.reset();
  }

  /** Asks for a map file and opens it. */
  async openFile() {
    const picked = await open({
      multiple: false,
      filters: [
        { name: t("libraryview.modbus_map"), extensions: MAP_EXTENSIONS },
      ],
    });
    if (typeof picked !== "string") return;
    if (await this.canReplace()) await this.load(picked, null);
  }

  /** Opens what another window asked for (a map, and a register in it). */
  async openTarget(target: BuilderTarget) {
    if (await this.canReplace()) await this.load(target.path, target.register);
  }

  /** Replaces the map with the file at `path`, scrolled to a register. */
  private async load(path: string, at: BuilderTarget["register"]) {
    try {
      const parsed = await api.openMap(path);
      if (!parsed.map) {
        this.diagnostics = parsed.diagnostics;
        this.message = { ok: false, text: t("builder.cannot_open") };
        return;
      }
      this.meta = parsed.map.meta;
      this.registers = parsed.map.registers;
      this.original = $state.snapshot(parsed.map.registers);
      this.path = path;
      this.fromLibrary = await inLibrary(path);
      this.dirty = false;
      this.message = this.fromLibrary
        ? { ok: true, text: t("builder.library_map") }
        : null;
      this.diagnostics = parsed.diagnostics;
      const index = at ? findRegister(this.registers, at) : -1;
      this.focus = index >= 0 ? index : null;
      this.schedule();
    } catch (e) {
      this.message = { ok: false, text: errorText(e) };
    }
  }

  /** Tags the map with the version that built it (`x_built_in`, a custom key
   * of the spec); it shows in the header's extra keys, where it can be edited. */
  private async stamp() {
    let version: string;
    try {
      version = await api.appVersion();
    } catch {
      return;
    }
    const value = `modbench_v${version}`;
    const rest = this.meta.extra.filter(([k]) => k !== BUILT_IN_KEY);
    this.meta.extra = [...rest, [BUILT_IN_KEY, value]];
  }

  /**
   * Saves the map where it belongs: a library map over its own file, a map
   * from another file back to that file, a new map into the library under
   * its vendor and model (asking before it replaces one).
   */
  async save() {
    this.message = null;
    await this.stamp();
    const snapshot = $state.snapshot({
      meta: this.meta,
      registers: this.registers,
    });
    try {
      if (this.path && !this.fromLibrary) {
        await api.saveMap(this.path, snapshot);
        this.dirty = false;
        this.message = {
          ok: true,
          text: t("builder.saved", { path: this.path }),
        };
        return;
      }
      if (this.errors > 0) {
        this.message = { ok: false, text: t("builder.fix_errors") };
        return;
      }
      let outcome = await api.librarySaveMap(snapshot, this.path, false);
      if (outcome.exists) {
        const replace = await ask(
          t("builder.replace_in_library", {
            vendor: this.meta.vendor,
            model: this.meta.model,
          }),
          { kind: "warning" },
        );
        if (!replace) return;
        outcome = await api.librarySaveMap(snapshot, null, true);
      }
      this.path = outcome.path;
      this.fromLibrary = true;
      this.dirty = false;
      this.message = {
        ok: true,
        text: t("builder.saved_library", { path: outcome.path }),
      };
    } catch (e) {
      this.message = {
        ok: false,
        text: t("builder.not_saved", { error: errorText(e) }),
      };
    }
  }

  /** Asks where to save, and writes the map as a version 2 file. */
  async saveAs() {
    await this.stamp();
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
      this.fromLibrary = await inLibrary(where);
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

/** Whether a file is inside the library's folder. */
async function inLibrary(path: string): Promise<boolean> {
  try {
    const folder = (await api.libraryFolder()).replaceAll("\\", "/");
    return path.replaceAll("\\", "/").startsWith(folder + "/");
  } catch {
    return false;
  }
}

export const builder = new MapBuilder();
