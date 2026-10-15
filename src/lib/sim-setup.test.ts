import { describe, expect, it } from "vitest";
import {
  parseSetup,
  setupText,
  type SetupDevice,
  type SetupListen,
} from "./sim-setup";
import type { DeviceMap } from "./types";

const listen: SetupListen = {
  kind: "tcp",
  port: 5020,
  public: false,
  serial_port: "",
  baudrate: 9600,
  parity: "N",
  data_bits: 8,
  stop_bits: 1,
};

const map = {
  meta: { vendor: "A", model: "B" },
  registers: [],
} as unknown as DeviceMap;

const devices: SetupDevice[] = [
  {
    unit: 1,
    strict: true,
    source: "C:/maps/a.mbmap",
    map,
    generators: [
      {
        register: "Voltage",
        kind: "sine",
        period_s: 10,
        amplitude: 5,
        offset: 230,
        every_ms: 500,
      },
    ],
  },
  { unit: null, strict: false, source: null, map: null, generators: [] },
];

/** A setup with one part changed. */
function edited(change: (s: Record<string, any>) => void): string {
  const s = JSON.parse(setupText(listen, devices));
  change(s);
  return JSON.stringify(s);
}

describe("simulator setups", () => {
  it("reads back what it saved", () => {
    const back = parseSetup(setupText(listen, devices));
    expect(back).toEqual({ modbench_sim_setup: 1, listen, devices });
  });

  it("refuses what is not a setup", () => {
    expect(parseSetup("not json")).toHaveProperty("error");
    expect(parseSetup("{}")).toHaveProperty("error");
    expect(parseSetup("[]")).toHaveProperty("error");
  });

  it("refuses a setup from a newer version", () => {
    const text = edited((s) => (s.modbench_sim_setup = 2));
    expect(parseSetup(text)).toEqual({
      error: "This setup was saved by a newer Modbench; update to open it.",
    });
  });

  it("refuses wrong listen settings", () => {
    expect(parseSetup(edited((s) => (s.listen.kind = "x")))).toHaveProperty(
      "error",
    );
    expect(parseSetup(edited((s) => (s.listen.port = 70000)))).toHaveProperty(
      "error",
    );
    expect(parseSetup(edited((s) => (s.listen.parity = "M")))).toHaveProperty(
      "error",
    );
  });

  it("refuses wrong devices", () => {
    expect(parseSetup(edited((s) => (s.devices = [])))).toEqual({
      error: "The setup has no devices.",
    });
    expect(parseSetup(edited((s) => (s.devices[1].unit = 300)))).toEqual({
      error: "Device 2 of the setup is not valid.",
    });
    expect(
      parseSetup(edited((s) => (s.devices[0].generators = "none"))),
    ).toEqual({ error: "Device 1 of the setup is not valid." });
  });

  it("refuses unit IDs the simulator would refuse", () => {
    expect(parseSetup(edited((s) => (s.devices[1].unit = 1)))).toEqual({
      error: "Two devices have unit ID 1.",
    });
    expect(parseSetup(edited((s) => (s.devices[0].unit = null)))).toEqual({
      error: "Only one device can answer every unit ID.",
    });
  });
});
