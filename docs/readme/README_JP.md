<p align="center"><img src="../../assets/icon.svg" width="112" alt=""></p>

# Modbench

[English](../../README.md) · [Español](README_ES.md) · [简体中文](README_ZHS.md) · **日本語**

[![CI](https://github.com/nobrega8/modbench/actions/workflows/ci.yml/badge.svg)](https://github.com/nobrega8/modbench/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/nobrega8/modbench)](https://github.com/nobrega8/modbench/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/nobrega8/modbench/total)](https://github.com/nobrega8/modbench/releases)
[![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-informational)](#ダウンロード)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Supports .mbmap](https://img.shields.io/badge/supports-.mbmap-0A7EA4?logo=data:image/svg%2bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCIgZmlsbD0iI2ZmZmZmZiIgZmlsbC1ydWxlPSJldmVub2RkIj4KICA8dGl0bGU+bWJtYXA8L3RpdGxlPgogIDxnIHRyYW5zZm9ybT0idHJhbnNsYXRlKDAgLTAuNTUpIj4KICAgIDxyZWN0IHg9IjEuNDUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjUuOTUiIHk9IjUuMyIgd2lkdGg9IjIuNiIgaGVpZ2h0PSIyLjYiIHJ4PSIwLjUiLz4KICAgIDxyZWN0IHg9IjEwLjQ1IiB5PSI1LjMiIHdpZHRoPSIyLjYiIGhlaWdodD0iMi42IiByeD0iMC41Ii8+CiAgICA8cGF0aCBkPSJNMS41NSAxOS4zVjEyLjQ1QTMuNDUgMy40NSAwIDAgMSA3LjI1IDkuODM1QTMuNDUgMy40NSAwIDAgMSAxMi45NSAxMi40NVYxOS4zYTAuNSAwLjUgMCAwIDEtMC41IDAuNUgxMS4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDguNDUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVINi41NWEwLjUgMC41IDAgMCAxLTAuNS0wLjVWMTIuNDVBMS4wNSAxLjA1IDAgMCAwIDMuOTUgMTIuNDVWMTkuM2EwLjUgMC41IDAgMCAxLTAuNSAwLjVIMi4wNWEwLjUgMC41IDAgMCAxLTAuNS0wLjVaIi8+CiAgICA8cGF0aCBkPSJNMTUuMDUgNS44YTAuNSAwLjUgMCAwIDEgMC41LTAuNUgxNi45NWEwLjUgMC41IDAgMCAxIDAuNSAwLjVWOUgxOUEzLjQ1IDMuNDUgMCAwIDEgMjIuNDUgMTIuNDVWMTYuMzVBMy40NSAzLjQ1IDAgMCAxIDE5IDE5LjhIMTUuNTVhMC41IDAuNSAwIDAgMS0wLjUtMC41Wk0xNy40NSAxMS40VjE3LjRIMTlBMS4wNSAxLjA1IDAgMCAwIDIwLjA1IDE2LjM1VjEyLjQ1QTEuMDUgMS4wNSAwIDAgMCAxOSAxMS40WiIvPgogIDwvZz4KPC9zdmc+Cg==)](https://github.com/nobrega8/mbmap-library)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue)](../../LICENSE)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy_Me_a_Coffee-support-FFDD00?logo=buymeacoffee&logoColor=black)](https://buymeacoffee.com/nobrega8)

Windows、Linux、macOS 向けの軽量な Modbus TCP/RTU/ASCII クライアントです（[ウェブサイト](https://nobrega8.github.io/modbench/)）。任意のアドレスを手動でテストしたり、デバイスマップを開いてすべてのレジスタを名前で、スケーリング済みの値と単位・ラベル付きで読み取ったりできます。

> これは[英語版 README](../../README.md) の翻訳です。英語版が正であり、より新しい場合があります。

![デバイスマップで電力量計を読み取る Modbench](../../assets/screenshots/device-map.png)

## ダウンロード

[Releases](https://github.com/nobrega8/modbench/releases/latest) からインストーラーを入手してください。Windows 10/11 は `Modbench_x.y.z_x64-setup.exe`（または `.msi`）、PC の Linux は `Modbench_x.y.z_amd64.AppImage`（または `.deb`）、Raspberry Pi などの 64 ビット ARM ボードは `Modbench_x.y.z_aarch64.AppImage`（または `_arm64.deb`）、32 ビット Raspberry Pi OS（Bookworm 以降）は `Modbench_x.y.z_armhf.deb`、macOS 10.15 以降（Apple シリコンと Intel）は `Modbench_x.y.z_universal.dmg` です。Modbench は起動時に新しいバージョンを知らせます。

macOS ではアプリが Apple の公証を受けていないため、初回は「アプリケーション」で Modbench を右クリックし、**開く**を選んでください（または「システム設定 › プライバシーとセキュリティ」で許可します）。シリアルアダプタは `/dev/cu.*` として表示されます。

Linux でシリアルポートを使うには、自分を `dialout` グループに追加して（`sudo usermod -aG dialout $USER`）、ログインし直してください。

<details>
<summary>APT でインストール（Debian、Ubuntu、Raspberry Pi OS）</summary>

`amd64`（PC）、`arm64`（64 ビットシステムの Raspberry Pi 3、4、5 とその他の 64 ビット ARM ボード）、`armhf`（32 ビット Raspberry Pi OS）に対応：Debian 12 と 13、Ubuntu 22.04 以降、Raspberry Pi OS Bookworm 以降。APT がマシンに合ったパッケージを選びます。Raspberry Pi OS Buster 以前では Modbench は動きません（WebKitGTK 4.1 がないため）。

```sh
curl -fsSL https://nobrega8.github.io/modbench/key.asc | sudo gpg --dearmor -o /usr/share/keyrings/modbench.gpg
echo "deb [signed-by=/usr/share/keyrings/modbench.gpg] https://nobrega8.github.io/modbench stable main" | sudo tee /etc/apt/sources.list.d/modbench.list
sudo apt update && sudo apt install modbench
```

以後の更新は `sudo apt upgrade` で届きます。以前のリリース向けに `arch=amd64` 付きで書いたリストは PC ではそのまま使えます。Raspberry Pi では上の行に置き換えてください。
</details>

## 機能

- **手動リクエスト**：任意のファンクションコード、アドレス、数量を、シリアル RTU または ASCII、Modbus TCP または UDP、RTU over TCP/UDP で送れます。
- **アドレス表記**：アドレスを 0 始まり、1 始まり、16 進数、Modicon（`400101`）、IEC 61131-3（`%MW100`）表記で表示します。アドレス欄には後の 2 つの表記をそのまま入力でき、その領域に切り替わります。
- **デバイスマップ**：[`.mbmap.json`](https://github.com/nobrega8/mbmap-library) を開いて、すべてのレジスタを名前で読み取ります。型、バイト順とワード順、スケーリング、単位、ビットフィールド、状態ラベル付きです。FC 列は各レジスタの読み取りに使うファンクションコードを示し、デバイスが一方にしか応答しない場合は右クリックでもう一方（FC03 または FC04）で読み取れます。[mbmap-library](https://github.com/nobrega8/mbmap-library) の 400 以上の共有マップを閲覧・ダウンロードできます。
- **マップビルダー**：「ライブラリ」タブから開く専用ウィンドウで、マニュアルを見ながらマップを作成できます。デバイス情報、レジスタ表（領域、マニュアルどおりのアドレス、型、スケール、単位、アクセス、状態ラベル、説明）があり、入力中は各行に検証メッセージが表示されます。保存形式はバージョン 2 の `.mbmap.json` です。同じウィンドウで既存のマップも編集できます。ライブラリのマップ、またはデバイスマップのレジスタを右クリックし、編集を選びます。変更一覧には元のマップとの差分が表示されます。**保存**は、ライブラリのマップを元のファイルに書き戻し（ライブラリでは編集済みと表示されます）、新しいマップをベンダーとモデルごとにライブラリへ追加し（既存のものを置き換える前に確認します）、別のフォルダから開いたマップはそのファイルに書き戻します。**名前を付けて保存…**は任意の場所に保存します。表計算ソフトから貼り付けたレジスタ表や CSV ファイルの取り込みもできます。
- **デバイスのチェック**：参照値でアドレスやバイト順の誤りを見つけられ、マップには対応するモデルとファームウェアのバージョンを記述できます。
- **安全な書き込み**：書き込み可能なレジスタ用の型付きコントロール、マップの制限値、任意の確認、すべての書き込みを止める読み取り専用モード。
- **グラフ、生バイト、ログ**：マップのレジスタや手動読み取りのアドレスを時系列で描画し（ホイールで時間軸を拡大、ドラッグで移動、Shift + ドラッグで範囲を拡大、マーカーでも範囲を設定可能）、グラフを PNG または SVG で保存し、フレームそのものを確認し、値をエクスポートしたり各サイクルを CSV に記録したりできます。
- **デバイスの検出**：ユニット ID、またはシリアルデバイスのボーレート、パリティ、ストップビットをスキャンします。
- **ツール**：
  - **シミュレータ**：Modbench 内で動く Modbus デバイスです。マップをもとに（参照値とアドレスを）作ることもでき、ハードウェアなしでマップや他のプログラムを試せます。複数のデバイスが 1 つのアドレスでそれぞれのユニット ID に応答し、ゲートウェイの先や同じ RS-485 ライン上のデバイスのように振る舞います。それ以外のユニット ID は応答しません。Modbus TCP または UDP、RTU over TCP/UDP、シリアルポート（RTU または ASCII、ボーレート・パリティ・ストップビットは任意）で待ち受けます。ジェネレーターで値を自動的に変化させられます（インクリメント、正弦波、ランダム、別アドレスのコピー）。値はマップのレジスタの単位で指定します。「セットアップを保存…」で、インターフェース、デバイス、ユニット ID、マップ、ジェネレーターを `.mbsim` ファイルに保存し、後で開いたりダブルクリックで開いたりできます。
  - **ゲートウェイ**：ある回線のクライアントからの要求を、別の回線にある手順 1 のデバイスへ中継します（どちら側も Modbus TCP または UDP、RTU over TCP/UDP、シリアルポートが使えます）。作業台のシリアル–イーサネット変換器のように動作し、すべての要求を一覧表示します。応答しないデバイスは例外 0x0B、回線がない場合は 0x0A として返ります。ユニット ID の書き換えや書き込みの拒否もできます。
  - **識別**：デバイスのベンダー、製品コード、リビジョンを読み取ります（FC43）。
  - **アドレス検索**：マニュアルのないデバイスで応答するレジスタを見つけます。
  - **データデコーダ**と**フレームデコーダ**：16 進数をすべての型とバイト順で表示し、フレームを CRC/LRC チェック付きでフィールドごとに解析します（「生データ」タブでも使えます）。
  - **統計**：原因別の失敗（タイムアウト、各例外、不正フレーム）とファンクションコード別の応答時間を示し、推奨タイムアウト（最も遅い応答の 4 倍）をワンクリックで手順 1 に設定できます。
  - **アラーム**：マップのレジスタの下限・上限。「デバイスマップ」タブで赤く表示されます。
  - **トレンドプレーヤー**：CSV の記録をシークバー付き、最大 60 倍速で再生し、グラフを画像として保存します。
- **ビット表示**：各レジスタの 16 ビットを表示し、保持レジスタの 1 ビットだけを切り替えられます。
- **他のソフトウェアへのエクスポート**：マップを Home Assistant の Modbus 設定、ESPHome のセンサー、Telegraf の入力、Node-RED のフロー、CSV のタグ一覧として書き出します。デバイスのアドレスは接続設定から取ります（ライブラリでマップを右クリック）。
- **値のテンプレート**：あるデバイスの設定を同じモデルの別のデバイスにコピーします。
- テスト中に回線が切れても**自動で再接続**します。
- ライトテーマとダークテーマ、文字サイズの調整、小さなインストーラー（Windows で約 2 MB）。

| グラフ | デバイスライブラリ | ダークテーマ |
|---|---|---|
| ![電圧と電流のグラフ](../../assets/screenshots/chart.png) | ![ベンダーアイコン付きのデバイスライブラリ](../../assets/screenshots/library.png) | ![ダークテーマのデバイスマップ](../../assets/screenshots/device-map-dark.png) |

<details>
<summary>Modbench が覚えていること</summary>

すべてこのコンピューター内に保存されます。起動時には前回の接続が再び入力されます。デバイス（マップ + 接続 + ユニット ID）ごとに、オフにしたレジスタ、強制したファンクションコード、バイト順の設定、ループが復元されます。復元によって書き込みやテストの開始が行われることはありません。どちらも「設定」でオフにでき、保存したデバイスの選択を消去することもできます。
</details>

## ユーザーマニュアル

[ユーザーマニュアル（PDF、英語）](https://raw.githubusercontent.com/nobrega8/modbench/main/manual/modbench-manual.pdf)をお読みください。LaTeX のソースは [`manual/`](../../manual/) にあり、そのフォルダで `latexmk` を使ってビルドできます（README を参照）。

## コマンドラインと MCP

`modbench-cli` は UI なしでデバイスを読み取ります。デバイスに書き込むことはありません。

```sh
modbench-cli read --tcp 192.168.1.10 holding 0x0405 1
modbench-cli map --rtu COM3 --unit 18 device.mbmap.json --filter voltage
modbench-cli scan --rtu COM3 --baud 9600 --from 1 --to 32
modbench-cli id --tcp 192.168.1.10               # FC43 デバイス識別
modbench-cli read --udp 192.168.1.10 holding 0 10   # Modbus UDP
modbench-cli simulate device.mbmap.json --strict  # ポート 5020 のシミュレートデバイス
modbench-cli simulate device.mbmap.json --generators moving.json
modbench-cli simulate device.mbmap.json --rtu-over-tcp   # TCP 上の RTU フレーム（ゲートウェイと同様）
modbench-cli simulate device.mbmap.json --rtu COM5 --baud 19200 --parity E
modbench-cli gateway --listen tcp:0.0.0.0:5502 --rtu COM3 --baud 9600   # Modbus TCP クライアント -> RTU デバイス
modbench-cli validate device.mbmap.json
modbench-cli convert old-device.mbmap          # バージョン 1 のマップ -> old-device.mbmap.json
modbench-cli export device.mbmap.json -f home-assistant --tcp 192.168.1.10   # または esphome、telegraf、node-red、csv
```

`--generators` には JSON のリストを渡します。各項目には、マップのレジスタ（工学単位）または生の `area` と `address`、`kind`、変化の間隔（`every_ms`、既定値 1000、最小 50）を指定します。

```json
[
  {"register": "Voltage L1", "kind": "sine", "offset": 230, "amplitude": 5, "period_s": 10, "every_ms": 500},
  {"area": "input", "address": 0, "kind": "increment", "step": 1, "min": 0, "max": 100},
  {"area": "holding", "address": 1, "kind": "random", "min": 10, "max": 20},
  {"area": "discrete", "address": 0, "kind": "copy", "from": {"area": "coil", "address": 0}}
]
```

すべてのコマンドとオプションは `modbench-cli --help` で確認できます。`modbench-cli mcp` は同じ操作を [MCP](https://modelcontextprotocol.io)（stdio）で提供し、Claude Code などの AI アシスタントがデバイスを読み取れるようにします。`.mcp.json` でこのプロジェクトに登録されています。

実行中のウィンドウでアシスタントに作業させる場合（マップを開く、接続を変更する、ループを開始する、グラフを表示する）は、「設定 > 接続」で**アシスタントによる Modbench の操作を許可**をオンにします。既定ではオフで、ローカルのみです（ループバックポートとトークンファイル）。`gui_*` ツールはウィンドウに転送され、バーにアシスタントの操作が表示されて一時停止や切断ができます。デバイスに書き込むことはありません。マニュアルの *Command line* の章を参照してください。

## 開発

必要なもの：[Rust](https://rustup.rs)、Node.js 22 以降、[Tauri の前提条件](https://tauri.app/start/prerequisites/)。

```sh
npm install
npm run tauri dev       # アプリを実行
npm run tauri build     # インストーラーをビルド
```

ハードウェアがない場合は、`src-tauri` で `cargo run --example simulator -- <map> 5020` を実行すると、変化する値を持つマップをポート 5020 の Modbus TCP で提供します。

CI は `npm run format:check && npm run check && npm test` と、`src-tauri` で `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` を実行します。

リリースするには、`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` のバージョンを上げてから `v*` タグをプッシュします。Release ワークフローがインストーラーと更新マニフェストをビルド、署名、公開します。

## ライセンス

Modbench はフリーソフトウェアです。Free Software Foundation が公開する [GNU 一般公衆利用許諾書](../../LICENSE)のバージョン 3、または（任意で）それ以降のバージョンの条件の下で、再配布や改変ができます。配布する改変版、または Modbench のコードを基にしたプログラムは、同じ条件で公開しなければなりません。

0.1.9 までのリリースは MIT ライセンスで公開されており、それらのコピーはそのライセンスのままです。[mbmap-library](https://github.com/nobrega8/mbmap-library) のデバイスマップはパブリックドメイン（CC0）です。

## 支援

Modbench は無料のオープンソースソフトウェアです。役に立った場合は、Buy Me a Coffee で開発を支援できます。

<a href="https://buymeacoffee.com/nobrega8"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me a Coffee で Modbench を支援" height="36"></a>
