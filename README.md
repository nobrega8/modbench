# Modbench

Cliente Modbus (TCP/RTU) leve e moderno para testar e ler devices, para Windows e Linux.

> 🚧 Em desenvolvimento inicial. Ver [PLAN.md](PLAN.md).

## Ideia

- **Ficheiros `.mbm` (ModBus Map)**: um ficheiro de texto, baseado em CSV, que descreve um device (marca, modelo, parâmetros de ligação) e todos os seus registos. Ao abrir um `.mbm`, a lista de variáveis fica logo pronta a ler.
- **Biblioteca de devices**: guarda uma coleção de mapas `.mbm` dentro da app.
- **Ultra-leve**: Tauri 2 + Rust no core e Svelte na UI.

## Stack

- [Tauri 2](https://tauri.app) · Rust (`tokio-modbus`, `tokio-serial`)
- Svelte 5 + TypeScript + Vite

## Licença

[MIT](LICENSE)

## Desenvolvimento

Requisitos: [Rust](https://rustup.rs) (stable), Node.js 22+ e os [pré-requisitos do Tauri](https://tauri.app/start/prerequisites/) para o teu SO.

```sh
npm install
npm run tauri dev     # app em modo desenvolvimento
npm run tauri build   # instaladores (MSI/NSIS no Windows, deb/AppImage no Linux)
```

Verificações (as mesmas da CI):

```sh
npm run format:check && npm run check
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```
