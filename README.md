<p align="center"><img src="assets/icon.svg" width="112" alt=""></p>

# Modbench

[![CI](https://github.com/nobrega8/modbench/actions/workflows/ci.yml/badge.svg)](https://github.com/nobrega8/modbench/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nobrega8/modbench)](https://github.com/nobrega8/modbench/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nobrega8/modbench/total)](https://github.com/nobrega8/modbench/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-informational)](#download)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

A lightweight Modbus TCP/RTU client for Windows and Linux. Test any address by hand, like a classic Modbus tester, or open a device map and read every register by name, already scaled, with units and labels.

## Download

Get the latest installer from [Releases](https://github.com/nobrega8/modbench/releases/latest):

| System | File |
|---|---|
| Windows 10/11 | `Modbench_x.y.z_x64-setup.exe` (or the `.msi`) |
| Linux | `Modbench_x.y.z_amd64.AppImage` (or the `.deb`) |

Modbench checks for new versions when it starts and installs them when you choose to. On Linux, in-app updates work for the AppImage.

On Linux, add yourself to the `dialout` group to use serial ports: `sudo usermod -aG dialout $USER`, then log in again.

## Features

- **Three-step workflow**: connection (serial RTU, Modbus TCP or RTU over TCP), request (any function code, or a device map), test (Start/Stop, valid/error/timeout counters).
- **Device maps (`.mbmap`)**: plain-text, CSV-based files that describe a device and all of its registers: types, byte order, scaling, units, bit fields, translation tables for state codes and sentinel values such as "unimplemented". The format is open and specified in [mbmap-library](https://github.com/nobrega8/mbmap-library), which also hosts shared maps.
- **Write from the map**: writable registers and control commands are listed with the right control for each type. A register can declare the values it accepts (`write_min`, `write_max`) and Modbench refuses anything outside them.
- **Find unit IDs**: *Find…* next to Device asks each unit ID for one register and lists the ones that answer. Read-only.
- **Value templates**: save the values read from a device's writable registers to a `.mbtpl` text file, and write them to another device of the same model. Only values that differ are written by default, and the last apply can be undone.
- **Reference checks**: a map can name registers with a known value; Modbench shows whether they match, which helps spot a wrong address, byte order or type.
- **Remembers each device**: the registers you switch off, the byte order override and Loop come back for the same map on the same connection and unit ID (see below).
- **Raw data**: the exact bytes sent and received, plus a request log you can save.
- **Device library**: keep your maps in the app, import them from a file or browse and download the shared maps of [mbmap-library](https://github.com/nobrega8/mbmap-library); double-click a `.mbmap` to open it.
- **Command line and MCP**: `modbench-cli` reads devices without the UI (see below).
- **Light and dark (OLED) themes**, a Rust core and small installers (about 2 MB on Windows).

## What Modbench remembers

Everything is stored on this computer, in the app's webview storage, and nothing is sent anywhere.

| What | Remembered for | Notes |
|---|---|---|
| Connection fields (port, baud rate, address, unit ID, timing) | the last test | Settings, "Remember the last connection" |
| Registers switched off, byte order override, Loop | one **device**: the map file, the connection endpoint and the unit ID | Settings, "Remember register choices for each device" |
| Theme, confirmations, log size, update check | the app | Settings |

A device is identified by the map file, **how it is reached** (host and port for TCP, the serial port name for RTU) and its **unit ID**. Two devices of the same model on different unit IDs or addresses keep separate choices. Registers are remembered by what the map says about them (area, address, type, bit and name), not by their position, so editing a map does not switch the wrong registers off.

A device seen for the first time starts with the registers last used with that map (all of them if there are none) and with the map's own byte order. Restoring choices only changes what the screen shows: it never writes to a device or starts a test. "Forget saved device choices" in Settings removes all of them.

## Stack

- [Tauri 2](https://tauri.app) · Rust (`tokio-modbus`, `tokio-serial`)
- Svelte 5 + TypeScript + Vite

## Development

Requirements: [Rust](https://rustup.rs) (stable), Node.js 22+ and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```sh
npm install
npm run tauri dev     # run the app in development mode
npm run tauri build   # build installers (MSI/NSIS on Windows, deb/AppImage on Linux)
```

To try the app without hardware, run the device simulator. It serves any `.mbmap` file over Modbus TCP, with values that change over time:

```sh
cd src-tauri
cargo run --example simulator -- ../vendor/mbmap-library/examples/example-corp/em-100.mbmap 5020
```

Then connect Modbench to `127.0.0.1`, port `5020`. You can also pass a map to the app on start: `npm run tauri dev -- -- -- path/to/map.mbmap`.

Checks (the same ones CI runs):

```sh
npm run format:check && npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Command line and MCP

`modbench-cli` reads devices without the UI. It is read-only: it never writes to a device.

```sh
cd src-tauri
cargo run --bin modbench-cli -- ports
cargo run --bin modbench-cli -- read --tcp 192.168.1.10 holding 0x0405 1
cargo run --bin modbench-cli -- read --rtu COM3 --baud 9600 --stop-bits 2 --unit 18 holding 1029 4
cargo run --bin modbench-cli -- map --rtu COM3 --unit 18 path/to/device.mbmap --filter voltage
cargo run --bin modbench-cli -- scan --rtu COM3 --baud 9600 --from 1 --to 32 --timeout 200
cargo run --bin modbench-cli -- validate path/to/device.mbmap
```

Settings not given on the command line come from the map (`@baudrate`, `@parity`, `@unit_id`…), then from defaults. Add `--json` to `read` and `map` for machine-readable output.

`modbench-cli mcp` runs the same operations as an [MCP](https://modelcontextprotocol.io) server over stdio (`list_serial_ports`, `read_registers`, `read_map`, `validate_map`). `.mcp.json` registers it for this project, so MCP clients such as Claude Code can use it.

A serial port can only be open in one program at a time: stop reading in the app before using the CLI on the same port.

## Releases and updates

Modbench checks GitHub Releases for a newer version when it starts and offers to install it. Updates are signed; the app only installs packages signed with the project's updater key.

To publish a release:

1. Bump `version` in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.
2. Commit, then push a tag: `git tag v0.2.0 && git push origin v0.2.0`.
3. The `Release` workflow builds the Windows (MSI, NSIS) and Linux (AppImage, deb) installers, signs the update packages with the `TAURI_SIGNING_PRIVATE_KEY` secret and publishes them with `latest.json`.

On Linux, in-app updates work for the AppImage; `.deb` installs are updated through the package manager.

## License

[MIT](LICENSE)
