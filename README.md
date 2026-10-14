<p align="center"><img src="assets/icon.svg" width="112" alt=""></p>

# Modbench

[![CI](https://github.com/nobrega8/modbench/actions/workflows/ci.yml/badge.svg)](https://github.com/nobrega8/modbench/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nobrega8/modbench)](https://github.com/nobrega8/modbench/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nobrega8/modbench/total)](https://github.com/nobrega8/modbench/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-informational)](#download)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Supports .mbmap](https://img.shields.io/badge/supports-.mbmap-0A7EA4)](https://github.com/nobrega8/mbmap-library)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Ko-fi](https://img.shields.io/badge/Ko--fi-support-FF5E5B?logo=ko-fi&logoColor=white)](https://ko-fi.com/nobrega)

A lightweight Modbus TCP/RTU client for Windows and Linux. Test any address by hand, or open a device map and read every register by name, already scaled, with units and labels.

![Modbench reading an energy meter through its device map](assets/screenshots/device-map.png)

## Download

Get the installer from [Releases](https://github.com/nobrega8/modbench/releases/latest): `Modbench_x.y.z_x64-setup.exe` (or `.msi`) for Windows 10/11, `Modbench_x.y.z_amd64.AppImage` (or `.deb`) for Linux. Modbench offers new versions when it starts.

On Linux, add yourself to the `dialout` group to use serial ports: `sudo usermod -aG dialout $USER`, then log in again.

<details>
<summary>Install with APT (Debian, Ubuntu)</summary>

```sh
curl -fsSL https://nobrega8.github.io/modbench/key.asc | sudo gpg --dearmor -o /usr/share/keyrings/modbench.gpg
echo "deb [arch=amd64 signed-by=/usr/share/keyrings/modbench.gpg] https://nobrega8.github.io/modbench stable main" | sudo tee /etc/apt/sources.list.d/modbench.list
sudo apt update && sudo apt install modbench
```

Updates then arrive with `sudo apt upgrade`.
</details>

## Features

- **Manual requests**: any function code, address and quantity, over serial RTU, Modbus TCP or RTU over TCP.
- **Device maps**: open a [`.mbmap.json`](https://github.com/nobrega8/mbmap-library) and read every register by name, with types, byte and word order, scaling, units, bit fields and state labels. Browse and download over 400 shared maps from [mbmap-library](https://github.com/nobrega8/mbmap-library).
- **Checks the device**: reference values catch a wrong address or byte order, and a map can declare which models and firmware versions it fits.
- **Write safely**: typed controls for writable registers, limits from the map, optional confirmation, and a read-only mode that blocks every write.
- **Chart, raw bytes and log**: plot registers over time, see the exact frames, export values or record every cycle to CSV.
- **Find devices**: scan for unit IDs, or for the baud rate, parity and stop bits of a serial device.
- **Value templates**: copy the settings of one device to another of the same model.
- **Reconnects by itself** when a link drops during a test.
- Light and dark themes, adjustable text size, and small installers (about 2 MB on Windows).

| Chart | Device library | Dark theme |
|---|---|---|
| ![Chart of voltages and currents](assets/screenshots/chart.png) | ![Device library with vendor icons](assets/screenshots/library.png) | ![Device map in the dark theme](assets/screenshots/device-map-dark.png) |

<details>
<summary>What Modbench remembers</summary>

Everything stays on this computer. The last connection is filled in again on start. For each device (map + connection + unit ID), the registers you switched off, the byte order override and Loop come back; restoring them never writes or starts a test. Both can be turned off, and saved device choices forgotten, in Settings.
</details>

## Command line and MCP

`modbench-cli` reads devices without the UI. It never writes to a device.

```sh
modbench-cli read --tcp 192.168.1.10 holding 0x0405 1
modbench-cli map --rtu COM3 --unit 18 device.mbmap.json --filter voltage
modbench-cli scan --rtu COM3 --baud 9600 --from 1 --to 32
modbench-cli validate device.mbmap.json
modbench-cli convert old-device.mbmap          # version 1 map -> old-device.mbmap.json
```

Run `modbench-cli --help` for every command and option. `modbench-cli mcp` serves the same operations over [MCP](https://modelcontextprotocol.io) (stdio), so AI assistants such as Claude Code can read devices; `.mcp.json` registers it for this project.

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

[MIT](LICENSE)

## Funding

Modbench is free and open source. If it saves you time, you can support its development on Ko-fi:

<a href="https://ko-fi.com/nobrega"><img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="Support Modbench on Ko-fi" height="36"></a>
