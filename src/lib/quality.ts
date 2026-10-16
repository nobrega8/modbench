// Notes for the log when map registers start failing or read again, so a
// gap in a chart or a recording can be traced to the register and the
// error.

import type { PointResult, Register } from "./types";

/** Names listed in one note before the rest are counted. */
const LISTED = 3;

function names(regs: Register[]): string {
  const shown = regs.slice(0, LISTED).map((r) => r.name);
  const more = regs.length - shown.length;
  return more > 0 ? `${shown.join(", ")} and ${more} more` : shown.join(", ");
}

/**
 * What changed between the last results and this cycle's: registers whose
 * reads now fail (grouped by error) and registers that read again. A
 * register seen for the first time only counts when it fails.
 */
export function qualityNotes(
  registers: Register[],
  before: Map<number, PointResult>,
  results: PointResult[],
): string[] {
  const failing = new Map<string, Register[]>();
  const back: Register[] = [];
  for (const r of results) {
    const reg = registers[r.index];
    if (!reg) continue;
    const was = before.get(r.index);
    if (r.error && !was?.error)
      (failing.get(r.error) ?? failing.set(r.error, []).get(r.error)!).push(
        reg,
      );
    else if (!r.error && was?.error) back.push(reg);
  }
  const notes = [...failing].map(
    ([error, regs]) => `${names(regs)}: read failed (${error})`,
  );
  if (back.length) notes.push(`${names(back)}: read again`);
  return notes;
}
