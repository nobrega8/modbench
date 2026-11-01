<p align="center"><img src="../../assets/icon.svg" width="112" alt=""></p>

# Modbench

[English](../../README.md) · **Español** · [简体中文](README_ZHS.md) · [日本語](README_JP.md)

[![CI](https://github.com/nobrega8/modbench/actions/workflows/ci.yml/badge.svg)](https://github.com/nobrega8/modbench/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nobrega8/modbench)](https://github.com/nobrega8/modbench/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nobrega8/modbench/total)](https://github.com/nobrega8/modbench/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-informational)](#descarga)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Supports .mbmap](https://img.shields.io/badge/supports-.mbmap-0A7EA4?logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0iI2ZmZmZmZiIgZmlsbC1ydWxlPSJldmVub2RkIj4KICA8dGl0bGU+bWJtYXA8L3RpdGxlPgogIDxnIHRyYW5zZm9ybT0idHJhbnNsYXRlKDAgLTAuNTUpIj4KICAgIDxyZWN0IHg9IjEuNDUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjUuOTUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjEwLjQ1IiB5PSI1LjMiIHdpZHRoPSIyLjYiIGhlaWdodD0iMi42IiByeD0iMC41Ii8+CiAgICA8cGF0aCBkPSJNMS41NSAxOS4zVjEyLjQ1QTMuNDUgMy40NSAwIDAgMSA3LjI1IDkuODM1QTMuNDUgMy40NSAwIDAgMSAxMi45NSAxMi40NVYxOS4zYTAuNSAwLjUgMCAwIDEtMC41IDAuNUgxMS4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDguNDUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVINi41NWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDMuOTUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVIMi4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVaIi8+CiAgICA8cGF0aCBkPSJNMTUuMDUgNS44YTAuNSAwLjUgMCAwIDEgMC41LTAuNUgxNi45NWEwLjUgMC41IDAgMCAxIDAuNSAwLjVWOUgxOUEzLjQ1IDMuNDUgMCAwIDEgMjIuNDUgMTIuNDVWMTYuMzVBMy40NSAzLjQ1IDAgMCAxIDE5IDE5LjhIMTUuNTVhMC41IDAuNSAwIDAgMS0wLjUtMC41Wk0xNy40NSAxMS40VjE3LjRIMTlBMS4wNSAxLjA1IDAgMCAwIDIwLjA1IDE2LjM1VjEyLjQ1QTEuMDUgMS4wNSAwIDAgMCAxOSAxMS40WiIvPgogIDwvZz4KPC9zdmc+Cg==)](https://github.com/nobrega8/mbmap-library)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](../../LICENSE)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy_Me_a_Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/nobrega8)

Un cliente Modbus TCP/RTU/ASCII ligero para Windows, Linux y macOS ([sitio web](https://nobrega8.github.io/modbench/)). Prueba cualquier dirección a mano, o abre un mapa del dispositivo y lee cada registro por su nombre, ya escalado, con unidades y etiquetas.

> Esta es una traducción del [README en inglés](../../README.md), que es la referencia y puede estar más al día.

![Modbench leyendo un contador de energía con su mapa de dispositivo](../../assets/screenshots/device-map.png)

## Descarga

Descarga el instalador desde [Releases](https://github.com/nobrega8/modbench/releases/latest): `Modbench_x.y.z_x64-setup.exe` (o `.msi`) para Windows 10/11, `Modbench_x.y.z_amd64.AppImage` (o `.deb`) para Linux en PC y `Modbench_x.y.z_aarch64.AppImage` (o `_arm64.deb`) para la Raspberry Pi y otras placas ARM de 64 bits, `Modbench_x.y.z_armhf.deb` para Raspberry Pi OS de 32 bits (Bookworm o posterior), `Modbench_x.y.z_universal.dmg` para macOS 10.15+ (Apple Silicon e Intel). Modbench ofrece las versiones nuevas al arrancar.

En macOS la app no está notarizada por Apple, así que la primera vez haz clic derecho en Modbench, en Aplicaciones, y elige **Abrir** (o permítela en Ajustes del Sistema › Privacidad y seguridad). Los adaptadores serie aparecen como `/dev/cu.*`.

En Linux, añádete al grupo `dialout` para usar puertos serie: `sudo usermod -aG dialout $USER`, y vuelve a iniciar sesión.

<details>
<summary>Instalar con APT (Debian, Ubuntu, Raspberry Pi OS)</summary>

Para `amd64` (PC), `arm64` (Raspberry Pi 3, 4 y 5 con un sistema de 64 bits, otras placas ARM de 64 bits) y `armhf` (Raspberry Pi OS de 32 bits): Debian 12 y 13, Ubuntu 22.04 y posteriores, Raspberry Pi OS Bookworm o posterior. APT elige el paquete de la máquina. Raspberry Pi OS Buster y anteriores no pueden ejecutar Modbench (no tienen WebKitGTK 4.1).

```sh
curl -fsSL https://nobrega8.github.io/modbench/key.asc | sudo gpg --dearmor -o /usr/share/keyrings/modbench.gpg
echo "deb [signed-by=/usr/share/keyrings/modbench.gpg] https://nobrega8.github.io/modbench stable main" | sudo tee /etc/apt/sources.list.d/modbench.list
sudo apt update && sudo apt install modbench
```

Las actualizaciones llegan después con `sudo apt upgrade`. Una lista escrita para una versión anterior con `arch=amd64` sigue funcionando en PC; en una Raspberry Pi, sustitúyela por la línea de arriba.
</details>

## Funciones

- **Peticiones manuales**: cualquier código de función, dirección y cantidad, por serie RTU o ASCII, Modbus TCP o UDP, o RTU sobre TCP o UDP.
- **Notaciones de dirección**: muestra las direcciones desde 0, desde 1, en hexadecimal, en notación Modicon (`400101`) o IEC 61131-3 (`%MW100`). Los campos de dirección aceptan estas dos últimas tal como se escriben y cambian a su área.
- **Mapas de dispositivo**: abre un [`.mbmap.json`](https://github.com/nobrega8/mbmap-library) y lee cada registro por su nombre, con tipos, orden de bytes y de palabras, escalado, unidades, campos de bits y etiquetas de estado. La columna FC muestra el código de función con que se lee cada registro, y un clic derecho lo lee con la otra función (FC03 o FC04) cuando un dispositivo solo responde a una. Explora y descarga más de 400 mapas compartidos de [mbmap-library](https://github.com/nobrega8/mbmap-library).
- **Editor de mapas**: en la pestaña Biblioteca, una ventana propia para escribir un mapa a partir de un manual: la cabecera del dispositivo, una tabla de registros (área, dirección tal como la numera el manual, tipo, escala, unidad, acceso, etiquetas de estado, descripción) y los mensajes del validador en cada fila mientras escribes. Guarda un `.mbmap.json` de versión 2. La misma ventana edita un mapa existente: haz clic derecho en un mapa de la Biblioteca, o en un registro del Mapa del dispositivo, y elige la opción de editar. Una lista de cambios muestra lo que difiere del original. **Guardar** escribe un mapa de la biblioteca en su propio archivo (la Biblioteca lo marca entonces como editado), pone un mapa nuevo en la biblioteca bajo su fabricante y modelo (preguntando antes de sustituir uno) y escribe en su archivo un mapa abierto desde otra carpeta; **Guardar como…** escribe un archivo donde elijas. También importa una tabla de registros pegada desde una hoja de cálculo o un archivo CSV.
- **Comprueba el dispositivo**: los valores de referencia detectan una dirección o un orden de bytes equivocados, y un mapa puede declarar a qué modelos y versiones de firmware corresponde.
- **Escritura segura**: controles con tipo para los registros escribibles, límites del mapa, confirmación opcional y un modo de solo lectura que bloquea cualquier escritura.
- **Gráfico, bytes en bruto y registro**: dibuja en el tiempo registros del mapa o direcciones de una lectura manual (la rueda amplía el eje del tiempo, arrastrar lo desplaza, Mayús + arrastrar amplía un intervalo y los marcadores pueden fijarlo) y guarda los gráficos como PNG o SVG, mira las tramas exactas, exporta valores o graba cada ciclo en CSV.
- **Encuentra dispositivos**: busca los ID de unidad, o la velocidad, la paridad y los bits de parada de un dispositivo serie.
- **Herramientas**:
  - **Simulador**: dispositivos Modbus dentro de Modbench, cada uno opcionalmente con la forma de un mapa (sus valores de referencia y direcciones), para probar mapas y otros programas sin hardware. Varios dispositivos responden a sus propios ID de unidad en una misma dirección, como los dispositivos detrás de una pasarela o en una línea RS-485; los demás ID de unidad no responden. Escucha en Modbus TCP o UDP, RTU sobre TCP o UDP, o un puerto serie (RTU o ASCII, con la velocidad, la paridad y los bits de parada que elijas). Los generadores hacen que los valores cambien solos (incremento, seno, aleatorio o copia de otra dirección), en las unidades del registro del mapa. Guardar configuración… conserva la interfaz, los dispositivos, los ID de unidad, los mapas y los generadores en un archivo `.mbsim` para abrirlo más tarde, o con un doble clic.
  - **Pasarela**: pasa las peticiones de clientes en un enlace al dispositivo del paso 1 en otro (Modbus TCP o UDP, RTU sobre TCP o UDP, o un puerto serie a cualquiera de los lados), como una pasarela serie-Ethernet en el banco, con cada petición listada. Un dispositivo que no responde vuelve como la excepción 0x0B, un enlace que falta como 0x0A; puede reescribir el ID de unidad y rechazar escrituras.
  - **Identificar**: lee el fabricante, el código de producto y la revisión del dispositivo (FC43).
  - **Buscar direcciones**: descubre qué registros responden en un dispositivo sin manual.
  - **Decodificador de datos** y **decodificador de tramas**: hexadecimal como cada tipo en cada orden de bytes, y tramas campo a campo con comprobación de CRC/LRC (también en la pestaña Datos sin procesar).
  - **Estadísticas**: fallos por causa (tiempos agotados, cada excepción, tramas erróneas) y tiempos de respuesta por código de función, y un tiempo de espera sugerido (4 × la respuesta más lenta) que un clic pone en el paso 1.
  - **Alarmas**: límites bajo/alto en registros del mapa, mostrados en rojo en la pestaña Mapa del dispositivo.
  - **Reproductor de tendencias**: reproduce una grabación CSV con una barra de desplazamiento y hasta 60× de velocidad, y guarda los gráficos como imagen.
- **Vista de bits**: mira los 16 bits de cada registro y cambia un solo bit de un registro holding.
- **Exportar a otro software**: escribe un mapa como configuración Modbus de Home Assistant, sensores de ESPHome, una entrada de Telegraf, un flujo de Node-RED o una lista de tags en CSV, con la dirección del dispositivo tomada de la conexión (clic derecho en un mapa de la Biblioteca).
- **Plantillas de valores**: copia la configuración de un dispositivo a otro del mismo modelo.
- **Se reconecta solo** cuando un enlace cae durante una prueba.
- Temas claro y oscuro, tamaño de texto ajustable e instaladores pequeños (unos 2 MB en Windows).

| Gráfico | Biblioteca de dispositivos | Tema oscuro |
|---|---|---|
| ![Gráfico de tensiones y corrientes](../../assets/screenshots/chart.png) | ![Biblioteca de dispositivos con iconos de fabricantes](../../assets/screenshots/library.png) | ![Mapa del dispositivo en el tema oscuro](../../assets/screenshots/device-map-dark.png) |

<details>
<summary>Lo que Modbench recuerda</summary>

Todo se queda en este ordenador. La última conexión se rellena de nuevo al arrancar. Para cada dispositivo (mapa + conexión + ID de unidad) vuelven los registros que desactivaste, los códigos de función que forzaste, el orden de bytes elegido y el Bucle; restaurarlos nunca escribe ni inicia una prueba. Ambas cosas se pueden desactivar, y olvidar las elecciones guardadas, en Ajustes.
</details>

## Manual de usuario

Lee el [manual de usuario (PDF, en inglés)](https://raw.githubusercontent.com/nobrega8/modbench/main/manual/modbench-manual.pdf). Su fuente LaTeX está en [`manual/`](../../manual/): compílala con `latexmk` en esa carpeta (consulta su README).

## Línea de comandos y MCP

`modbench-cli` lee dispositivos sin la interfaz. Nunca escribe en un dispositivo.

```sh
modbench-cli read --tcp 192.168.1.10 holding 0x0405 1
modbench-cli map --rtu COM3 --unit 18 device.mbmap.json --filter voltage
modbench-cli scan --rtu COM3 --baud 9600 --from 1 --to 32
modbench-cli id --tcp 192.168.1.10               # identificación del dispositivo (FC43)
modbench-cli read --udp 192.168.1.10 holding 0 10   # Modbus UDP
modbench-cli simulate device.mbmap.json --strict  # un dispositivo simulado en el puerto 5020
modbench-cli simulate device.mbmap.json --generators moving.json
modbench-cli simulate device.mbmap.json --rtu-over-tcp   # tramas RTU sobre TCP, como una pasarela
modbench-cli simulate device.mbmap.json --rtu COM5 --baud 19200 --parity E
modbench-cli gateway --listen tcp:0.0.0.0:5502 --rtu COM3 --baud 9600   # clientes Modbus TCP -> dispositivo RTU
modbench-cli validate device.mbmap.json
modbench-cli convert old-device.mbmap          # mapa de versión 1 -> old-device.mbmap.json
modbench-cli export device.mbmap.json -f home-assistant --tcp 192.168.1.10   # o esphome, telegraf, node-red, csv
```

`--generators` recibe una lista JSON. Cada entrada nombra un registro del mapa (en unidades de ingeniería) o un `area` y `address` en bruto, un `kind` y cada cuánto cambia (`every_ms`, por defecto 1000, al menos 50):

```json
[
  {"register": "Voltage L1", "kind": "sine", "offset": 230, "amplitude": 5, "period_s": 10, "every_ms": 500},
  {"area": "input", "address": 0, "kind": "increment", "step": 1, "min": 0, "max": 100},
  {"area": "holding", "address": 1, "kind": "random", "min": 10, "max": 20},
  {"area": "discrete", "address": 0, "kind": "copy", "from": {"area": "coil", "address": 0}}
]
```

Ejecuta `modbench-cli --help` para ver todos los comandos y opciones. `modbench-cli mcp` ofrece las mismas operaciones por [MCP](https://modelcontextprotocol.io) (stdio), para que asistentes de IA como Claude Code puedan leer dispositivos; `.mcp.json` lo registra para este proyecto.

Para que un asistente trabaje en la ventana abierta (abrir un mapa, cambiar la conexión, iniciar un bucle, mostrar un gráfico), activa **Permitir que un asistente controle Modbench** en Ajustes > Conexión. Está desactivado por defecto y es solo local (un puerto de loopback más un archivo de token); las herramientas `gui_*` se reenvían a la ventana, una barra muestra lo que hizo el asistente y permite pausarlo o desconectarlo, y nunca escribe en un dispositivo. Consulta el manual, capítulo *Command line*.

## Desarrollo

Requisitos: [Rust](https://rustup.rs), Node.js 22+ y los [requisitos de Tauri](https://tauri.app/start/prerequisites/).

```sh
npm install
npm run tauri dev       # ejecutar la app
npm run tauri build     # compilar los instaladores
```

¿Sin hardware? `cargo run --example simulator -- <map> 5020` (en `src-tauri`) sirve un mapa por Modbus TCP en el puerto 5020 con valores que cambian.

La CI ejecuta `npm run format:check && npm run check && npm test` y, en `src-tauri`, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test`.

Para publicar una versión, sube la versión en `package.json`, `src-tauri/Cargo.toml` y `src-tauri/tauri.conf.json`, y envía una etiqueta `v*`: el workflow Release compila, firma y publica los instaladores y el manifiesto de actualizaciones.

## Licencia

Modbench es software libre: puedes redistribuirlo y/o modificarlo bajo los términos de la [Licencia Pública General de GNU](../../LICENSE) publicada por la Free Software Foundation, ya sea la versión 3 de la Licencia o (a tu elección) cualquier versión posterior. Una versión modificada que distribuyas, o un programa construido sobre el código de Modbench, debe publicarse bajo los mismos términos.

Las versiones hasta la 0.1.9 se publicaron bajo la licencia MIT, y esas copias la conservan. Los mapas de dispositivo de [mbmap-library](https://github.com/nobrega8/mbmap-library) siguen en el dominio público (CC0).

## Financiación

Modbench es libre y de código abierto. Si te ahorra tiempo, puedes apoyar su desarrollo en Buy Me a Coffee:

<a href="https://buymeacoffee.com/nobrega8"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Apoya Modbench en Buy Me a Coffee" height="36"></a>
