import { afterEach, describe, expect, it } from "vitest";
import { localizeError } from "./errors";
import { i18n } from "./i18n.svelte";

afterEach(() => i18n.apply("en"));

describe("backend errors", () => {
  it("stay as they are in English", () => {
    for (const m of [
      "not connected",
      "timed out after 1000 ms",
      'invalid address "x"',
      "device exception 0x02 (illegal data address)",
      "device exception 0x02 (illegal data address) to FC03 (read holding registers) for 10 from address 100, unit 1",
      "something the system said",
    ])
      expect(localizeError(m)).toBe(m);
  });

  it("are worded in the current language, keeping the details", () => {
    i18n.apply("pt-PT");
    expect(localizeError("timed out after 1000 ms")).toBe(
      "tempo esgotado após 1000 ms",
    );
    expect(
      localizeError(
        "device exception 0x02 (illegal data address) to FC03 (read holding registers) for 10 from address 100, unit 1",
      ),
    ).toBe(
      "exceção do dispositivo 0x02 (endereço de dados inválido) a FC03 (Ler registos de retenção) para 10 a partir do endereço 100, unidade 1",
    );
    expect(localizeError("I/O error: Connection refused (os error 61)")).toBe(
      "erro de E/S: Connection refused (os error 61)",
    );
    expect(localizeError("something the system said")).toBe(
      "something the system said",
    );
  });
});
