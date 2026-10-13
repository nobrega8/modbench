# Modbench

A lightweight, modern Modbus TCP/RTU client for testing and reading devices on Windows and Linux.

> 🚧 Early development.

## Features

- **`.mbmap` device maps**: plain-text, CSV-based files that describe a device (vendor, model, connection defaults) and all of its registers. Open a `.mbmap` and the device's variables are ready to read, already named, scaled and typed. The format is open and specified in [mbmap-library](https://github.com/nobrega8/mbmap-library), which also hosts community-shared maps.
- **Device library**: keep a collection of `.mbmap` maps inside the app.
- **Lightweight**: Tauri 2 with a Rust core and a Svelte UI.

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
