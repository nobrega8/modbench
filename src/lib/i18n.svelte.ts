// The display language: catalogues of strings per locale, the lookup with
// English as the fallback, and the choice of locale (the system's language,
// or one picked in Settings). See docs/i18n.md.

import en from "./locales/en.json";
import ptPT from "./locales/pt-PT.json";
import ru from "./locales/ru.json";
import zhHans from "./locales/zh-Hans.json";

export type Locale = "en" | "pt-PT" | "ru" | "zh-Hans";
/** A string, or its plural forms by Intl.PluralRules category. */
type Entry = string | Partial<Record<Intl.LDMLPluralRule, string>>;
export type Catalogue = Record<string, Entry>;

export const CATALOGUES: Record<Locale, Catalogue> = {
  en,
  "pt-PT": ptPT,
  ru,
  "zh-Hans": zhHans,
};

/** Supported locales, each with its own name for the language. */
export const LOCALES: { id: Locale; name: string }[] = [
  { id: "en", name: "English" },
  { id: "pt-PT", name: "Português (Portugal)" },
  { id: "ru", name: "Русский" },
  { id: "zh-Hans", name: "简体中文" },
];

/** Keys of the English catalogue: the only keys there are. */
export type Key = keyof typeof en;

/**
 * The first supported locale among the system's languages, most preferred
 * first, or English. Portuguese of any region gets pt-PT; Chinese only in
 * simplified script (zh, zh-CN, zh-SG, zh-Hans), not zh-TW or zh-HK.
 */
export function detectLocale(languages: readonly string[]): Locale {
  for (const raw of languages) {
    const tag = raw.toLowerCase();
    const [lang] = tag.split("-");
    if (lang === "en") return "en";
    if (lang === "pt") return "pt-PT";
    if (lang === "ru") return "ru";
    if (lang === "zh") {
      if (/-(hant|tw|hk|mo)\b/.test(tag)) continue;
      return "zh-Hans";
    }
  }
  return "en";
}

/** Replaces `{name}` with `vars.name`; unknown names stay as written. */
function fill(text: string, vars: Record<string, string | number>): string {
  return text.replace(/\{(\w+)\}/g, (m, k: string) =>
    k in vars ? String(vars[k]) : m,
  );
}

/** Looks a key up in a locale, then in English, then gives the key back. */
export function translate(
  locale: Locale,
  key: string,
  vars: Record<string, string | number> = {},
): string {
  const entry = CATALOGUES[locale][key] ?? (en as Catalogue)[key];
  if (entry === undefined) return key;
  if (typeof entry === "string") return fill(entry, vars);
  const n = Number(vars.count ?? 0);
  const form = new Intl.PluralRules(locale).select(n);
  const text =
    entry[form] ??
    entry.other ??
    ((en as Catalogue)[key] as Record<string, string>)?.other ??
    key;
  return fill(text, vars);
}

class I18n {
  locale = $state<Locale>("en");

  /** Uses a locale, or the system's language for "system". */
  apply(choice: Locale | "system") {
    this.locale =
      choice === "system"
        ? detectLocale(
            typeof navigator === "undefined" ? [] : navigator.languages,
          )
        : choice;
    try {
      document.documentElement.lang = this.locale;
    } catch {
      // No document (tests).
    }
  }
}

export const i18n = new I18n();

/** The string for a key in the current language. */
export function t(key: Key, vars: Record<string, string | number> = {}) {
  return translate(i18n.locale, key, vars);
}
