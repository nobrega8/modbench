import { describe, expect, it } from "vitest";
import { changes, summary } from "./release-notes";

const body = `## What's new
* Simulator: several devices, and serial / RTU over TCP by @nobrega8 in https://github.com/nobrega8/modbench/pull/61
* Modicon and IEC 61131-3 address notation by @nobrega8 in https://github.com/nobrega8/modbench/pull/63
## Dependencies
* Bump vite from 8.0.15 to 8.0.16 by @dependabot[bot] in https://github.com/nobrega8/modbench/pull/58

**Full Changelog**: https://github.com/nobrega8/modbench/compare/v0.1.9...v0.2.0

Download the installer for your system.`;

describe("release notes", () => {
  it("lists the pull requests without who merged them", () => {
    expect(changes(body)).toEqual([
      "Simulator: several devices, and serial / RTU over TCP",
      "Modicon and IEC 61131-3 address notation",
      "Bump vite from 8.0.15 to 8.0.16",
    ]);
  });

  it("joins them on one line, or keeps notes without a list", () => {
    expect(summary("* One\n* Two")).toBe("One · Two");
    expect(summary("  Download the installer.  ")).toBe(
      "Download the installer.",
    );
  });
});
