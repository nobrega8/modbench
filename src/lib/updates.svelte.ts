// Update check shared by the update banner and the Settings tab.

import { check, type Update } from "@tauri-apps/plugin-updater";

class Updates {
  available = $state.raw<Update | null>(null);
  checking = $state(false);
  /** Result of the last check, for Settings. */
  message = $state<string | null>(null);

  async check(quiet: boolean) {
    this.checking = true;
    try {
      this.available = await check();
      this.message = this.available
        ? `Modbench ${this.available.version} is available.`
        : "Modbench is up to date.";
    } catch (e) {
      // No release published yet, or offline.
      if (!quiet) this.message = `Could not check for updates: ${String(e)}`;
    } finally {
      this.checking = false;
    }
  }
}

export const updates = new Updates();
