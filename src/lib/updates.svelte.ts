// Update check shared by the update banner and the Settings tab.

import { check, type Update } from "@tauri-apps/plugin-updater";
import { api } from "#lib/api.ts";
import { t } from "./i18n.svelte.ts";

/** Commands that update a copy owned by a package manager. */
const UPGRADE = {
  apt: "sudo apt update && sudo apt upgrade",
  flatpak: "flatpak update io.github.nobrega8.modbench",
};

class Updates {
  available = $state.raw<Update | null>(null);
  checking = $state(false);
  /** Result of the last check, for Settings. */
  message = $state<string | null>(null);
  /** Installed from the .deb or as a Flatpak: that package manager installs
   * updates, the app only reports them. */
  manager = $state<"apt" | "flatpak" | null>(null);

  /** Command that installs updates, when a package manager owns this copy. */
  get upgrade(): string | null {
    return this.manager && UPGRADE[this.manager];
  }

  constructor() {
    api.packageManager().then(
      (m) => (this.manager = m),
      () => {},
    );
  }

  async check(quiet: boolean) {
    this.checking = true;
    try {
      this.available = await check();
      this.message = this.available
        ? t("upd.available", { version: this.available.version })
        : t("upd.uptodate");
    } catch (e) {
      // No release published yet, or offline.
      if (!quiet) this.message = t("upd.failed", { error: String(e) });
    } finally {
      this.checking = false;
    }
  }
}

export const updates = new Updates();
