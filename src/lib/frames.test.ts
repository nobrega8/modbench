import { describe, expect, it } from "vitest";
import { crc16, decodeFrame, lrc, parseHex } from "./frames";

const labels = (f: { label: string }[]) => f.map((x) => x.label);

describe("frames", () => {
  it("computes the Modbus CRC and LRC", () => {
    // Read 1 holding register at 0 from unit 1: CRC 84 0A on the wire.
    expect(crc16([1, 3, 0, 0, 0, 1])).toBe(0x0a84);
    expect(lrc([0x11, 0x03, 0x00, 0x6b, 0x00, 0x02])).toBe(0x7f);
  });

  it("reads hex in the usual spellings", () => {
    expect(parseHex("01 03 00 00")).toEqual([1, 3, 0, 0]);
    expect(parseHex("0x01,0x03")).toEqual([1, 3]);
    expect(parseHex("0103 0A0b")).toEqual([1, 3, 10, 11]);
    expect(parseHex("01 3")).toBeNull();
    expect(parseHex("zz")).toBeNull();
  });

  it("splits an RTU request and checks its CRC", () => {
    const d = decodeFrame(
      parseHex("01 03 00 00 00 01 84 0A")!,
      "rtu",
      "request",
    );
    expect(labels(d.fields)).toEqual([
      "Unit ID",
      "Function",
      "Address",
      "Quantity",
      "CRC",
    ]);
    expect(d.summary).toBe("Read holding registers 0 × 1");
    expect(d.error).toBeNull();
    const broken = decodeFrame(
      parseHex("01 03 00 00 00 01 84 0B")!,
      "rtu",
      "request",
    );
    expect(broken.error).toMatch(/CRC/);
    expect(broken.fields.at(-1)!.bad).toBe(true);
  });

  it("reads a TCP response with its MBAP header", () => {
    const d = decodeFrame(
      parseHex("00 07 00 00 00 07 01 03 04 00 0A 01 02")!,
      "tcp",
      "response",
    );
    expect(labels(d.fields)).toEqual([
      "Transaction",
      "Protocol",
      "Length",
      "Unit ID",
      "Function",
      "Byte count",
      "Registers",
    ]);
    expect(d.fields.at(-1)!.meaning).toBe("10, 258");
    expect(d.summary).toBe("Read holding registers: 2 registers");
    expect(d.error).toBeNull();
  });

  it("names exceptions", () => {
    const d = decodeFrame(
      parseHex("00 01 00 00 00 03 01 83 02")!,
      "tcp",
      "response",
    );
    expect(d.summary).toBe("Exception 2 to read holding registers");
    expect(d.fields.at(-1)!.meaning).toBe("2 = illegal data address");
  });

  it("reads an ASCII frame and its LRC", () => {
    const bytes = [...":1103006B00027F\r\n"].map((c) => c.charCodeAt(0));
    const d = decodeFrame(bytes, "ascii", "request");
    expect(d.summary).toBe("Read holding registers 107 × 2");
    expect(d.fields.at(-1)).toMatchObject({ label: "LRC", meaning: "OK" });
  });

  it("reports a frame cut short", () => {
    const d = decodeFrame(
      parseHex("00 01 00 00 00 06 01 10 00 01")!,
      "tcp",
      "request",
    );
    expect(d.error).toMatch(/header says 6/);
  });

  it("reads device identification objects", () => {
    const pduBytes = parseHex(
      "01 2B 0E 01 01 00 00 02 00 04 41 43 4D 45 01 02 50 31",
    )!;
    const crc = crc16(pduBytes);
    const rtu = decodeFrame(
      [...pduBytes, crc & 0xff, crc >> 8],
      "rtu",
      "response",
    );
    expect(rtu.error).toBeNull();
    expect(rtu.fields.find((f) => f.label === "Object 0")!.meaning).toBe(
      '"ACME"',
    );
    expect(rtu.fields.find((f) => f.label === "Object 1")!.meaning).toBe(
      '"P1"',
    );
  });
});
