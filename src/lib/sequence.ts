// Pure parts of the test sequence tool: new steps, a step in words, and what
// is wrong with a step for a map (see `sequence.rs`, which runs them).

import type { DeviceMap, Sequence, SequenceStep } from "./types";

export const SEQUENCE_EXTENSIONS = ["mbtest.json", "json"];

export type StepKind = SequenceStep["do"];

/** A step of `kind` on `register` (the previous step's, or the map's first). */
export function newStep(kind: StepKind, register: string): SequenceStep {
  switch (kind) {
    case "write":
      return { do: "write", register, value: 0 };
    case "wait":
      return { do: "wait", ms: 1000 };
    case "expect":
      return { do: "expect", register, equals: 0, within_ms: 1000 };
    case "read":
      return { do: "read", register };
  }
}

/** The register a step works on, if any. */
export function stepRegister(s: SequenceStep): string | null {
  return s.do === "wait" ? null : s.register;
}

/** The step in a few words, as the report and the CLI write it. */
export function stepText(s: SequenceStep): string {
  switch (s.do) {
    case "write":
      return `write ${s.register} = ${s.value}`;
    case "wait":
      return `wait ${s.ms} ms`;
    case "read":
      return `read ${s.register}`;
    case "expect": {
      const within = s.within_ms > 0 ? ` within ${s.within_ms} ms` : "";
      let test = "anything";
      if (s.equals !== undefined)
        test = s.tolerance ? `= ${s.equals} ± ${s.tolerance}` : `= ${s.equals}`;
      else if (s.min !== undefined && s.max !== undefined)
        test = `in ${s.min}..${s.max}`;
      else if (s.min !== undefined) test = `≥ ${s.min}`;
      else if (s.max !== undefined) test = `≤ ${s.max}`;
      return `expect ${s.register} ${test}${within}`;
    }
  }
}

/** What would make a step fail before it reaches the device, or null. */
export function stepProblem(
  s: SequenceStep,
  map: DeviceMap | null,
): string | null {
  const name = stepRegister(s);
  if (name === null)
    return s.do === "wait" && s.ms < 0 ? "negative wait" : null;
  if (!map) return null;
  const r = map.registers.find((r) => r.name === name);
  if (!r) return `${name} is not in the map`;
  if (s.do === "write" && r.access === "R") return `${name} is read-only`;
  if (r.scale_factor) return `${name} takes its scale from another register`;
  return null;
}

export function writes(seq: Sequence): boolean {
  return seq.steps.some((s) => s.do === "write");
}

/** A sequence from a file's text, or an error that says why not. */
export function parseSequence(text: string): Sequence {
  const data = JSON.parse(text) as Partial<Sequence>;
  if (!Array.isArray(data.steps)) throw new Error("no steps in the file");
  const kinds = ["write", "wait", "expect", "read"];
  for (const s of data.steps)
    if (!kinds.includes((s as { do?: string }).do ?? ""))
      throw new Error(`unknown step: ${JSON.stringify(s)}`);
  return { name: data.name ?? "", steps: data.steps };
}
