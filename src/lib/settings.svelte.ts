// User preferences, kept in this machine's webview storage.

import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { AddressFormat } from "./format";

export type Theme = "system" | "light" | "dark";
export type CsvSeparator = "," | ";";

interface Stored {
  theme: Theme;
  rememberConnection: boolean;
  rememberDevices: boolean;
  confirmWrites: boolean;
  readOnly: boolean;
  singleWriteFc16: boolean;
  autoReconnect: boolean;
  logLimit: number;
  csvSeparator: CsvSeparator;
  addressFormat: AddressFormat;
  highlightSeconds: number;
  checkUpdates: boolean;
  textScale: number;
}

const KEY = "modbench.settings";
const DEFAULTS: Stored = {
  theme: "system",
  rememberConnection: true,
  rememberDevices: true,
  confirmWrites: false,
  readOnly: false,
  singleWriteFc16: false,
  autoReconnect: true,
  logLimit: 1000,
  csvSeparator: ",",
  addressFormat: "base0",
  highlightSeconds: 2,
  checkUpdates: true,
  textScale: 1,
};

/** Smallest and largest text size, as a zoom factor of the whole window. */
export const TEXT_SCALE_MIN = 0.5;
export const TEXT_SCALE_MAX = 3;

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
  /** Writes a single holding register with FC16 instead of FC06. */
  singleWriteFc16 = $state(false);
  /** Opens a lost connection again while a test runs. */
  autoReconnect = $state(true);
  logLimit = $state(1000);
  /** Comma, or semicolon for spreadsheets set up for a decimal comma. */
  csvSeparator = $state<CsvSeparator>(",");
  /** How the register grid, the map and the log show addresses. */
  addressFormat = $state<AddressFormat>("base0");
  /** How long a changed value stays highlighted; 0 turns it off. */
  highlightSeconds = $state(2);
  checkUpdates = $state(true);
  /** Size of text and controls, 0.5 to 3 (1 = normal). */
  textScale = $state(1);

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
      singleWriteFc16: this.singleWriteFc16,
      autoReconnect: this.autoReconnect,
      logLimit: this.logLimit,
      csvSeparator: this.csvSeparator,
      addressFormat: this.addressFormat,
      highlightSeconds: this.highlightSeconds,
      checkUpdates: this.checkUpdates,
      textScale: this.textScale,
    };
    try {
      localStorage.setItem(KEY, JSON.stringify(data));
    } catch {
      // Storage unavailable: settings last for this session only.
    }
  }

  /** Zooms the window's content to the text size. The webview's own zoom
   * keeps the layout right, where CSS zoom would break viewport units. */
  async applyTextScale() {
    const scale = Math.min(
      TEXT_SCALE_MAX,
      Math.max(TEXT_SCALE_MIN, this.textScale || 1),
    );
    try {
      await getCurrentWebview().setZoom(scale);
    } catch {
      // Not running in Tauri (tests): nothing to zoom.
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
