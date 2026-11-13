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

/** Every part on one line, `name: value`, for a cell or a confirmation. */
export function partsLine(
  parts: Part[],
  values: (Value | null | undefined)[],
): string {
  return parts
    .map((p, i) => `${p.name}: ${partText(values[i]) || "—"}`)
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
    if (part.type === "string") {
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
