import { describe, expect, it } from "vitest";
import { extraRows, headerRows, mapStatusOf } from "./map-header";
import type { Meta } from "./types";

function meta(extra: Partial<Meta> = {}): Meta {
  return {
    vendor: "Acme",
    model: "PM-200",
    vendor_website: null,
    description: null,
    firmware: null,
    source: null,
    protocol: null,
    unit_id: null,
    port: null,
    baudrate: null,
    parity: null,
    data_bits: null,
    stop_bits: null,
    byte_order: null,
    address_base: null,
    max_read_registers: null,
    max_read_bits: null,
    enums: [],
    sentinels: [],
    extra: [],
    ...extra,
  };
}

describe("headerRows", () => {
  it("leaves out what the map does not say", () => {
    expect(headerRows(meta(), 3)).toEqual([["Registers", "3"]]);
  });

  it("lists the connection defaults and where the map comes from", () => {
    const rows = Object.fromEntries(
      headerRows(
        meta({
          models: ["PM-200", "PM-210"],
          revision: 2,
          revised: "2026-10-17",
          firmware_min: "1",
          protocol: "rtu",
          unit_id: 1,
          baudrate: 9600,
          parity: "N",
          address_base: 0,
          max_read_registers: 50,
          max_read_bits: 2000,
          vendor_website: "acme.example",
        }),
        120,
      ),
    );
    expect(rows).toMatchObject({
      Models: "PM-200, PM-210",
      Revision: "2, 2026-10-17",
      Firmware: "1 to latest",
      Registers: "120",
      Protocol: "Modbus RTU",
      "Unit ID": "1",
      Serial: "9600 baud, 8N1",
      Addresses: "0-based",
      "Max read": "50 registers, 2000 bits",
      Website: "acme.example",
    });
  });
});

describe("x_ keys", () => {
  it("reads the status and keeps the other keys apart", () => {
    const m = meta({
      extra: [
        ["x_status", "official"],
        ["x_origin", "omniconsole"],
      ],
    });
    expect(mapStatusOf(m)).toBe("official");
    expect(extraRows(m)).toEqual([["x_origin", "omniconsole"]]);
    expect(mapStatusOf(meta())).toBeNull();
  });
});
