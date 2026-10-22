// Formulas of computed values: arithmetic, comparison and bit operators and a
// few functions over the values of map registers, e.g.
//   Voltage_L1 * Current_L1 / 1000
//   [Energy import] - [Energy export]
//   round(Power / Power_max * 100, 1)
//
// A register is named by its name with every run of characters other than
// letters and digits written as "_" (Voltage L1 → Voltage_L1), or by its
// exact name in brackets: [Voltage L1].

import { t as tr } from "./i18n.svelte.ts";

export type Node =
  | { k: "num"; v: number }
  | { k: "var"; name: string }
  | { k: "un"; op: string; a: Node }
  | { k: "bin"; op: string; a: Node; b: Node }
  | { k: "call"; fn: string; args: Node[] };

export class FormulaError extends Error {}

const FUNCTIONS: Record<
  string,
  [min: number, max: number, (...x: number[]) => number]
> = {
  abs: [1, 1, Math.abs],
  min: [1, 16, Math.min],
  max: [1, 16, Math.max],
  round: [1, 2, (x, d = 0) => Math.round(x * 10 ** d) / 10 ** d],
  floor: [1, 1, Math.floor],
  ceil: [1, 1, Math.ceil],
  sqrt: [1, 1, Math.sqrt],
  pow: [2, 2, Math.pow],
  avg: [1, 16, (...x) => x.reduce((a, b) => a + b, 0) / x.length],
};

/** How a register is written in a formula without brackets. */
export function plainName(name: string): string {
  return name
    .trim()
    .replace(/[^\p{L}\p{N}]+/gu, "_")
    .replace(/^_|_$/g, "");
}

interface Token {
  t: "num" | "id" | "op" | "(" | ")" | "," | "end";
  v: string;
  at: number;
}

const OPS = [
  "**",
  "<<",
  ">>",
  "<=",
  ">=",
  "==",
  "!=",
  "&&",
  "||",
  "+",
  "-",
  "*",
  "/",
  "%",
  "<",
  ">",
  "&",
  "|",
  "^",
  "~",
  "!",
];

function tokenize(text: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    if (/\s/.test(c)) {
      i++;
      continue;
    }
    const at = i;
    if (/[0-9.]/.test(c)) {
      const m = /^(0x[0-9a-f]+|0b[01]+|\d*\.?\d+(?:e[+-]?\d+)?)/i.exec(
        text.slice(i),
      );
      if (!m)
        throw new FormulaError(tr("formula.unexpected", { c, at: i + 1 }));
      out.push({ t: "num", v: m[1], at });
      i += m[1].length;
    } else if (c === "[") {
      const end = text.indexOf("]", i);
      if (end < 0)
        throw new FormulaError(tr("formula.unclosed", { at: i + 1 }));
      out.push({ t: "id", v: text.slice(i + 1, end), at });
      i = end + 1;
    } else if (/[\p{L}_]/u.test(c)) {
      const m = /^[\p{L}_][\p{L}\p{N}_]*/u.exec(text.slice(i))!;
      out.push({ t: "id", v: m[0], at });
      i += m[0].length;
    } else if ("(),".includes(c)) {
      out.push({ t: c as "(" | ")" | ",", v: c, at });
      i++;
    } else {
      const op = OPS.find((o) => text.startsWith(o, i));
      if (!op)
        throw new FormulaError(tr("formula.unexpected", { c, at: i + 1 }));
      out.push({ t: "op", v: op, at });
      i += op.length;
    }
  }
  out.push({ t: "end", v: "", at: text.length });
  return out;
}

/** Binding power of binary operators, low to high. */
const BINARY: Record<string, number> = {
  "||": 1,
  "&&": 2,
  "|": 3,
  "^": 4,
  "&": 5,
  "==": 6,
  "!=": 6,
  "<": 7,
  "<=": 7,
  ">": 7,
  ">=": 7,
  "<<": 8,
  ">>": 8,
  "+": 9,
  "-": 9,
  "*": 10,
  "/": 10,
  "%": 10,
  "**": 12,
};

/** Parses a formula, or throws a FormulaError that says what is wrong. */
export function parse(text: string): Node {
  if (!text.trim()) throw new FormulaError(tr("formula.empty"));
  const tokens = tokenize(text);
  let p = 0;
  const peek = () => tokens[p];
  const next = () => tokens[p++];

  function expr(minBp: number): Node {
    let left = prefix();
    for (;;) {
      const t = peek();
      if (t.t !== "op" || !(t.v in BINARY)) break;
      const bp = BINARY[t.v];
      if (bp < minBp) break;
      next();
      // ** is right-associative
      const right = expr(t.v === "**" ? bp : bp + 1);
      left = { k: "bin", op: t.v, a: left, b: right };
    }
    return left;
  }

  function prefix(): Node {
    const t = next();
    if (t.t === "num") {
      const v = /^0b/i.test(t.v) ? parseInt(t.v.slice(2), 2) : Number(t.v);
      if (!Number.isFinite(v))
        throw new FormulaError(tr("formula.not_number", { v: t.v }));
      return { k: "num", v };
    }
    if (t.t === "op" && ["-", "+", "~", "!"].includes(t.v))
      return { k: "un", op: t.v, a: expr(11) };
    if (t.t === "(") {
      const inner = expr(0);
      if (next().t !== ")")
        throw new FormulaError(tr("formula.paren_open", { at: t.at + 1 }));
      return inner;
    }
    if (t.t === "id") {
      if (peek().t === "(") {
        const fn = t.v.toLowerCase();
        if (!(fn in FUNCTIONS))
          throw new FormulaError(
            tr("formula.unknown_function", {
              name: t.v,
              list: Object.keys(FUNCTIONS).join(", "),
            }),
          );
        next();
        const args: Node[] = [];
        if (peek().t !== ")") {
          for (;;) {
            args.push(expr(0));
            if (peek().t === ",") next();
            else break;
          }
        }
        if (next().t !== ")")
          throw new FormulaError(tr("formula.paren_call", { name: t.v }));
        const [lo, hi] = FUNCTIONS[fn];
        if (args.length < lo || args.length > hi)
          throw new FormulaError(
            lo === hi
              ? tr("formula.takes", { fn, count: lo })
              : tr("formula.takes_range", { fn, lo, hi }),
          );
        return { k: "call", fn, args };
      }
      return { k: "var", name: t.v };
    }
    if (t.t === "end") throw new FormulaError(tr("formula.too_early"));
    throw new FormulaError(tr("formula.unexpected", { c: t.v, at: t.at + 1 }));
  }

  const node = expr(0);
  const rest = peek();
  if (rest.t !== "end")
    throw new FormulaError(
      tr("formula.unexpected", { c: rest.v, at: rest.at + 1 }),
    );
  return node;
}

/** The register names a formula uses, as written. */
export function variables(node: Node, out = new Set<string>()): Set<string> {
  switch (node.k) {
    case "var":
      out.add(node.name);
      break;
    case "un":
      variables(node.a, out);
      break;
    case "bin":
      variables(node.a, out);
      variables(node.b, out);
      break;
    case "call":
      node.args.forEach((a) => variables(a, out));
  }
  return out;
}

/**
 * Resolves the names a formula uses to register names: exact names, or
 * their plain form. Throws for names that match no register (or several).
 */
export function resolve(node: Node, registers: string[]): Map<string, string> {
  const map = new Map<string, string>();
  const unknown: string[] = [];
  for (const written of variables(node)) {
    const exact = registers.find((r) => r === written);
    const plain = registers.filter(
      (r) => plainName(r).toLowerCase() === plainName(written).toLowerCase(),
    );
    if (exact) map.set(written, exact);
    else if (plain.length === 1) map.set(written, plain[0]);
    else if (plain.length > 1)
      throw new FormulaError(
        tr("formula.ambiguous", {
          name: written,
          list: plain.join(tr("formula.or")),
        }),
      );
    else unknown.push(written);
  }
  if (unknown.length)
    throw new FormulaError(
      tr("formula.no_register", {
        names: unknown.map((u) => `"${u}"`).join(", "),
      }),
    );
  return map;
}

const int = (x: number) => Math.trunc(x) | 0;

/** Evaluates a formula; `value` gives each register's value by its name. */
export function evaluate(node: Node, value: (name: string) => number): number {
  switch (node.k) {
    case "num":
      return node.v;
    case "var":
      return value(node.name);
    case "un": {
      const a = evaluate(node.a, value);
      return node.op === "-"
        ? -a
        : node.op === "~"
          ? ~int(a)
          : node.op === "!"
            ? Number(!a)
            : a;
    }
    case "call":
      return FUNCTIONS[node.fn][2](...node.args.map((x) => evaluate(x, value)));
    case "bin": {
      const a = evaluate(node.a, value);
      const b = evaluate(node.b, value);
      switch (node.op) {
        case "+":
          return a + b;
        case "-":
          return a - b;
        case "*":
          return a * b;
        case "/":
          return a / b;
        case "%":
          return a % b;
        case "**":
          return a ** b;
        case "<":
          return Number(a < b);
        case "<=":
          return Number(a <= b);
        case ">":
          return Number(a > b);
        case ">=":
          return Number(a >= b);
        case "==":
          return Number(a === b);
        case "!=":
          return Number(a !== b);
        case "&&":
          return Number(!!a && !!b);
        case "||":
          return Number(!!a || !!b);
        case "&":
          return int(a) & int(b);
        case "|":
          return int(a) | int(b);
        case "^":
          return int(a) ^ int(b);
        case "<<":
          return int(a) << int(b);
        case ">>":
          return int(a) >> int(b);
      }
    }
  }
  throw new FormulaError(tr("formula.unknown_op"));
}

/** Checks a formula against the map's registers: null when it is fine. */
export function formulaProblem(
  text: string,
  registers: string[],
): string | null {
  try {
    resolve(parse(text), registers);
    return null;
  } catch (e) {
    return e instanceof FormulaError ? e.message : String(e);
  }
}
