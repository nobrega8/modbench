import { describe, expect, it } from "vitest";
import {
  CATALOGUES,
  LOCALES,
  detectLocale,
  translate,
  type Catalogue,
} from "./i18n.svelte";

const en = CATALOGUES.en;
const placeholders = (e: Catalogue[string]) =>
  [
    ...new Set(
      (typeof e === "string" ? [e] : Object.values(e)).flatMap((x) =>
        [...(x ?? "").matchAll(/\{(\w+)\}/g)].map((m) => m[1]),
      ),
    ),
  ].sort();

describe("catalogues", () => {
  for (const { id } of LOCALES) {
    it(`${id} has only English keys, with the same placeholders`, () => {
      const cat = CATALOGUES[id];
      for (const [key, entry] of Object.entries(cat)) {
        expect(en, `${id}: unknown key ${key}`).toHaveProperty([key]);
        expect(placeholders(entry), `${id}: ${key}`).toEqual(
          placeholders(en[key]),
        );
        expect(typeof entry === "string").toBe(typeof en[key] === "string");
      }
    });
  }

  it("cover every key of English in the shipped translations", () => {
    for (const { id } of LOCALES) {
      const missing = Object.keys(en).filter((k) => !(k in CATALOGUES[id]));
      expect(missing, id).toEqual([]);
    }
  });
});

describe("lookup", () => {
  it("fills placeholders, picks plural forms and falls back to English", () => {
    expect(translate("pt-PT", "test.running", { where: "COM3" })).toBe(
      "A correr · COM3",
    );
    expect(translate("ru", "count.registers", { count: 1 })).toBe("1 регистр");
    expect(translate("ru", "count.registers", { count: 3 })).toBe("3 регистра");
    expect(translate("ru", "count.registers", { count: 5 })).toBe(
      "5 регистров",
    );
    expect(translate("en", "count.registers", { count: 2 })).toBe(
      "2 registers",
    );
    expect(translate("zh-Hans", "count.registers", { count: 2 })).toBe(
      "2 个寄存器",
    );
    expect(translate("pt-PT", "no.such.key")).toBe("no.such.key");
  });
});

describe("system language", () => {
  it("picks the first supported language, English otherwise", () => {
    expect(detectLocale(["pt-BR", "en-US"])).toBe("pt-PT");
    expect(detectLocale(["de-DE", "ru-RU"])).toBe("ru");
    expect(detectLocale(["zh-CN"])).toBe("zh-Hans");
    expect(detectLocale(["zh-Hans-SG"])).toBe("zh-Hans");
    expect(detectLocale(["zh-TW", "fr"])).toBe("en");
    expect(detectLocale(["zh-Hant-HK", "pt"])).toBe("pt-PT");
    expect(detectLocale([])).toBe("en");
  });
});
