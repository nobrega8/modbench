// Update check shared by the update banner and the Settings tab.

import { check, type Update } from "@tauri-apps/plugin-updater";
import { api } from "#lib/api.ts";
import { t } from "./i18n.svelte.ts";

/** Command that updates a copy installed from the .deb package. */
export const APT_UPGRADE = "sudo apt update && sudo apt upgrade";

class Updates {
  available = $state.raw<Update | null>(null);
  checking = $state(false);
  /** Result of the last check, for Settings. */
  message = $state<string | null>(null);
  /** Installed from the .deb: apt installs updates, the app only reports them. */
  viaApt = $state(false);

  constructor() {
    api.installedWithApt().then(
      (apt) => (this.viaApt = apt),
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
