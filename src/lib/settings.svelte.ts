// User preferences, kept in this machine's webview storage.

import type { AddressFormat } from "./format";

export type Theme = "system" | "light" | "dark";
export type CsvSeparator = "," | ";";

interface Stored {
  theme: Theme;
  rememberConnection: boolean;
  rememberDevices: boolean;
  confirmWrites: boolean;
  readOnly: boolean;
  logLimit: number;
  csvSeparator: CsvSeparator;
  addressFormat: AddressFormat;
  checkUpdates: boolean;
}

const KEY = "modbench.settings";
const DEFAULTS: Stored = {
  theme: "system",
  rememberConnection: true,
  rememberDevices: true,
  confirmWrites: false,
  readOnly: false,
  logLimit: 1000,
  csvSeparator: ",",
  addressFormat: "base0",
  checkUpdates: true,
};

function load(): Stored {
  try {
    const raw = localStorage.getItem(KEY);
    return {
      ...DEFAULTS,
      ...(raw ? (JSON.parse(raw) as Partial<Stored>) : {}),
    };
  } catch {
    return { ...DEFAULTS };
  }
}

class Settings {
  theme = $state<Theme>("system");
  rememberConnection = $state(true);
  rememberDevices = $state(true);
  confirmWrites = $state(false);
  /** Blocks every write; the backend refuses them while this is on. */
  readOnly = $state(false);
  logLimit = $state(1000);
  /** Comma, or semicolon for spreadsheets set up for a decimal comma. */
  csvSeparator = $state<CsvSeparator>(",");
  /** How the register grid, the map and the log show addresses. */
  addressFormat = $state<AddressFormat>("base0");
  checkUpdates = $state(true);

  constructor() {
    Object.assign(this, load());
  }

  save() {
    const data: Stored = {
      theme: this.theme,
      rememberConnection: this.rememberConnection,
      rememberDevices: this.rememberDevices,
      confirmWrites: this.confirmWrites,
      readOnly: this.readOnly,
      logLimit: this.logLimit,
      csvSeparator: this.csvSeparator,
      addressFormat: this.addressFormat,
      checkUpdates: this.checkUpdates,
    };
    try {
      localStorage.setItem(KEY, JSON.stringify(data));
    } catch {
      // Storage unavailable: settings last for this session only.
    }
  }

  /** Applies the theme to the page (data-theme on <html>). */
  applyTheme() {
    const root = document.documentElement;
    if (this.theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", this.theme);
  }
}

export const settings = new Settings();
