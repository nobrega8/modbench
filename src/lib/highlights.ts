// Highlighted registers: a few colours to tell groups of registers apart,
// and how the list is ordered or filtered when only they matter. Highlights
// only describe the screen; they never read or write anything on a device.

import type { Area } from "./types";

/** The colours on offer, by number (the names are in the language files). */
export const MARKS = [1, 2, 3, 4] as const;
export type Mark = (typeof MARKS)[number];

/** Whether the list shows everything, the highlighted first, or only them. */
export type MarksView = "all" | "first" | "only";
export const MARKS_VIEWS: MarksView[] = ["all", "first", "only"];

export function isMark(n: unknown): n is Mark {
  return typeof n === "number" && (MARKS as readonly number[]).includes(n);
}

export function isMarksView(v: unknown): v is MarksView {
  return v === "all" || v === "first" || v === "only";
}

/** Registers that are read come before those that are not; in `first`
 * view the highlighted ones come first among each. */
export function compareRows(
  a: { index: number },
  b: { index: number },
  skipped: Set<number>,
  marks: Map<number, Mark>,
  view: MarksView,
): number {
  return (
    Number(skipped.has(a.index)) - Number(skipped.has(b.index)) ||
    (view === "first"
      ? Number(marks.has(b.index)) - Number(marks.has(a.index))
      : 0) ||
    a.index - b.index
  );
}

/** Whether a register is shown: all of them, or only the highlighted. */
export function shown(
  index: number,
  marks: Map<number, Mark>,
  view: MarksView,
): boolean {
  return view !== "only" || marks.has(index);
}

/** A word of a manual read, by area and address. */
export const wordKey = (area: Area, address: number) => `${area}:${address}`;
