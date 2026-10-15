// The app's own right-click menu. Parts of the page show their actions with
// `contextMenu.show(e, items)` from an `oncontextmenu` handler; elsewhere
// `fallback` gives text fields Cut, Copy, Paste and Select all, selected
// text Copy, and nothing at all instead of the webview's browser menu.

import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

export interface MenuItem {
  label: string;
  action: () => unknown;
  disabled?: boolean;
  /** Removes or deletes something. */
  danger?: boolean;
  /** Shown on the right, e.g. a shortcut. */
  hint?: string;
}

/** An item, or null for a separator. */
export type MenuEntry = MenuItem | null | false | undefined;

export interface OpenMenu {
  x: number;
  y: number;
  items: (MenuItem | null)[];
}

/** Separators only between items: none first, last or twice in a row. */
export function tidy(entries: MenuEntry[]): (MenuItem | null)[] {
  const out: (MenuItem | null)[] = [];
  for (const e of entries) {
    if (e === false || e === undefined) continue;
    if (e === null && (out.length === 0 || out.at(-1) === null)) continue;
    out.push(e);
  }
  while (out.at(-1) === null) out.pop();
  return out;
}

class ContextMenu {
  open = $state.raw<OpenMenu | null>(null);

  /** Shows `entries` where the pointer is, instead of any other menu. */
  show(e: MouseEvent, entries: MenuEntry[]) {
    e.preventDefault();
    e.stopPropagation();
    const items = tidy(entries);
    this.open = items.length ? { x: e.clientX, y: e.clientY, items } : null;
  }

  close() {
    this.open = null;
  }
}

export const contextMenu = new ContextMenu();

/** Puts text on the clipboard. */
export async function copy(text: string) {
  try {
    await writeText(text);
  } catch {
    // Not running under Tauri (tests, a plain browser).
    await navigator.clipboard?.writeText(text);
  }
}

const TEXT_TYPES = new Set([
  "text",
  "search",
  "number",
  "url",
  "email",
  "tel",
  "",
]);

function textField(
  target: EventTarget | null,
): HTMLInputElement | HTMLTextAreaElement | null {
  const el = (target as Element | null)?.closest?.("input, textarea");
  if (el instanceof HTMLTextAreaElement) return el;
  if (el instanceof HTMLInputElement && TEXT_TYPES.has(el.type)) return el;
  return null;
}

/** Cut, Copy, Paste and Select all for a text field. */
function fieldItems(
  field: HTMLInputElement | HTMLTextAreaElement,
): MenuEntry[] {
  // Number fields have no selection range: they act on the whole value.
  let start: number | null = null;
  let end: number | null = null;
  try {
    start = field.selectionStart;
    end = field.selectionEnd;
  } catch {
    // Not a field with a selection.
  }
  const ranged = start !== null && end !== null;
  const selected = ranged ? field.value.slice(start!, end!) : field.value;
  const editable = !field.disabled && !field.readOnly;

  /** Focuses the field with the selection it had when the menu opened. */
  const restore = () => {
    field.focus();
    if (ranged) field.setSelectionRange(start, end);
    else field.select();
  };
  return [
    {
      label: "Cut",
      disabled: !editable || selected === "",
      action: async () => {
        await copy(selected);
        restore();
        document.execCommand("delete");
      },
    },
    {
      label: "Copy",
      disabled: selected === "",
      action: () => copy(selected),
    },
    {
      label: "Paste",
      disabled: !editable,
      action: async () => {
        let text = "";
        try {
          text = await readText();
        } catch {
          text = (await navigator.clipboard?.readText()) ?? "";
        }
        restore();
        // As if typed: the field's input and change events follow.
        document.execCommand("insertText", false, text);
      },
    },
    null,
    {
      label: "Select all",
      disabled: field.value === "",
      action: () => {
        field.focus();
        field.select();
      },
    },
  ];
}

/**
 * For a right-click that no part of the page took: the text menu over a
 * text field or selected text, and no menu anywhere else. In development,
 * Shift + right-click keeps the webview's own menu (to inspect the page).
 */
export function fallback(e: MouseEvent) {
  if (e.defaultPrevented) return;
  if (import.meta.env.DEV && e.shiftKey) return;
  const field = textField(e.target);
  if (field) return contextMenu.show(e, fieldItems(field));
  const text = getSelection()?.toString() ?? "";
  if (text.trim()) {
    return contextMenu.show(e, [{ label: "Copy", action: () => copy(text) }]);
  }
  e.preventDefault();
  contextMenu.close();
}
