// User preferences, kept in this machine's webview storage.

export type Theme = "system" | "light" | "dark";

interface Stored {
  theme: Theme;
  rememberConnection: boolean;
  rememberDevices: boolean;
  confirmWrites: boolean;
  logLimit: number;
  checkUpdates: boolean;
}

const KEY = "modbench.settings";
const DEFAULTS: Stored = {
  theme: "system",
  rememberConnection: true,
  rememberDevices: true,
  confirmWrites: false,
  logLimit: 1000,
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
  logLimit = $state(1000);
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
      logLimit: this.logLimit,
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
