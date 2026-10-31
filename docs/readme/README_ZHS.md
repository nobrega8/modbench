<p align="center"><img src="../../assets/icon.svg" width="112" alt=""></p>

# Modbench

[English](../../README.md) · [Español](README_ES.md) · **简体中文** · [日本語](README_JP.md)

[![CI](https://github.com/nobrega8/modbench/actions/workflows/ci.yml/badge.svg)](https://github.com/nobrega8/modbench/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nobrega8/modbench)](https://github.com/nobrega8/modbench/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nobrega8/modbench/total)](https://github.com/nobrega8/modbench/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-informational)](#下载)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Supports .mbmap](https://img.shields.io/badge/supports-.mbmap-0A7EA4?logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0iI2ZmZmZmZiIgZmlsbC1ydWxlPSJldmVub2RkIj4KICA8dGl0bGU+bWJtYXA8L3RpdGxlPgogIDxnIHRyYW5zZm9ybT0idHJhbnNsYXRlKDAgLTAuNTUpIj4KICAgIDxyZWN0IHg9IjEuNDUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjUuOTUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjEwLjQ1IiB5PSI1LjMiIHdpZHRoPSIyLjYiIGhlaWdodD0iMi42IiByeD0iMC41Ii8+CiAgICA8cGF0aCBkPSJNMS41NSAxOS4zVjEyLjQ1QTMuNDUgMy40NSAwIDAgMSA3LjI1IDkuODM1QTMuNDUgMy40NSAwIDAgMSAxMi45NSAxMi40NVYxOS4zYTAuNSAwLjUgMCAwIDEtMC41IDAuNUgxMS4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDguNDUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVINi41NWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDMuOTUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVIMi4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVaIi8+CiAgICA8cGF0aCBkPSJNMTUuMDUgNS44YTAuNSAwLjUgMCAwIDEgMC41LTAuNUgxNi45NWEwLjUgMC41IDAgMCAxIDAuNSAwLjVWOUgxOUEzLjQ1IDMuNDUgMCAwIDEgMjIuNDUgMTIuNDVWMTYuMzVBMy40NSAzLjQ1IDAgMCAxIDE5IDE5LjhIMTUuNTVhMC41IDAuNSAwIDAgMS0wLjUtMC41Wk0xNy40NSAxMS40VjE3LjRIMTlBMS4wNSAxLjA1IDAgMCAwIDIwLjA1IDE2LjM1VjEyLjQ1QTEuMDUgMS4wNSAwIDAgMCAxOSAxMS40WiIvPgogIDwvZz4KPC9zdmc+Cg==)](https://github.com/nobrega8/mbmap-library)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](../../LICENSE)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy_Me_a_Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/nobrega8)

一款适用于 Windows、Linux 和 macOS 的轻量级 Modbus TCP/RTU/ASCII 客户端（[网站](https://nobrega8.github.io/modbench/)）。可以手动测试任意地址，也可以打开设备映射，按名称读取每个寄存器，数值已换算，并带有单位和标签。

> 本文是[英文 README](../../README.md) 的译文；英文版为准，内容可能更新。

![Modbench 通过设备映射读取电能表](../../assets/screenshots/device-map.png)

## 下载

从 [Releases](https://github.com/nobrega8/modbench/releases/latest) 下载安装程序：Windows 10/11 使用 `Modbench_x.y.z_x64-setup.exe`（或 `.msi`），PC 上的 Linux 使用 `Modbench_x.y.z_amd64.AppImage`（或 `.deb`），树莓派及其他 64 位 ARM 板使用 `Modbench_x.y.z_aarch64.AppImage`（或 `_arm64.deb`），32 位 Raspberry Pi OS（Bookworm 或更新版本）使用 `Modbench_x.y.z_armhf.deb`，macOS 10.15+（Apple 芯片和 Intel）使用 `Modbench_x.y.z_universal.dmg`。Modbench 启动时会提示新版本。

在 macOS 上，应用未经 Apple 公证，因此第一次打开时，请在“应用程序”中右键点击 Modbench 并选择**打开**（或在“系统设置 › 隐私与安全性”中允许）。串口适配器显示为 `/dev/cu.*`。

在 Linux 上，要使用串口，请把自己加入 `dialout` 组：`sudo usermod -aG dialout $USER`，然后重新登录。

<details>
<summary>用 APT 安装（Debian、Ubuntu、Raspberry Pi OS）</summary>

支持 `amd64`（PC）、`arm64`（运行 64 位系统的树莓派 3、4、5 及其他 64 位 ARM 板）和 `armhf`（32 位 Raspberry Pi OS）：Debian 12 和 13、Ubuntu 22.04 及更新版本、Raspberry Pi OS Bookworm 或更新版本。APT 会为机器选择合适的软件包。Raspberry Pi OS Buster 及更早版本无法运行 Modbench（缺少 WebKitGTK 4.1）。

```sh
curl -fsSL https://nobrega8.github.io/modbench/key.asc | sudo gpg --dearmor -o /usr/share/keyrings/modbench.gpg
echo "deb [signed-by=/usr/share/keyrings/modbench.gpg] https://nobrega8.github.io/modbench stable main" | sudo tee /etc/apt/sources.list.d/modbench.list
sudo apt update && sudo apt install modbench
```

之后通过 `sudo apt upgrade` 获取更新。为旧版本编写、带有 `arch=amd64` 的源列表在 PC 上仍然可用；在树莓派上，请换成上面那一行。
</details>

## 功能

- **手动请求**：任意功能码、地址和数量，通过串口 RTU 或 ASCII、Modbus TCP 或 UDP，或 RTU over TCP/UDP。
- **地址表示法**：以 0 起始、1 起始、十六进制、Modicon（`400101`）或 IEC 61131-3（`%MW100`）格式显示地址。地址输入框可直接输入后两种格式，并会切换到对应的区域。
- **设备映射**：打开 [`.mbmap.json`](https://github.com/nobrega8/mbmap-library)，按名称读取每个寄存器，包括类型、字节序和字序、缩放、单位、位字段和状态标签。FC 列显示读取每个寄存器所用的功能码；当设备只响应其中一种时，右键可以改用另一种功能（FC03 或 FC04）读取。可从 [mbmap-library](https://github.com/nobrega8/mbmap-library) 浏览并下载 400 多个共享映射。
- **映射编辑器**：在“库”标签中，有一个独立窗口，可根据手册编写映射：设备信息、寄存器表（区域、按手册编号的地址、类型、缩放、单位、访问权限、状态标签、说明），输入时每行都会显示校验器的提示。它保存为版本 2 的 `.mbmap.json`。同一个窗口也可以编辑已有映射：在“库”中右键一个映射，或在“设备映射”中右键一个寄存器，选择编辑。变更列表显示与原文件的差异。**保存**会把库中的映射写回它自己的文件（库随后会标记为已编辑），把新映射按厂商和型号放入库中（替换已有映射前会先询问），并把从其他文件夹打开的映射写回原文件；**另存为…**可保存到任意位置。也可以导入从电子表格粘贴的寄存器表或 CSV 文件。
- **检查设备**：参考值可以发现地址或字节序错误，映射还可以声明适用的型号和固件版本。
- **安全写入**：可写寄存器有按类型的控件、来自映射的限值、可选的确认，以及阻止所有写入的只读模式。
- **图表、原始字节和日志**：随时间绘制映射寄存器或手动读取的地址（滚轮缩放时间轴，拖动平移，Shift + 拖动放大某个区间，标记也可设定区间），把图表保存为 PNG 或 SVG，查看完整的报文，导出数值或把每个周期记录到 CSV。
- **查找设备**：扫描从站地址，或串口设备的波特率、校验和停止位。
- **工具**：
  - **模拟器**：在 Modbench 内运行的 Modbus 设备，每个设备可按某个映射构建（其参考值和地址），无需硬件即可测试映射和其他程序。多个设备在同一地址上响应各自的从站地址，就像网关后面或同一条 RS-485 线上的设备；其他从站地址不响应。它可监听 Modbus TCP 或 UDP、RTU over TCP/UDP，或串口（RTU 或 ASCII，波特率、校验和停止位可选）。生成器让数值自动变化（递增、正弦、随机或复制另一个地址），以映射寄存器自身的单位表示。“保存设置…”把接口、设备、从站地址、映射和生成器保存在 `.mbsim` 文件中，之后可再次打开或双击打开。
  - **网关**：把一条链路上客户端的请求转发给另一条链路上第 1 步的设备（两侧都可以是 Modbus TCP 或 UDP、RTU over TCP/UDP 或串口），就像实验台上的串口转以太网网关，并列出每个请求。设备无应答时返回异常 0x0B，链路不可用时返回 0x0A；可以改写从站地址并拒绝写入。
  - **识别**：读取设备的厂商、产品代码和版本（FC43）。
  - **查找地址**：在没有手册的设备上找出哪些寄存器有响应。
  - **数据解码器**和**帧解码器**：把十六进制按每种类型和每种字节序显示，并逐字段解析报文、校验 CRC/LRC（“原始数据”标签中也有）。
  - **统计**：按原因统计失败（超时、各种异常、错误报文），按功能码统计响应时间，并给出建议的超时时间（最慢响应的 4 倍），一键填入第 1 步。
  - **报警**：映射寄存器的上下限，在“设备映射”标签中以红色显示。
  - **趋势播放器**：回放 CSV 记录，带进度条，最高 60 倍速，并可把图表保存为图片。
- **位视图**：查看每个寄存器的 16 个位，并切换保持寄存器的单个位。
- **导出到其他软件**：把映射写成 Home Assistant 的 Modbus 配置、ESPHome 传感器、Telegraf 输入、Node-RED 流程或 CSV 标签列表，设备地址取自当前连接（在“库”中右键一个映射）。
- **数值模板**：把一台设备的设置复制到同型号的另一台设备。
- 测试中链路断开时**自动重连**。
- 浅色和深色主题，可调文字大小，安装程序小巧（Windows 上约 2 MB）。

| 图表 | 设备库 | 深色主题 |
|---|---|---|
| ![电压和电流图表](../../assets/screenshots/chart.png) | ![带厂商图标的设备库](../../assets/screenshots/library.png) | ![深色主题下的设备映射](../../assets/screenshots/device-map-dark.png) |

<details>
<summary>Modbench 会记住什么</summary>

所有数据都保存在本机。启动时会重新填入上一次的连接。对每台设备（映射 + 连接 + 从站地址），会恢复你关闭的寄存器、强制使用的功能码、字节序设置和循环；恢复时不会写入，也不会开始测试。这两项都可以在“设置”中关闭，也可以清除已保存的设备选择。
</details>

## 用户手册

阅读[用户手册（PDF，英文）](https://raw.githubusercontent.com/nobrega8/modbench/main/manual/modbench-manual.pdf)。其 LaTeX 源文件位于 [`manual/`](../../manual/)：在该文件夹中用 `latexmk` 编译（见其 README）。

## 命令行和 MCP

`modbench-cli` 无需界面即可读取设备。它从不向设备写入。

```sh
modbench-cli read --tcp 192.168.1.10 holding 0x0405 1
modbench-cli map --rtu COM3 --unit 18 device.mbmap.json --filter voltage
modbench-cli scan --rtu COM3 --baud 9600 --from 1 --to 32
modbench-cli id --tcp 192.168.1.10               # FC43 设备识别
modbench-cli read --udp 192.168.1.10 holding 0 10   # Modbus UDP
modbench-cli simulate device.mbmap.json --strict  # 端口 5020 上的模拟设备
modbench-cli simulate device.mbmap.json --generators moving.json
modbench-cli simulate device.mbmap.json --rtu-over-tcp   # 通过 TCP 传输 RTU 报文，类似网关
modbench-cli simulate device.mbmap.json --rtu COM5 --baud 19200 --parity E
modbench-cli gateway --listen tcp:0.0.0.0:5502 --rtu COM3 --baud 9600   # Modbus TCP 客户端 -> RTU 设备
modbench-cli validate device.mbmap.json
modbench-cli convert old-device.mbmap          # 版本 1 映射 -> old-device.mbmap.json
modbench-cli export device.mbmap.json -f home-assistant --tcp 192.168.1.10   # 或 esphome、telegraf、node-red、csv
```

`--generators` 接受一个 JSON 列表。每一项指定一个映射寄存器（工程单位）或原始的 `area` 和 `address`，一个 `kind`，以及变化频率（`every_ms`，默认 1000，至少 50）：

```json
[
  {"register": "Voltage L1", "kind": "sine", "offset": 230, "amplitude": 5, "period_s": 10, "every_ms": 500},
  {"area": "input", "address": 0, "kind": "increment", "step": 1, "min": 0, "max": 100},
  {"area": "holding", "address": 1, "kind": "random", "min": 10, "max": 20},
  {"area": "discrete", "address": 0, "kind": "copy", "from": {"area": "coil", "address": 0}}
]
```

运行 `modbench-cli --help` 查看所有命令和选项。`modbench-cli mcp` 通过 [MCP](https://modelcontextprotocol.io)（stdio）提供相同的操作，让 Claude Code 等 AI 助手可以读取设备；`.mcp.json` 为本项目注册了它。

如果想让助手在正在运行的窗口中操作（打开映射、修改连接、开始循环、显示图表），请在“设置 > 连接”中打开**允许助手控制 Modbench**。该功能默认关闭，且仅限本机（回环端口加令牌文件）；`gui_*` 工具会转发到窗口，一个横条会显示助手做了什么，并可暂停或断开它；它从不向设备写入。参见手册的 *Command line* 一章。

## 开发

需要：[Rust](https://rustup.rs)、Node.js 22+ 以及 [Tauri 的前置条件](https://tauri.app/start/prerequisites/)。

```sh
npm install
npm run tauri dev       # 运行应用
npm run tauri build     # 构建安装程序
```

没有硬件？在 `src-tauri` 中运行 `cargo run --example simulator -- <map> 5020`，即可在端口 5020 上以 Modbus TCP 提供一个数值不断变化的映射。

CI 运行 `npm run format:check && npm run check && npm test`，并在 `src-tauri` 中运行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 和 `cargo test`。

发布新版本时，更新 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json` 中的版本号，然后推送一个 `v*` 标签：Release 工作流会构建、签名并发布安装程序和更新清单。

## 许可证

Modbench 是自由软件：你可以根据自由软件基金会发布的 [GNU 通用公共许可证](../../LICENSE)第 3 版或（由你选择）任何更新版本的条款重新分发和/或修改它。你分发的修改版本，或基于 Modbench 代码构建的程序，必须以相同条款发布。

0.1.9 及之前的版本以 MIT 许可证发布，这些副本仍保留该许可证。[mbmap-library](https://github.com/nobrega8/mbmap-library) 中的设备映射属于公有领域（CC0）。

## 赞助

Modbench 是免费的开源软件。如果它为你节省了时间，可以在 Buy Me a Coffee 上支持它的开发：

<a href="https://buymeacoffee.com/nobrega8"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="在 Buy Me a Coffee 上支持 Modbench" height="36"></a>
