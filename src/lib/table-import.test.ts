import { describe, expect, it } from "vitest";
import {
  addressFormat,
  guessColumns,
  hasHeader,
  importRows,
  parseAccess,
  parseArea,
  parseScale,
  parseTable,
  parseType,
} from "./table-import";

describe("parseTable", () => {
  it("splits a spreadsheet copy on tabs and drops blank lines", () => {
    expect(parseTable("Address\tName\r\n\r\n40001\tVoltage\n")).toEqual([
      ["Address", "Name"],
      ["40001", "Voltage"],
    ]);
  });

  it("reads CSV with quotes, and semicolons when the line has more of them", () => {
    expect(parseTable('0,"Volt, L1","say ""hi"""')).toEqual([
      ["0", "Volt, L1", 'say "hi"'],
    ]);
    expect(parseTable("Addr;Scale\n1;0,1")).toEqual([
      ["Addr", "Scale"],
      ["1", "0,1"],
    ]);
  });
});

describe("guessColumns", () => {
  it("names the columns from the header", () => {
    expect(
      guessColumns([
        "Register",
        "Parameter",
        "Data type",
        "No. of registers",
        "Multiplier",
        "Unit",
        "R/W",
        "Comment",
        "Something else",
      ]),
    ).toEqual([
      "address",
      "name",
      "type",
      "length",
      "scale",
      "unit",
      "access",
      "description",
      null,
    ]);
  });

  it("takes the description as the name when there is no name column", () => {
    expect(guessColumns(["Address", "Description", "Units"])).toEqual([
      "address",
      "name",
      "unit",
    ]);
  });

  it("tells a header row from a data row", () => {
    expect(hasHeader([["Address (hex)", "Name"]])).toBe(true);
    expect(hasHeader([["40001", "Voltage"]])).toBe(false);
  });
});

describe("cells", () => {
  it("reads areas without guessing single digits", () => {
    expect(parseArea("Holding Register")).toBe("holding");
    expect(parseArea("FC04")).toBe("input");
    expect(parseArea("3x")).toBe("input");
    expect(parseArea("03")).toBe("holding");
    expect(parseArea("Coil")).toBe("coil");
    expect(parseArea("1x")).toBe("discrete");
    expect(parseArea("3")).toBeNull();
  });

  it("reads the types manuals write", () => {
    expect(parseType("UINT16")).toBe("uint16");
    expect(parseType("S32")).toBe("int32");
    expect(parseType("IEEE754")).toBe("float32");
    expect(parseType("Float (2 regs)")).toBe("float32");
    expect(parseType("unsigned", 2)).toBe("uint32");
    expect(parseType("ASCII")).toBe("string");
    expect(parseType("BCD")).toBeNull();
  });

  it("reads access and scale", () => {
    expect(parseAccess("R/W")).toBe("RW");
    expect(parseAccess("read-only")).toBe("R");
    expect(parseScale("x0.1")).toBe(0.1);
    expect(parseScale("1/100")).toBe(0.01);
    expect(parseScale("0,01")).toBe(0.01);
    expect(parseScale("")).toBeNull();
  });

  it("recognises a Modicon address column", () => {
    expect(addressFormat(["40001", "40003"])).toBe("modicon");
    expect(addressFormat(["10", "40001"])).toBe("base0");
  });
});

describe("importRows", () => {
  const cols = guessColumns([
    "Address",
    "Name",
    "Type",
    "Scale",
    "Unit",
    "Access",
  ]);

  it("makes a register of each row, addresses as the manual numbers them", () => {
    const { registers, problems } = importRows(
      [
        ["101", "Voltage", "U16", "0.1", "V", "R"],
        ["103", "Energy", "U32", "", "kWh", "R/W"],
      ],
      cols,
      { base: 1, area: "holding" },
    );
    expect(problems.size).toBe(0);
    expect(
      registers.map((r) => [r.address, r.area, r.type, r.scale, r.access]),
    ).toEqual([
      [100, "holding", "uint16", 0.1, "R"],
      [102, "holding", "uint32", 1, "RW"],
    ]);
  });

  it("takes the area from Modicon addresses, and input registers are read-only", () => {
    const { registers } = importRows(
      [["30001", "Current", "float", "", "A", "RW"]],
      cols,
      { base: 0, area: "holding" },
    );
    expect(registers[0]).toMatchObject({
      area: "input",
      address: 0,
      type: "float32",
      access: "R",
    });
  });

  it("keeps rows it cannot read, and says what was wrong", () => {
    const { registers, problems } = importRows(
      [["abc", "Mystery", "BCD", "?", "", "maybe"]],
      cols,
      { base: 0, area: "holding" },
    );
    expect(registers).toHaveLength(1);
    expect(registers[0].type).toBe("uint16");
    expect(problems.get(0)).toEqual([
      "address: abc",
      "type: BCD",
      "scale: ?",
      "access: maybe",
    ]);
  });

  it("gives a string its length in registers", () => {
    const { registers } = importRows(
      [["0", "Serial", "ASCII", "8"]],
      ["address", "name", "type", "length"],
      { base: 0, area: "holding" },
    );
    expect(registers[0]).toMatchObject({ type: "string", length: 8 });
  });
});
