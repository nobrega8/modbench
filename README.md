# ComTest Lite (nome provisório)

Cliente Modbus (TCP/RTU) leve e moderno para testar e ler devices, para Windows e Linux.

> 🚧 Em fase de planeamento. Ver [PLAN.md](PLAN.md).

## Ideia

- **Ficheiros `.mbm` (ModBus Map)**: um ficheiro de texto, baseado em CSV, que descreve um device (marca, modelo, parâmetros de ligação) e todos os seus registos. Ao abrir um `.mbm`, a lista de variáveis fica logo pronta a ler.
- **Biblioteca de devices**: guarda uma coleção de mapas `.mbm` dentro da app.
- **Ultra-leve**: Tauri 2 + Rust no core e Svelte na UI.

## Stack

- [Tauri 2](https://tauri.app) · Rust (`tokio-modbus`, `tokio-serial`)
- Svelte 5 + TypeScript + Vite

## Licença

[MIT](LICENSE)
