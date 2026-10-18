// The display language: catalogues of strings per locale, the lookup with
// English as the fallback, and the choice of locale (the system's language,
// or one picked in Settings). See docs/i18n.md.

import en from "./locales/en.json";
import es from "./locales/es.json";
import de from "./locales/de.json";
import fr from "./locales/fr.json";
import it from "./locales/it.json";
import ptPT from "./locales/pt-PT.json";
import ptBR from "./locales/pt-BR.json";
import tr from "./locales/tr.json";
import pl from "./locales/pl.json";
import ru from "./locales/ru.json";
import hi from "./locales/hi.json";
import bn from "./locales/bn.json";
import ta from "./locales/ta.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import zhHans from "./locales/zh-Hans.json";
import zhHant from "./locales/zh-Hant.json";

export type Locale =
  | "en"
  | "es"
  | "de"
  | "fr"
  | "it"
  | "pt-PT"
  | "pt-BR"
  | "tr"
  | "pl"
  | "ru"
  | "hi"
  | "bn"
  | "ta"
  | "ja"
  | "ko"
  | "zh-Hans"
  | "zh-Hant";
/** A string, or its plural forms by Intl.PluralRules category. */
type Entry = string | Partial<Record<Intl.LDMLPluralRule, string>>;
export type Catalogue = Record<string, Entry>;

export const CATALOGUES: Record<Locale, Catalogue> = {
  en,
  es,
  de,
  fr,
  it,
  "pt-PT": ptPT,
  "pt-BR": ptBR,
  tr,
  pl,
  ru,
  hi,
  bn,
  ta,
  ja,
  ko,
  "zh-Hans": zhHans,
  "zh-Hant": zhHant,
};

/** Supported locales, each with its own name for the language. */
export const LOCALES: { id: Locale; name: string }[] = [
  { id: "en", name: "English" },
  { id: "es", name: "Español" },
  { id: "de", name: "Deutsch" },
  { id: "fr", name: "Français" },
  { id: "it", name: "Italiano" },
  { id: "pt-PT", name: "Português (Portugal)" },
  { id: "pt-BR", name: "Português (Brasil)" },
  { id: "tr", name: "Türkçe" },
  { id: "pl", name: "Polski" },
  { id: "ru", name: "Русский" },
  { id: "hi", name: "हिन्दी" },
  { id: "bn", name: "বাংলা" },
  { id: "ta", name: "தமிழ்" },
  { id: "ja", name: "日本語" },
  { id: "ko", name: "한국어" },
  { id: "zh-Hans", name: "简体中文" },
  { id: "zh-Hant", name: "繁體中文" },
];

/** Keys of the English catalogue: the only keys there are. */
export type Key = keyof typeof en;

/**
 * The first supported locale among the system's languages, most preferred
 * first, or English. Portuguese from Brazil gets pt-BR and any other
 * Portuguese pt-PT; Chinese in traditional script (zh-Hant, zh-TW, zh-HK,
 * zh-MO) gets zh-Hant and any other Chinese zh-Hans.
 */
export function detectLocale(languages: readonly string[]): Locale {
  for (const raw of languages) {
    const tag = raw.toLowerCase();
    const [lang] = tag.split("-");
    if (lang === "en") return "en";
    if (lang === "pt") return /-br\b/.test(tag) ? "pt-BR" : "pt-PT";
    if (lang === "zh") {
      return /-(hant|tw|hk|mo)\b/.test(tag) ? "zh-Hant" : "zh-Hans";
    }
    if (
      [
        "es",
        "de",
        "fr",
        "it",
        "tr",
        "pl",
        "ru",
        "hi",
        "bn",
        "ta",
        "ja",
        "ko",
      ].includes(lang)
    )
      return lang as Locale;
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

/** Like t(), for a key built at run time (checked by the tests, not by the type). */
export function td(key: string, vars: Record<string, string | number> = {}) {
  return translate(i18n.locale, key, vars);
}

/** The string for a key in the current language. */
export function t(key: Key, vars: Record<string, string | number> = {}) {
  return translate(i18n.locale, key, vars);
}
