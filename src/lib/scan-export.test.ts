import { describe, expect, it } from "vitest";
import {
  addressScanCsv,
  networkScanCsv,
  serialScanCsv,
  unitScanCsv,
} from "./scan-export";
import type { NetHit } from "./types";

const NOW = new Date(2026, 9, 16, 9, 30, 5, 120).getTime();

describe("scan exports", () => {
  it("list every unit ID, and which were not asked after a stop", () => {
    const text = unitScanCsv(
      [
        {
          unit: 2,
          response: "responds with exception 0x02 (illegal data address)",
        },
      ],
      { from: 1, to: 4, timeout_ms: 300, connection: "192.168.1.10:502" },
      { stoppedAt: 3 },
      ",",
      NOW,
    );
    expect(text).toBe(
      [
        "scan,unit IDs",
        "date,2026-10-16 09:30:05.120",
        "connection,192.168.1.10:502",
        "unit IDs,1-4",
        "wait_ms,300",
        "request,read 1 holding register at address 0 (FC03)",
        "found,1",
        "",
        "unit_id,status,response",
        "1,no answer,",
        "2,answered,responds with exception 0x02 (illegal data address)",
        "3,no answer,",
        "4,not asked (scan stopped),",
        "",
      ].join("\n"),
    );
  });

  it("say so when nothing answered", () => {
    const serial = serialScanCsv(
      [],
      {
        port: "COM3",
        baudrates: [9600, 19200],
        parities: ["E", "N"],
        stop_bits: [1],
        data_bits: 8,
        from: 1,
        to: 1,
        area: "holding",
        address: 0,
        timeout_ms: 200,
        stop_at_first: true,
      },
      { stopped: false },
      ";",
      NOW,
    );
    expect(serial).toContain("baud rates;9600 19200");
    expect(serial).toContain("result;complete");
    expect(serial.trimEnd().split("\n").at(-1)).toBe(
      ";;;;;no setting answered",
    );

    const addresses = addressScanCsv(
      [],
      {
        area: "input",
        from: 0,
        to: 99,
        block: 50,
        timeout_ms: 300,
        connection: "COM3 · 9600 8E1",
        unit: 5,
      },
      { stoppedAt: 49 },
      ",",
      NOW,
    );
    expect(addresses).toContain("result,stopped at address 49");
    expect(addresses).toContain("addresses that answered,0");
  });

  it("quote fields that need it and count what did not connect", () => {
    const hit: NetHit = {
      ip: "10.0.0.5",
      port: 502,
      unit: 1,
      status: "tcp_only",
      response_ms: 150.04,
      response: 'timed out, "late"',
      identity: null,
    };
    const text = networkScanCsv(
      [hit],
      {
        targets: "10.0.0.0/30, 10.0.0.9",
        port: 502,
        unit: 1,
        timeout_ms: 150,
        identify: false,
        retries: 0,
      },
      { done: 3, total: 3 },
      ",",
      NOW,
    );
    expect(text).toContain('addresses,"10.0.0.0/30, 10.0.0.9"');
    expect(text).toContain("no connection,2");
    expect(text).toContain(
      '10.0.0.5,502,1,port open,150.0,"timed out, ""late""",',
    );
  });
});
