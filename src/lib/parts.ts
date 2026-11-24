// Parts of a block register (spec 4.10): named values inside one string
// register, read from the block and written whole in one request.

import { t } from "./i18n.svelte.ts";
import { parseBool, parseInteger } from "./format.ts";
import type { Part, Register, Value } from "./types.ts";

/** The register is a block with named parts. */
export function hasParts(reg: Register): reg is Register & { parts: Part[] } {
  return !!reg.parts?.length;
}

/** A part's value as shown: text as is, numbers and flags plainly. */
export function partText(value: Value | null | undefined): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "boolean") return value ? "1" : "0";
  return String(value);
}

/** The part is a bit field of a word, not a value in a block. */
export function isBitField(part: Part): boolean {
  return part.bit !== undefined && part.bit !== null;
}

/** A part's value with its label, when the bit field has one: `PLAY (2)`. */
export function partLabel(part: Part, value: Value | null | undefined): string {
  const text = partText(value);
  if (text === "" || !part.enum?.length) return text;
  const label = part.enum.find(([v]) => String(v) === text)?.[1];
  return label ? `${label} (${text})` : text;
}

/** Every part on one line, `name: value`, for a cell or a confirmation. */
export function partsLine(
  parts: Part[],
  values: (Value | null | undefined)[],
): string {
  return parts
    .map((p, i) => `${p.name}: ${partLabel(p, values[i]) || "—"}`)
    .join(" · ");
}

/** Most characters a part holds (2 per register for text). */
export function partMax(part: Part): number {
  return part.type === "string" ? part.length * 2 : 0;
}

/**
 * The value of each part from what was typed. An empty field is `null`,
 * which the device gets as zeros.
 */
export function parseParts(
  parts: Part[],
  texts: string[],
): { values: (Value | null)[] } | { error: string } {
  const values: (Value | null)[] = [];
  for (const [i, part] of parts.entries()) {
    const text = texts[i] ?? "";
    if (text === "") {
      values.push(null);
      continue;
    }
    if (isBitField(part)) {
      const n = parseInteger(text);
      const width = part.bits ?? 1;
      if (n === null || n < 0 || n >= 2 ** width)
        return {
          error: `${part.name}: ${t("fmt.number")} (0–${2 ** width - 1})`,
        };
      values.push(n);
    } else if (part.type === "string") {
      const max = partMax(part);
      if (text.length > max)
        return { error: t("parts.too_long", { name: part.name, max }) };
      values.push(text);
    } else if (part.type === "bool") {
      const b = parseBool(text);
      if (b === null) return { error: `${part.name}: ${t("fmt.bool")}` };
      values.push(b);
    } else {
      const asInt = parseInteger(text);
      const n = asInt ?? Number(text.trim().replace(",", "."));
      if (!Number.isFinite(n))
        return { error: `${part.name}: ${t("fmt.number")}` };
      values.push(n);
    }
  }
  return { values };
}
