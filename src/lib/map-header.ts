// The header of a map (everything but its registers) as label/value rows,
// for the online library's details and the Explore window.

import type { Meta } from "./types";

const PROTOCOL = {
  tcp: "Modbus TCP",
  rtu: "Modbus RTU",
  "tcp+rtu": "Modbus TCP and RTU",
};

/** The x_status of a map, if it has one. */
export function mapStatusOf(meta: Meta): string | null {
  return meta.extra.find(([k]) => k === "x_status")?.[1] ?? null;
}

/** The general data of a map, as label/value pairs (empty ones left out). */
export function headerRows(meta: Meta, registers: number): [string, string][] {
  const m = meta;
  const serial = [
    m.baudrate ? `${m.baudrate} baud` : null,
    m.data_bits || m.parity || m.stop_bits
      ? `${m.data_bits ?? 8}${m.parity ?? "E"}${m.stop_bits ?? 1}`
      : null,
  ].filter(Boolean);
  const firmware = [
    m.firmware,
    m.firmware_min || m.firmware_max
      ? `${m.firmware_min ?? "any"} to ${m.firmware_max ?? "latest"}`
      : null,
  ].filter(Boolean);
  const maxRead = [
    m.max_read_registers ? `${m.max_read_registers} registers` : null,
    m.max_read_bits ? `${m.max_read_bits} bits` : null,
  ].filter(Boolean);
  const rows: [string, string | null | undefined][] = [
    ["Models", m.models?.length ? m.models.join(", ") : null],
    ["Description", m.description],
    [
      "Revision",
      m.revision ? `${m.revision}${m.revised ? `, ${m.revised}` : ""}` : null,
    ],
    ["Firmware", firmware.join("; ") || null],
    ["Registers", String(registers)],
    ["Protocol", m.protocol ? PROTOCOL[m.protocol] : null],
    ["Unit ID", m.unit_id?.toString()],
    ["TCP port", m.port?.toString()],
    ["Serial", serial.join(", ") || null],
    ["Byte order", m.byte_order],
    ["Addresses", m.address_base === null ? null : `${m.address_base}-based`],
    ["Max read", maxRead.join(", ") || null],
    ["Source", m.source],
    ["Website", m.vendor_website],
  ];
  return rows.filter((r): r is [string, string] => !!r[1]);
}

/** Keys of the file this version does not know (`x_…`), but x_status, which
    is shown as an icon. */
export function extraRows(meta: Meta): [string, string][] {
  return meta.extra.filter(([k]) => k !== "x_status");
}
