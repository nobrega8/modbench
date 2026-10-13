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

Checks (the same ones CI runs):

```sh
npm run format:check && npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## License

[MIT](LICENSE)
