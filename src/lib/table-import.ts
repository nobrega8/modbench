// A register table pasted from a spreadsheet or a PDF, or read from a CSV
// file, turned into map builder registers: the cells, which column holds
// what (guessed from the header row), and each cell read the way device
// manuals write it.

import { blankRegister, wireAddress, withArea } from "./builder";
import { parseAddress, parseInteger, type AddressFormat } from "./format";
import type { Access, Area, DataType, Register } from "./types";

/** What a column of the table holds. */
export type Column =
  | "name"
  | "group"
  | "area"
  | "address"
  | "type"
  | "length"
  | "scale"
  | "unit"
  | "access"
  | "description";

export const COLUMNS: Column[] = [
  "name",
  "group",
  "area",
  "address",
  "type",
  "length",
  "scale",
  "unit",
  "access",
  "description",
];

/**
 * The cells of a table. Tabs separate them when the text has any (a copy from
 * a spreadsheet), else semicolons or commas, whichever the first line has
 * more of; CSV quoting ("a, b" and "" for a quote) is understood. Blank lines
 * are left out.
 */
export function parseTable(text: string): string[][] {
  const first = text.split(/\r?\n/, 1)[0] ?? "";
  const count = (c: string) => first.split(c).length - 1;
  const sep = text.includes("\t") ? "\t" : count(";") > count(",") ? ";" : ",";
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  const endCell = () => {
    row.push(cell.trim());
    cell = "";
  };
  const endRow = () => {
    endCell();
    if (row.some((c) => c !== "")) rows.push(row);
    row = [];
  };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c === '"' && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (c === '"') quoted = false;
      else cell += c;
    } else if (c === '"' && cell.trim() === "") {
      quoted = true;
      cell = "";
    } else if (c === sep) endCell();
    else if (c === "\n") endRow();
    else if (c !== "\r") cell += c;
  }
  endRow();
  return rows;
}

/** Header words that name each column, as manuals and spreadsheets write them. */
const HEADER: [Column, RegExp][] = [
  [
    "address",
    /^(start(ing)?\s*)?(address|addr|reg(ister)?\s*(no|nr|#|address)?|offset\s*address|adr|modbus\s*(address|register))\.?$/i,
  ],
  [
    "name",
    /^(name|parameter|variable|tag|signal|point|quantity|measurement|register\s*name)$/i,
  ],
  ["group", /^(group|category|section|block)$/i],
  [
    "area",
    /^(area|table|register\s*type|object\s*type|function(\s*code)?|fc|reg\.?\s*type)$/i,
  ],
  ["type", /^(data\s*)?(type|format)$|^data$/i],
  [
    "length",
    /^(length|len|size|words|regs|registers|no\.?\s*of\s*(registers|words)|quantity\s*of\s*registers|count|number\s*of\s*registers)$/i,
  ],
  [
    "scale",
    /^(scale|scaling|factor|multiplier|gain|resolution|scale\s*factor)$/i,
  ],
  ["unit", /^(unit|units|uom|eng\.?\s*unit)$/i],
  ["access", /^(access|r\/?w|read\s*\/?\s*write|mode|permission|attribute)$/i],
  [
    "description",
    /^(description|desc|comment|comments|remark|remarks|notes?|details|meaning)$/i,
  ],
];

/** The column each header cell names, or null for one to leave out. A table
 * with a description but no name column takes the description as the name,
 * since every register needs one. */
export function guessColumns(header: string[]): (Column | null)[] {
  const taken = new Set<Column>();
  const cols = header.map((cell) => {
    const text = cell.replace(/\s*\(.*\)\s*$/, "").trim();
    const hit = HEADER.find(([col, re]) => !taken.has(col) && re.test(text));
    if (!hit) return null;
    taken.add(hit[0]);
    return hit[0];
  });
  if (!taken.has("name") && taken.has("description"))
    cols[cols.indexOf("description")] = "name";
  return cols;
}

/** True when the first row reads as a header: it names an address column and
 * one more, and its address cell is not an address itself. */
export function hasHeader(rows: string[][]): boolean {
  if (rows.length === 0) return false;
  const cols = guessColumns(rows[0]);
  const at = cols.indexOf("address");
  return (
    at >= 0 &&
    cols.filter(Boolean).length >= 2 &&
    parseAddress(rows[0][at], "base0") === null
  );
}

/**
 * The area a cell names: words, `4x`-style prefixes or a two-digit read
 * function code (`03`, `FC04`). A single digit is not read, since `3` is an
 * input register as a prefix but a holding register as a function code.
 */
export function parseArea(text: string): Area | null {
  const t = text
    .trim()
    .toLowerCase()
    .replace(/[\s_-]+/g, " ");
  const areas: [Area, RegExp][] = [
    [
      "holding",
      /^(holding( registers?)?|hr|hreg|4x|4xxxx|(fc ?)?03h?|fc ?3|analog outputs?|output registers?)$/,
    ],
    [
      "input",
      /^(input registers?|inputs?|ir|ireg|3x|3xxxx|(fc ?)?04h?|fc ?4|analog inputs?)$/,
    ],
    ["coil", /^(coils?|co|0x|0xxxx|(fc ?)?01h?|fc ?1|digital outputs?|do)$/],
    [
      "discrete",
      /^(discrete( inputs?)?|di|1x|1xxxx|(fc ?)?02h?|fc ?2|digital inputs?|input status)$/,
    ],
  ];
  return areas.find(([, re]) => re.test(t))?.[0] ?? null;
}

/** The type a cell names, in the many ways manuals write them; null if unknown.
 * `words` (registers per value), when the table gives it, settles types that
 * do not say their size, like `unsigned` or `int`. */
export function parseType(
  text: string,
  words: number | null = null,
): DataType | null {
  const t = text
    .toLowerCase()
    .replace(/\(.*?\)/g, "")
    .replace(/[\s_-]+/g, "");
  if (t === "") return null;
  const sized = (n16: DataType, n32: DataType, n64: DataType) =>
    words === 2 ? n32 : words === 4 ? n64 : n16;
  if (/^(bool|boolean|bit|bits)$/.test(t)) return "bool";
  if (/^(string|str|ascii|char|chars|text|asciistring)/.test(t))
    return "string";
  if (/^(float64|double|f64|lreal|ieee754double|real64|fp64)$/.test(t))
    return "float64";
  if (
    /^(float32|float|f32|real|single|ieee754|ieee754float|ieee754single|fp32|fl32|real32)$/.test(
      t,
    )
  )
    return words === 4 ? "float64" : "float32";
  if (/^(uint64|u64|unsigned64|ulonglong|uint8x8|qword)$/.test(t))
    return "uint64";
  if (/^(int64|s64|i64|signed64|longlong|sint64)$/.test(t)) return "int64";
  if (/^(uint32|u32|unsigned32|udint|dword|ulong|unsignedlong)$/.test(t))
    return "uint32";
  if (/^(int32|s32|i32|signed32|dint|long|sint32|signedlong)$/.test(t))
    return "int32";
  if (
    /^(uint16|u16|unsigned16|uint|word|ushort|unsignedshort|unsigned|u)$/.test(
      t,
    )
  )
    return sized("uint16", "uint32", "uint64");
  if (
    /^(int16|s16|i16|signed16|int|short|sint|signed|s|sint16|integer)$/.test(t)
  )
    return sized("int16", "int32", "int64");
  return null;
}

/** The access a cell names (`R`, `RO`, `R/W`, `read/write`…), or null. */
export function parseAccess(text: string): Access | null {
  const t = text.toLowerCase().replace(/[\s_-]+/g, "");
  if (/^(r|ro|read|readonly|rd)$/.test(t)) return "R";
  if (/^(w|wo|write|writeonly|wr)$/.test(t)) return "W";
  if (/^(rw|r\/w|w\/r|read\/write|readwrite|rd\/wr)$/.test(t)) return "RW";
  return null;
}

/** A scale as manuals write it: `0.1`, `x0.1`, `×10`, `1/10`, `/100`, `0,01`. */
export function parseScale(text: string): number | null {
  const t = text
    .trim()
    .replace(/^[x×*]\s*/i, "")
    .replace(",", ".");
  if (t === "") return null;
  const div = /^(\d*\.?\d*)?\s*\/\s*(\d+\.?\d*)$/.exec(t);
  if (div) {
    const n = div[1] ? Number(div[1]) : 1;
    const d = Number(div[2]);
    return d && Number.isFinite(n) ? n / d : null;
  }
  const n = Number(t);
  return Number.isFinite(n) && n !== 0 ? n : null;
}

/** How the address column is written: Modicon (`40001`) when every address
 * is five or six digits that start with 0, 1, 3 or 4 and one is 30001 or
 * more, else as numbers. */
export function addressFormat(cells: string[]): AddressFormat {
  const filled = cells.map((c) => c.trim()).filter(Boolean);
  const modicon =
    filled.length > 0 &&
    filled.every((c) => /^[0134]\d{4,5}$/.test(c)) &&
    filled.some((c) => Number(c) >= 30001);
  return modicon ? "modicon" : "base0";
}

export interface ImportOptions {
  /** How the map numbers addresses (see `address_base`). */
  base: number;
  /** The area of rows that do not name one. */
  area: Area;
}

export interface Imported {
  registers: Register[];
  /** Problems by row of the result: what could not be read, kept as default. */
  problems: Map<number, string[]>;
}

/**
 * Registers from the rows of a table (without its header) and the column each
 * cell belongs to. Every row becomes a register: a cell that cannot be read
 * leaves the field at its default and is reported for that row.
 */
export function importRows(
  rows: string[][],
  columns: (Column | null)[],
  options: ImportOptions,
): Imported {
  const at = (col: Column) => columns.indexOf(col);
  const cell = (row: string[], col: Column) =>
    at(col) >= 0 ? (row[at(col)] ?? "").trim() : "";
  const format = addressFormat(rows.map((r) => cell(r, "address")));
  const registers: Register[] = [];
  const problems = new Map<number, string[]>();

  rows.forEach((row, i) => {
    const bad: string[] = [];
    const note = (col: Column, text: string) =>
      text !== "" && bad.push(`${col}: ${text}`);

    const areaText = cell(row, "area");
    let area = parseArea(areaText);
    if (area === null) note("area", areaText);

    const addressText = cell(row, "address");
    const parsed = parseAddress(addressText, format);
    let address = 0;
    if (parsed) {
      // A Modicon or IEC address is already on the wire and names its area.
      const wire = parsed.area
        ? parsed.address
        : wireAddress(parsed.address, options.base);
      if (wire === null) bad.push(`address: ${addressText}`);
      else address = wire;
      area = area ?? parsed.area;
    } else bad.push(`address: ${addressText || "—"}`);

    const lengthText = cell(row, "length");
    const length = lengthText === "" ? null : parseInteger(lengthText);
    if (lengthText !== "" && (length === null || length < 1))
      note("length", lengthText);
    const words = length !== null && length >= 1 ? length : null;

    const typeText = cell(row, "type");
    const type = parseType(typeText, words);
    if (type === null) note("type", typeText);

    const scaleText = cell(row, "scale");
    const scale = parseScale(scaleText);
    if (scale === null) note("scale", scaleText);

    const accessText = cell(row, "access");
    const access = parseAccess(accessText);
    if (access === null) note("access", accessText);

    let reg = blankRegister({
      name: cell(row, "name"),
      group: cell(row, "group"),
      address,
      unit: cell(row, "unit"),
      description: cell(row, "description"),
      scale: scale ?? 1,
      access: access ?? "R",
    });
    reg = withArea(reg, area ?? options.area);
    if (type && reg.type !== "bool") {
      // A bool in a word area needs a bit, which tables do not give.
      reg.type = type === "bool" ? "uint16" : type;
      if (type === "bool") note("type", typeText);
    }
    if (reg.type === "string") reg.length = words ?? 1;
    if (bad.length) problems.set(i, bad);
    registers.push(reg);
  });
  return { registers, problems };
}
