// The backend words its errors in English (src-tauri/src/modbus.rs). The
// ones with a fixed shape are put in the current language for display;
// anything else (the system's own messages, map diagnostics) is shown as it
// came. Never use the result to decide what an error is: match the original.

import { CATALOGUES, i18n, td, t } from "./i18n.svelte.ts";

/** Function code of a function name as the backend words it, e.g. "read coils". */
function codeOf(name: string): number | null {
  for (const key of Object.keys(CATALOGUES.en)) {
    const m = /^fc\.(\d+)$/.exec(key);
    if (m && String(CATALOGUES.en[key]).toLowerCase() === name) return +m[1];
  }
  return null;
}

/** An exception's name as the backend words it, in the current language. */
function exceptionOf(name: string): string {
  for (const code of [1, 2, 3, 4, 5, 6, 8, 10, 11, "unknown"]) {
    if (CATALOGUES.en[`frame.exc.${code}`] === name)
      return td(`frame.exc.${code}`);
  }
  return name;
}

/** What a failed request asked for, appended to an exception message. */
function requestOf(tail: string): string {
  const m =
    /^ to FC(\d+) \((.+?)\)(?: for (\d+) from address (\d+))?, unit (\d+)$/.exec(
      tail,
    );
  if (!m) return tail;
  const code = codeOf(m[2]);
  const name = code === null ? m[2] : td(`fc.${code}`);
  const vars = { fc: m[1], name, unit: m[5] };
  return m[3] === undefined
    ? t("err.request", vars)
    : t("err.request_range", { ...vars, count: m[3], address: m[4] });
}

/** An error from the backend, worded in the current language where it can be. */
export function localizeError(message: string): string {
  // English is what the backend says
  if (i18n.locale === "en") return message;
  let m: RegExpExecArray | null;
  if (message === "not connected") return t("err.not_connected");
  if (message === "read-only mode is on: writes are blocked")
    return t("err.readonly");
  if ((m = /^timed out after (\d+) ms$/.exec(message)))
    return t("err.timeout", { ms: m[1] });
  if ((m = /^invalid address "(.*)"$/.exec(message)))
    return t("err.bad_address", { value: m[1] });
  if ((m = /^I\/O error: (.*)$/s.exec(message)))
    return t("err.io", { detail: m[1] });
  if ((m = /^serial port error: (.*)$/s.exec(message)))
    return t("err.serial", { detail: m[1] });
  if ((m = /^protocol error: (.*)$/s.exec(message)))
    return t("err.protocol", { detail: m[1] });
  if ((m = /^device exception 0x([0-9A-F]{2}) \((.+?)\)(.*)$/s.exec(message)))
    return t("err.exception", {
      code: m[1],
      name: exceptionOf(m[2]),
      request: requestOf(m[3]),
    });
  return message;
}
