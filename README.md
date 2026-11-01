<p align="center"><img src="assets/icon.svg" width="112" alt=""></p>

# Modbench

**English** · [Español](docs/readme/README_ES.md) · [简体中文](docs/readme/README_ZHS.md) · [日本語](docs/readme/README_JP.md)

[![CI](https://github.com/nobrega8/modbench/actions/workflows/ci.yml/badge.svg)](https://github.com/nobrega8/modbench/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nobrega8/modbench)](https://github.com/nobrega8/modbench/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nobrega8/modbench/total)](https://github.com/nobrega8/modbench/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-informational)](#download)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Supports .mbmap](https://img.shields.io/badge/supports-.mbmap-0A7EA4?logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0iI2ZmZmZmZiIgZmlsbC1ydWxlPSJldmVub2RkIj4KICA8dGl0bGU+bWJtYXA8L3RpdGxlPgogIDxnIHRyYW5zZm9ybT0idHJhbnNsYXRlKDAgLTAuNTUpIj4KICAgIDxyZWN0IHg9IjEuNDUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjUuOTUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjEwLjQ1IiB5PSI1LjMiIHdpZHRoPSIyLjYiIGhlaWdodD0iMi42IiByeD0iMC41Ii8+CiAgICA8cGF0aCBkPSJNMS41NSAxOS4zVjEyLjQ1QTMuNDUgMy40NSAwIDAgMSA3LjI1IDkuODM1QTMuNDUgMy40NSAwIDAgMSAxMi45NSAxMi40NVYxOS4zYTAuNSAwLjUgMCAwIDEtMC41IDAuNUgxMS4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDguNDUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVINi41NWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDMuOTUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVIMi4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVaIi8+CiAgICA8cGF0aCBkPSJNMTUuMDUgNS44YTAuNSAwLjUgMCAwIDEgMC41LTAuNUgxNi45NWEwLjUgMC41IDAgMCAxIDAuNSAwLjVWOUgxOUEzLjQ1IDMuNDUgMCAwIDEgMjIuNDUgMTIuNDVWMTYuMzVBMy40NSAzLjQ1IDAgMCAxIDE5IDE5LjhIMTUuNTVhMC41IDAuNSAwIDAgMS0wLjUtMC41Wk0xNy40NSAxMS40VjE3LjRIMTlBMS4wNSAxLjA1IDAgMCAwIDIwLjA1IDE2LjM1VjEyLjQ1QTEuMDUgMS4wNSAwIDAgMCAxOSAxMS40WiIvPgogIDwvZz4KPC9zdmc+Cg==)](https://github.com/nobrega8/mbmap-library)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](LICENSE)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy_Me_a_Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/nobrega8)

A lightweight Modbus TCP/RTU/ASCII client for Windows, Linux and macOS ([website](https://nobrega8.github.io/modbench/)). Test any address by hand, or open a device map and read every register by name, already scaled, with units and labels.

![Modbench reading an energy meter through its device map](assets/screenshots/device-map.png)

## Download

Get the installer from [Releases](https://github.com/nobrega8/modbench/releases/latest): `Modbench_x.y.z_x64-setup.exe` (or `.msi`) for Windows 10/11, `Modbench_x.y.z_amd64.AppImage` (or `.deb`) for Linux on PCs and `Modbench_x.y.z_aarch64.AppImage` (or `_arm64.deb`) for the Raspberry Pi and other 64-bit ARM boards, `Modbench_x.y.z_armhf.deb` for 32-bit Raspberry Pi OS (Bookworm or later), `Modbench_x.y.z_universal.dmg` for macOS 10.15+ (Apple Silicon and Intel). Modbench offers new versions when it starts.

On macOS the app is not notarized by Apple, so the first time, right-click Modbench in Applications and choose **Open** (or allow it under System Settings › Privacy & Security). Serial adapters appear as `/dev/cu.*`.

On Linux, add yourself to the `dialout` group to use serial ports: `sudo usermod -aG dialout $USER`, then log in again.

<details>
<summary>Install with APT (Debian, Ubuntu, Raspberry Pi OS)</summary>

For `amd64` (PCs), `arm64` (Raspberry Pi 3, 4 and 5 with a 64-bit system, other 64-bit ARM boards) and `armhf` (32-bit Raspberry Pi OS): Debian 12 and 13, Ubuntu 22.04 and later, Raspberry Pi OS Bookworm or later. APT picks the package for the machine. Raspberry Pi OS Buster and older cannot run Modbench (no WebKitGTK 4.1).

```sh
curl -fsSL https://nobrega8.github.io/modbench/key.asc | sudo gpg --dearmor -o /usr/share/keyrings/modbench.gpg
echo "deb [signed-by=/usr/share/keyrings/modbench.gpg] https://nobrega8.github.io/modbench stable main" | sudo tee /etc/apt/sources.list.d/modbench.list
sudo apt update && sudo apt install modbench
```

Updates then arrive with `sudo apt upgrade`. A list written for an older release with `arch=amd64` keeps working on PCs; on a Raspberry Pi, replace it with the line above.
</details>

<details>
<summary>Install from the AUR (Arch Linux, Manjaro)</summary>

`modbench-bin` repackages the release's `.deb` for `x86_64`, `aarch64` and `armv7h`:

```sh
yay -S modbench-bin
```

Updates then arrive with your AUR helper, for example `yay -Syu`.
</details>

## Features

- **Manual requests**: any function code, address and quantity, over serial RTU or ASCII, Modbus TCP or UDP, or RTU over TCP or UDP.
- **Modbus/TCP Security**: Modbus TCP inside TLS (port 802), checking the device's certificate against a CA (or trusting any) and showing a client certificate to devices that ask for one.
- **Address notations**: show addresses 0-based, 1-based, in hex, in Modicon (`400101`) or IEC 61131-3 (`%MW100`) notation. Address fields take the last two as typed, and switch to their area.
- **Device maps**: open a [`.mbmap.json`](https://github.com/nobrega8/mbmap-library) and read every register by name, with types, byte and word order, scaling, units, bit fields and state labels. The FC column shows the function code each register is read with, and a right-click reads one with the other function (FC03 or FC04) when a device only answers one. Browse and download over 400 shared maps from [mbmap-library](https://github.com/nobrega8/mbmap-library).
- **Map builder**: in the Library tab, a window of its own to write a map from a manual: the device header, a table of registers (area, address as the manual numbers it, type, scale, unit, access, state labels, description) and the validator's messages on each row while you type. It saves a version 2 `.mbmap.json`. The same window edits an existing map: right-click a map in the Library, or a register on the Device map, and choose the edit entry. A changes list shows what differs from the original. **Save** writes a library map back to its own file (the Library then shows it as edited), puts a new map in the library under its vendor and model (asking before it replaces one) and writes a map opened from another folder back to that file; **Save as…** writes a file wherever you choose.
- **Checks the device**: reference values catch a wrong address or byte order, and a map can declare which models and firmware versions it fits.
- **Write safely**: typed controls for writable registers, limits from the map, optional confirmation, and a read-only mode that blocks every write.
- **Chart, raw bytes and log**: plot map registers, or addresses of a manual read, over time (the wheel zooms the time axis, dragging pans it, Shift + drag zooms into a range, and the markers can set the range) and save the charts as PNG or SVG, see the exact frames, export values or record every cycle to CSV.
- **Find devices**: scan for unit IDs, or for the baud rate, parity and stop bits of a serial device.
- **Tools**:
  - **Simulator**: Modbus devices inside Modbench, each optionally shaped after a map (its reference values and addresses), to try maps and other programs without hardware. Several devices answer their own unit IDs at one address, like devices behind a gateway or on one RS-485 line; other unit IDs stay silent. It listens on Modbus TCP or UDP, RTU over TCP or UDP, or a serial port (RTU or ASCII, with the baud rate, parity and stop bits you choose). Generators make values move on their own (increment, sine, random, or a copy of another address), in a map register's own units. Save setup… keeps the interface, devices, unit IDs, maps and generators in a `.mbsim` file to open again later, or with a double-click.
  - **Gateway**: pass requests from clients on one link to the device of step 1 on another (Modbus TCP or UDP, RTU over TCP or UDP, or a serial port on either side), like a serial-to-Ethernet gateway on the bench, with every request listed. A silent device comes back as exception 0x0B, a missing link as 0x0A; it can rewrite the unit ID and refuse writes.
  - **Bus monitor**: listen to an RS-485 line where a master already talks to its devices, without sending a byte, and see each request with its answer, decoded: values, exceptions, requests nobody answered and bad CRCs, with filters and a CSV export.
  - **Identify**: read the device's vendor, product code and revision (FC43).
  - **Find addresses**: discover which registers answer on a device without a manual.
  - **Data decoder** and **frame decoder**: hex as every type in every byte order, and frames field by field with CRC/LRC checks (also on the Raw data tab).
  - **Statistics**: failures by cause (timeouts, each exception, bad frames) and response times per function code, and a suggested timeout (4 × the slowest answer) that one click puts in step 1.
  - **Alarms**: low/high limits on map registers, shown in red on the Device map tab.
  - **Test sequence**: for commissioning, steps on the map's registers (write a value, wait, expect a value or a range within a time, read one to record it), saved as `.mbtest.json`, with a result per step and an HTML report.
  - **Trend player**: play back a CSV recording with a scrub bar and up to 60× speed, and save the charts as an image.
- **Bit view**: see the 16 bits of each register and switch a single bit of a holding register.
- **Export to other software**: write a map as a Home Assistant Modbus configuration, ESPHome sensors, a Telegraf input, a Node-RED flow or a CSV tag list, with the device's address from the connection (right-click a map in the Library).
- **Value templates**: copy the settings of one device to another of the same model.
- **Reconnects by itself** when a link drops during a test.
- Light and dark themes, adjustable text size, and small installers (about 2 MB on Windows).

| Chart | Device library | Dark theme |
|---|---|---|
| ![Chart of voltages and currents](assets/screenshots/chart.png) | ![Device library with vendor icons](assets/screenshots/library.png) | ![Device map in the dark theme](assets/screenshots/device-map-dark.png) |

<details>
<summary>What Modbench remembers</summary>

Everything stays on this computer. The last connection is filled in again on start. For each device (map + connection + unit ID), the registers you switched off, the function codes you forced, the byte order override and Loop come back; restoring them never writes or starts a test. Both can be turned off, and saved device choices forgotten, in Settings.
</details>

## User manual

Read the [user manual (PDF)](https://raw.githubusercontent.com/nobrega8/modbench/main/manual/modbench-manual.pdf). Its LaTeX source is in [`manual/`](manual/): build it with `latexmk` in that folder (see its README).

## Command line and MCP

`modbench-cli` reads devices without the UI. It never writes to a device, except the write steps of a test sequence run with `--allow-writes`.

```sh
modbench-cli read --tcp 192.168.1.10 holding 0x0405 1
modbench-cli map --rtu COM3 --unit 18 device.mbmap.json --filter voltage
modbench-cli scan --rtu COM3 --baud 9600 --from 1 --to 32
modbench-cli id --tcp 192.168.1.10               # FC43 device identification
modbench-cli read --udp 192.168.1.10 holding 0 10   # Modbus UDP
modbench-cli read --tcp 192.168.1.10 --tls --ca ca.pem --cert me.pem --key me.key holding 0 1   # Modbus/TCP Security, port 802
modbench-cli simulate device.mbmap.json --strict  # a simulated device on port 5020
modbench-cli simulate device.mbmap.json --generators moving.json
modbench-cli simulate device.mbmap.json --rtu-over-tcp   # RTU frames over TCP, like a gateway
modbench-cli simulate device.mbmap.json --rtu COM5 --baud 19200 --parity E
modbench-cli simulate device.mbmap.json --tls-cert server.pem --tls-key server.key --tls-client-ca ca.pem   # inside TLS
modbench-cli gateway --listen tcp:0.0.0.0:5502 --rtu COM3 --baud 9600   # Modbus TCP clients -> RTU device
modbench-cli monitor --rtu COM3 --baud 19200 --parity E   # listen to a line, never send
modbench-cli test --tcp 192.168.1.10 device.mbmap.json start.mbtest.json --allow-writes --report out.html
modbench-cli validate device.mbmap.json
modbench-cli convert old-device.mbmap          # version 1 map -> old-device.mbmap.json
modbench-cli export device.mbmap.json -f home-assistant --tcp 192.168.1.10   # or esphome, telegraf, node-red, csv
```

`--generators` takes a JSON list. Each entry names a map register (in engineering units) or a raw `area` and `address`, a `kind`, and how often it changes (`every_ms`, default 1000, at least 50):

```json
[
  {"register": "Voltage L1", "kind": "sine", "offset": 230, "amplitude": 5, "period_s": 10, "every_ms": 500},
  {"area": "input", "address": 0, "kind": "increment", "step": 1, "min": 0, "max": 100},
  {"area": "holding", "address": 1, "kind": "random", "min": 10, "max": 20},
  {"area": "discrete", "address": 0, "kind": "copy", "from": {"area": "coil", "address": 0}}
]
```

Run `modbench-cli --help` for every command and option. `modbench-cli mcp` serves the same operations over [MCP](https://modelcontextprotocol.io) (stdio), so AI assistants such as Claude Code can read devices; `.mcp.json` registers it for this project.

To let an assistant work in the running window instead (open a map, change the connection, start a loop, show a chart), turn on **Let an assistant control Modbench** in Settings > Connection. It is off by default and local only (loopback port plus a token file); the `gui_*` tools forward to the window, a bar shows what the assistant did and lets you pause or disconnect it, and it never writes to a device. See the manual, chapter *Command line*.

## Development

Requirements: [Rust](https://rustup.rs), Node.js 22+ and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri dev       # run the app
npm run tauri build     # build the installers
```

No hardware? `cargo run --example simulator -- <map> 5020` (in `src-tauri`) serves a map over Modbus TCP on port 5020 with changing values.

CI runs `npm run format:check && npm run check && npm test` and, in `src-tauri`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.

To release, bump the version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, then push a `v*` tag: the Release workflow builds, signs and publishes the installers and the update manifest.

## License

Modbench is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. A modified version you distribute, or a program built on Modbench's code, must be published under the same terms.

Releases up to 0.1.9 were published under the MIT licence, and those copies keep it. The device maps in [mbmap-library](https://github.com/nobrega8/mbmap-library) stay in the public domain (CC0).

## Funding

Modbench is free and open source. If it saves you time, you can support its development on Buy Me a Coffee:

<a href="https://buymeacoffee.com/nobrega8"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Support Modbench on Buy Me a Coffee" height="36"></a>
