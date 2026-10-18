// The header of a map (everything but its registers) as label/value rows,
// for the online library's details and the Explore window.

import type { Meta } from "./types";
import { t } from "./i18n.svelte.ts";

const PROTOCOL = {
  tcp: "Modbus TCP",
  rtu: "Modbus RTU",
  get "tcp+rtu"() {
    return t("hdr.tcprtu");
  },
};

/** The x_status of a map, if it has one. */
export function mapStatusOf(meta: Meta): string | null {
  return meta.extra.find(([k]) => k === "x_status")?.[1] ?? null;
}

/** The general data of a map, as label/value pairs (empty ones left out). */
export function headerRows(meta: Meta, registers: number): [string, string][] {
  const m = meta;
  const serial = [
    m.baudrate ? t("hdr.baud", { n: m.baudrate }) : null,
    m.data_bits || m.parity || m.stop_bits
      ? `${m.data_bits ?? 8}${m.parity ?? "E"}${m.stop_bits ?? 1}`
      : null,
  ].filter(Boolean);
  const firmware = [
    m.firmware,
    m.firmware_min || m.firmware_max
      ? t("hdr.fwrange", {
          min: m.firmware_min ?? t("hdr.any"),
          max: m.firmware_max ?? t("hdr.latest"),
        })
      : null,
  ].filter(Boolean);
  const maxRead = [
    m.max_read_registers ? t("hdr.nregs", { n: m.max_read_registers }) : null,
    m.max_read_bits ? t("hdr.nbits", { n: m.max_read_bits }) : null,
  ].filter(Boolean);
  const rows: [string, string | null | undefined][] = [
    [t("hdr.models"), m.models?.length ? m.models.join(", ") : null],
    [t("hdr.description"), m.description],
    [
      t("hdr.revision"),
      m.revision ? `${m.revision}${m.revised ? `, ${m.revised}` : ""}` : null,
    ],
    [t("hdr.firmware"), firmware.join("; ") || null],
    [t("hdr.registers"), String(registers)],
    [t("hdr.protocol"), m.protocol ? PROTOCOL[m.protocol] : null],
    [t("hdr.unit"), m.unit_id?.toString()],
    [t("hdr.port"), m.port?.toString()],
    [t("hdr.serial"), serial.join(", ") || null],
    [t("hdr.byteorder"), m.byte_order],
    [
      t("hdr.addresses"),
      m.address_base === null ? null : t("hdr.based", { base: m.address_base }),
    ],
    [t("hdr.maxread"), maxRead.join(", ") || null],
    [t("hdr.source"), m.source],
    [t("hdr.website"), m.vendor_website],
  ];
  return rows.filter((r): r is [string, string] => !!r[1]);
}

/** Keys of the file this version does not know (`x_…`), but x_status, which
    is shown as an icon. */
export function extraRows(meta: Meta): [string, string][] {
  return meta.extra.filter(([k]) => k !== "x_status");
}
