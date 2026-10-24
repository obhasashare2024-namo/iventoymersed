# iventoymersed (Dual-Engine Unified Netboot Server)

**Merged & Immersed: 雙軌融合 iVentoy/iPXE + NetLoop GRUB 全格式網絡啟動一體機**  
*Pure Rust (Tokio + Axum) · 單一靜態二進制 (1.7MB) · 雙引擎智慧路由 · 內建動態 VHDX 引擎 · 模式 2 超大檔案穩定串流 · 輕量級 NAS / 路由器開箱即用*

[![Release](https://img.shields.io/github/v/release/obhasashare2024-namo/iventoymersed?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed/releases)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-x86__64%20%7C%20armv7%20%7C%20aarch64-orange?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed/releases)
[![Musl Static](https://img.shields.io/badge/libc-musl%20(static)-green?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed)

---

## 目录 / Table of Contents / 目次

- [🇨🇳 中文說明 (Default Chinese)](#-中文說明-default-chinese)
  - [🚀 v3.3.0 重大突破與核心特色](#-v330-重大突破與核心特色)
  - [💎 核心特性深入解析](#-核心特性深入解析)
    - [1. 純 Rust 內建動態 VHDX 引擎 (Pure Rust In-Tree Dynamic VHDX Engine)](#1-純-rust-內建動態-vhdx-引擎-pure-rust-in-tree-dynamic-vhdx-engine)
    - [2. 模式 2（NetLoop GRUB HTTP 206 Loopback）超大檔案穩定支援](#2-模式-2netloop-grub-http-206-loopback超大檔案穩定支援)
    - [3. 輕量級 NAS 與嵌入式路由器專屬優化 (OpenWrt / TrueNAS / CasaOS)](#3-輕量級-nas-與嵌入式路由器專屬優化-openwrt--truenas--casaos)
  - [🔀 雙引擎智慧路由架構 (Dual-Engine Smart Routing)](#-雙引擎智慧路由架構-dual-engine-smart-routing)
  - [📊 引擎方案對比速查表](#-引擎方案對比速查表)
  - [📦 快速部署指南 (Quick Start)](#-快速部署指南-quick-start)
  - [🛠️ 輕量 NAS 與 OpenWrt 部署實踐](#️-輕量-nas-與-openwrt-部署實踐)
- [🇺🇸 English Documentation](#-english-documentation)
  - [Overview & v3.3.0 Breakthroughs](#overview--v330-breakthroughs)
  - [Architecture & Key Highlights](#architecture--key-highlights)
  - [Lightweight NAS & OpenWrt Edge Deployment](#lightweight-nas--openwrt-edge-deployment)
  - [Quick Start](#quick-start)
- [🇯🇵 日本語ドキュメント](#-日本語ドキュメント)
  - [概要および v3.3.0 の主要な進化点](#概要および-v330-の主要な進化点)
  - [アーキテクチャと主な特徴](#アーキテクチャと主な特徴)
  - [軽量 NAS および OpenWrt への導入](#軽量-nas-および-openwrt-への導入)
  - [クイックスタート](#クイックスタート)
- [🤝 開源社群協作與反饋 (Community & Feedback)](#-開源社群協作與反饋-community--feedback)

---

## 🇨🇳 中文說明 (Default Chinese)

### 🚀 v3.3.0 重大突破與核心特色

`iventoymersed` 是專為現代無盤網絡啟動、虛擬化維護與邊緣儲存打造的**純 Rust 雙軌融合網絡引導伺服器**。在最新發布的 **v3.3.0** 中，我們實現了三大深水區重大突破：

1. **純 Rust 內建動態 VHDX 解析與 COW 差分引擎**：
   - 徹底終結對宿主機 `qemu-nbd`、`kpartx`、`dmsetup` 等龐大外部工具的依賴；
   - 內建微軟 VHDX 規範二進制解碼器，記憶體動態映射 BAT（區塊分配表），記憶體佔用極低（75GB VHDX 僅需 ~296KB RAM）；
   - 原生支援 COW（寫時複製）差分層（`.diff`），完美保護只讀母盤，客戶端重啟即秒級還原；
   - 完美適配 32 位元 ARMv7 指令集，全 64 位元整數運算防止扇區定址溢位，實測 Windows 11 25H2 實機與 QEMU 雙向跑通桌面，OCR 驗收 100% 通過。
2. **模式 2（NetLoop GRUB HTTP 206 Loopback）超大檔案穩定性加固**：
   - 實測完美支援 **142GB COS (CachyOS) / 75GB Win11 VHDX / 60GB Artix / 8GB antiX** 巨型虛擬磁盤；
   - HTTP 206 Byte-Range 核心修復：全面適配大小寫無關的 HTTP Header（修復 GRUB HTTP 模組在小寫 `content-range` / `content-length` 下的崩潰解析問題）；
   - Fixed VHD 自動識別與 512B Conectix 尾部自動剔除，徹底規避 GPT/MBR 分區表因扇區偏移錯位而損壞的頑疾；
   - 伺服器端純無狀態（Stateless），零 NBD/iSCSI 長連線負擔，局域網實測隨機讀取吞吐達 186 MB/s。
3. **全平台輕量 NAS 與開源路由器開箱即用支援**：
   - 靜態編譯 Musl 二進制（體積僅 1.7MB ~ 1.9MB），零 glibc 依賴；
   - 完美運行於 **OpenWrt (op)**、極路由 X（Gee K2P/HC5961 / Cortex-A7）、**TrueNAS**、**CasaOS**、**Unraid**、群暉/威聯通/綠聯/極空間等邊緣硬體；
   - 運行時 RSS 記憶體佔用 < 15MB，真正實現「15 元路由器 / 舊電視盒子秒變企業級無盤 PXE 伺服器」。

---

### 💎 核心特性深入解析

#### 1. 純 Rust 內建動態 VHDX 引擎 (Pure Rust In-Tree Dynamic VHDX Engine)

傳統網絡啟動在處理微軟動態 VHDX 時，必須依賴 `qemu-nbd` 進行塊設備映射。然而 `qemu-nbd` 不僅依賴龐大的 glibc 與 QEMU 運行庫（在 OpenWrt 或精簡 NAS 上難以部署），而且在高並發或異常斷開時極易留下孤兒進程或死鎖 `/dev/nbd*` 節點。

`iventoymersed` v3.3.0 在內部以純 Rust 原生實現了完整微軟 VHDX 規範：
- **結構化規範解碼**：精準解析 VHDX 檔案標頭 (`vhdxfile`)、區域表 (`regi`)、BAT 區塊分配表 GUID (`6677c22d-23f6-4200-9d64-115e9bfd4a08`) 以及元數據表 (`metadata`)；
- **BAT 記憶體索引映射**：讀取 Payload 區塊狀態（State 6 = `PAYLOAD_BLOCK_FULLY_PRESENT`）並將 64 位元檔案偏移常駐於 `Arc<Vec<u64>>`。對於 75GB 虛擬容量、2MB 區塊大小的動態 VHDX，BAT 表僅需約 37,500 個條目，**運行時僅消耗 296 KB 記憶體**！
- **透明隨機讀取與空洞填零**：遇到未分配的稀疏區塊自動填充 `0x00`，遇到跨越 2MB 區塊邊界的隨機 I/O 自動分段處理；
- **COW 差分保護層**：掛載時自動啟用 `.diff` 差分檔案與寫入位圖（Bitmap）。母盤以只讀方式保護，客戶端所有的修改、分區操作、快取寫入均導向差分層，確保黃金母盤絕對安全；
- **硬體級 32 位元 ARMv7 適配**：针对 Cortex-A7 等 32 位元處理器，所有扇區偏移、區塊索引與檔案長度運算均嚴格採用 `u64`，根除了 32 位元算術溢位引發的檔案指針死鎖。

```
+-------------------------------------------------------------------------+
|                  Client Request: Read Sector (Virtual Offset)            |
+-------------------------------------------------------------------------+
                                    |
                    [Check COW Bitmap / Diff File]
                    /                            \
           (Written in Diff?)                     (Clean / Base)
                 /                                      \
    [Read from .diff File]                     [Translate via VHDX BAT]
                                                        |
                                          BAT[BlockIndex] -> FileOffset
                                          + BlockOffset (in 2MB chunk)
                                                        |
                                            [Read Raw Base VHDX]
                                            (or Zero-fill if sparse)
```

#### 2. 模式 2（NetLoop GRUB HTTP 206 Loopback）超大檔案穩定支援

**模式 2（Engine B - Pure NetLoop）** 徹底顛覆了傳統無盤「必須在伺服器端維持長連線塊設備」的模式：
- **伺服器端純無狀態（Stateless）**：伺服器只是一個標準的 HTTP 1.1 伺服器，利用 `RFC 7233 / Byte-Range 206` 進行切片串流。即便有 100 台客戶端同時啟動，伺服器也無需維護任何塊設備會話，網絡抖動也不會引發連線卡死；
- **客戶端 GRUB 原生解析**：客戶端啟動 `grubnet.efi` 後，直接執行：
  ```grub
  loopback loop (http,${net_default_server}:26001)/iso/0001COSext4.vhd.vtoy
  linux (loop,gpt2)/boot/vmlinuz-linux-cachyos ...
  initrd (loop,gpt2)/boot/initramfs-linux-cachyos.img
  boot
  ```
  GRUB 直接向伺服器發送隨機 Range 請求，在客戶端記憶體中透明解析 GPT 分區表與 ext4/btrfs 檔案系統，**如同將 142GB 的遠程磁盤以光速隨機存取插入客戶端！**
- **HTTP 小寫 Header 完美相容**：修復了部分 GRUB2 構建在解析 HTTP 響應時對大小寫敏感的深水 Bug，確保 `content-range`、`content-length`、`accept-ranges` 均能被 GRUB 內建網絡堆疊 100% 穩定識別；
- **Fixed VHD 智慧切除 512B 尾部**：微軟固定大小 VHD（Fixed VHD）本質是原始 Raw 扇區加上末尾 512 位元組的 Conectix 簽名。模式 2 自動識別該簽名並在導出時自動剔除 512B 尾部，確保扇區邊界與 GPT/MBR 嚴格對齊，杜絕分區丟失。

#### 3. 輕量級 NAS 與嵌入式路由器專屬優化 (OpenWrt / TrueNAS / CasaOS)

傳統無盤軟體動輒數百 MB，且深度綁定 Windows Server 或大型 Linux 發行版。`iventoymersed` 是市面上極少數專為**邊緣嵌入式硬體**量身定做的方案：
- **OpenWrt (op) 路由器完美支援**：
  - 極路由 X（HC5961，雙核 Cortex-A7 ARMv7）、MT7621、IPQ4019/6000/807x、Rockchip 等路由器均可直接運行；
  - 1.7MB 靜態 Musl 二進制，可直接放置於外接隨身碟、SATA 硬碟或 `/tmp` 記憶體中，免依賴任何 `opkg` 運行庫；
  - 內建物聯網專用輕量 HTTP 靜態檔案引擎，隨機並發讀取下 CPU 佔用率 < 5%。
- **主流輕量 NAS 深度整合**：
  - **TrueNAS (CORE / SCALE)**：可在獨立 Jail 或 Docker 容器中以極簡模式常駐；
  - **CasaOS / ZimaOS**：單一 Docker 容器或二進制服務直接管理共享目錄；
  - **Unraid**：支援直接掛載 Array/Cache 上的鏡像目錄，無須額外 VM 開銷；
  - **綠聯 (UGREEN) / 極空間 / 群暉 (Synology) / 威聯通 (QNAP)**：一鍵以背景服務運行，極致省電且記憶體佔用忽略不計。

---

### 🔀 雙引擎智慧路由架構 (Dual-Engine Smart Routing)

`iventoymersed` 自動根據映像類型與作業系統特性分流至最佳引擎：

```mermaid
flowchart TD
    Client[客戶端 PXE / UEFI 啟動] --> DHCP[DHCP 獲取網路參數]
    DHCP --> M0[統一選單: /boot.ipxe\n(Unified OS Boot Menu)]
    
    M0 -->|Windows 11/10 VHDX| WinTrack[Engine A: Pure Rust In-Tree NBD\n(原生 COW 差分 / 0 RAM 耗損 / 免 qemu-nbd)]
    M0 -->|Linux VHD / ISO (142GB COS / Artix)| LinuxTrack[Engine B: NetLoop GRUB Loopback\n(無狀態 HTTP 206 / 186 MB/s 隨機串流)]
    M0 -->|WinPE 維護映像 (ISO)| PETrack[Engine B 接管: iPXE HTTP Range SANBOOT\n(避開 0xc000000e 韌體死鎖 / 直通 2K 桌面)]
```

---

### 📊 引擎方案對比速查表

| 指標 / 特性 | Engine A (Pure Rust NBD / iSCSI) | Engine B (Pure NetLoop GRUB HTTP 206) |
| :--- | :--- | :--- |
| **主要適用作業系統** | **Windows 11 / Windows 10 VHDX** | **Linux (Arch/COS/Artix/antiX/Debian) & PE** |
| **外部工具依賴** | **零依賴**（純 Rust 內建 BAT 解碼） | **零依賴**（純標準 HTTP 1.1 靜態伺服器） |
| **伺服器狀態開銷** | 極低（無鎖並行 pread/pwrite） | **純無狀態 (0% 狀態維持，隨連隨走)** |
| **客戶端 RAM 佔用** | **0 MB**（由 iPXE / 內核塊驅動直接映射） | 僅需加載內核與 initramfs（~50MB-200MB） |
| **超大檔案支援能力** | 支援 100GB+ 動態 VHDX（BAT 表僅數百 KB） | **實測穩定支援 142GB COS 巨型磁盤** |
| **寫入保護與還原** | **原生 COW 差分層 (.diff)，重啟自動還原** | 記憶體 OverlayFS，重啟無痕還原 |
| **輕量 NAS / 路由器相容性** | 完美（支援 ARMv7 / aarch64 / x86_64） | 完美（任意靜態 HTTP 伺服器即可驅動） |

---

### 📦 快速部署指南 (Quick Start)

#### 1. 下載預編譯靜態發布包
從 [GitHub Releases](https://github.com/obhasashare2024-namo/iventoymersed/releases) 下載對應架構的壓縮包：
- `x86_64` (通用 PC / NAS / 伺服器)：`iventoymersed-v3.3.0-linux-x86_64.tar.gz`
- `armv7` (32 位元 ARM / OpenWrt 路由器 / 樹莓派 2/3)：`iventoymersed-v3.3.0-linux-armv7.tar.gz`
- `aarch64` (64 位元 ARM / 樹莓派 4/5 / 軟路由 / Apple Silicon)：`iventoymersed-v3.3.0-linux-aarch64.tar.gz`

#### 2. 解壓並啟動
```bash
tar -xzf iventoymersed-v3.3.0-linux-x86_64.tar.gz
chmod +x iventoymersed

# 啟動服務（指定磁盤鏡像目錄與監聽端口，預設 26001）
./iventoymersed --dir /mnt/disks/vdisks --port 26001
```

#### 3. 配置網關 DHCP / TFTP
在局域網 DHCP 伺服器（如 OpenWrt dnsmasq 或 ISC DHCP）中添加配置：
```dnsmasq
# 指定預設 PXE 啟動檔案
dhcp-boot=undionly.kpxe

# 指向 iventoymersed 統一選單
dhcp-option-force=209,http://192.168.1.1:26001/boot.ipxe
```

---

### 🛠️ 輕量 NAS 與 OpenWrt 部署實踐

#### 在 OpenWrt (op) 路由器上部署（以極路由 X HC5961 / MT7621 / ARMv7 為例）
1. 將 `iventoymersed-armv7` 上傳至路由器的外接儲存設備（如 `/mnt/sda1/iventoy/`）：
   ```bash
   scp iventoymersed-armv7 root@192.168.1.1:/mnt/sda1/iventoy/iventoymersed
   ```
2. 在 OpenWrt 終端設置執行權限並啟動：
   ```bash
   chmod +x /mnt/sda1/iventoy/iventoymersed
   COW_DIR=/mnt/sda1/cow /mnt/sda1/iventoy/iventoymersed --dir /mnt/sda1/iso --port 26001 > /var/log/iventoy.log 2>&1 &
   ```
3. 在 `/etc/config/dhcp` 中加入無盤啟動項：
   ```uci
   uci add_list dhcp.@dnsmasq[0].dhcp_boot='undionly.kpxe,,192.168.1.1'
   uci commit dhcp
   /etc/init.d/dnsmasq restart
   ```
4. 開機客戶端 PC，即刻享受全速千兆無盤秒級引導！

---

## 🇺🇸 English Documentation

### Overview & v3.3.0 Breakthroughs

`iventoymersed` is an ultra-lightweight, high-performance **dual-engine netboot server written in 100% Pure Rust (Tokio + Axum)**. In version **v3.3.0**, it introduces three landmark innovations:

1. **In-Tree Pure Rust Dynamic VHDX & COW Engine**:
   - Zero external dependencies: completely eliminates `qemu-nbd`, `kpartx`, and `dmsetup`;
   - Full Microsoft VHDX specification decoder with in-memory BAT (Block Allocation Table) caching (~296 KB RAM footprint for a 75GB disk);
   - Transparent COW (Copy-On-Write) difference overlay (`.diff`), guaranteeing immutable golden master protection and instant reboot recovery;
   - 32-bit ARMv7 (Cortex-A7) arithmetic optimization: full 64-bit integer indexing avoids addressing overflows, verified with Windows 11 25H2 live boot and OCR visual pass.
2. **Mode 2 (NetLoop GRUB HTTP 206 Loopback) Large File Stability**:
   - Production-tested with massive virtual images: **142GB COS (CachyOS) / 75GB Win11 VHDX / 60GB Artix / 8GB antiX**;
   - Case-insensitive HTTP Header normalization for `content-range` and `content-length`, fixing critical GRUB HTTP parser crash bugs;
   - Automatic Fixed VHD detection and 512-byte Conectix trailer stripping to ensure strict sector alignment;
   - 100% Stateless server design: zero persistent socket overhead, delivering 186 MB/s random byte-range throughput.
3. **Turnkey Support for Lightweight NAS & OpenWrt Routers**:
   - Compact static Musl binaries (1.7 MB ~ 1.9 MB), zero glibc runtime requirements;
   - Tested and supported on **OpenWrt (op)**, Extreme Router X (Gee K2P/HC5961 / Cortex-A7), **TrueNAS**, **CasaOS**, **Unraid**, and Synology/QNAP/UGREEN NAS;
   - Runtime RSS memory footprint < 15MB, transforming edge routers into enterprise-grade PXE servers.

---

### Architecture & Key Highlights

```mermaid
flowchart TD
    Client[Client PXE/UEFI Netboot] --> DHCP[DHCP Network Config]
    DHCP --> M0[Unified Menu: /boot.ipxe]
    
    M0 -->|Windows 11/10 VHDX| WinTrack[Engine A: Pure Rust In-Tree NBD\n(COW Diff Layer / Zero RAM Loss / Zero qemu-nbd)]
    M0 -->|Linux VHD / ISO (142GB COS)| LinuxTrack[Engine B: NetLoop GRUB Loopback\n(Stateless HTTP 206 / 186 MB/s Streaming)]
    M0 -->|WinPE ISO| PETrack[Engine B Proxy: iPXE HTTP Range SANBOOT\n(Bypasses 0xc000000e Firmware Bug / 2K Desktop)]
```

---

### Lightweight NAS & OpenWrt Edge Deployment

Deploying on an OpenWrt router (e.g., Gee K2P / HC5961 / Cortex-A7 ARMv7):
```bash
# Transfer binary to router storage
scp iventoymersed-armv7 root@192.168.1.1:/mnt/sda1/iventoy/iventoymersed

# Run with COW differential layer
chmod +x /mnt/sda1/iventoy/iventoymersed
COW_DIR=/mnt/sda1/cow /mnt/sda1/iventoy/iventoymersed --dir /mnt/sda1/iso --port 26001 > /var/log/iventoy.log 2>&1 &
```

---

### Quick Start

```bash
# Download and untar release
tar -xzf iventoymersed-v3.3.0-linux-x86_64.tar.gz
chmod +x iventoymersed

# Start server
./iventoymersed --dir /path/to/images --port 26001
```

---

## 🇯🇵 日本語ドキュメント

### 概要および v3.3.0 の主要な進化点

`iventoymersed` は、Pure Rust（Tokio + Axum）で開発された**デュアルエンジン統合型ネットワークブート（PXE/Netboot）サーバー**です。最新の **v3.3.0** では、以下の画期的な機能が追加されました：

1. **Pure Rust 組み込み動的 VHDX 解析および COW 差分エンジン**：
   - `qemu-nbd` や `kpartx` などの外部ホスト依存を完全に排除；
   - マイクロソフト VHDX 仕様の完全デコードと BAT（ブロック割り当てテーブル）のインメモリ展開により、メモリ消費を極小化（75GB VHDX でもわずか約 296KB の RAM 使用量）；
   - COW（コピーオンライト）差分レイヤー（`.diff`）をネイティブサポートし、マスターイメージを完全に保護したまま再起動で初期状態へ復元可能；
   - 32 ビット ARMv7（Cortex-A7）最適化により、アドレス計算のオーバーフローを徹底防止。Windows 11 25H2 の実機および QEMU 双方でのデスクトップ起動・OCR 認識を 100% 達成。
2. **Mode 2（NetLoop GRUB HTTP 206 Loopback）大容量ファイルの安定性向上**：
   - **142GB COS (CachyOS) / 75GB Win11 VHDX / 60GB Artix / 8GB antiX** などの超大容量イメージの安定起動を実証；
   - 大文字・小文字を区別しない HTTP ヘッダー（`content-range` / `content-length`）の正常処理により、GRUB HTTP モジュールのパーサークラッシュを解消；
   - 固定 VHD（Fixed VHD）の自動検出と 512 バイト Conectix トレーラーの自動除外により、セクタ境界のズレを根絶；
   - ステートレス設計（サーバー側のセッション維持ゼロ）により、ギガビット LAN で 186 MB/s の高速ランダムストリーミングを実現。
3. **軽量 NAS および OpenWrt ルーターへの即時導入**：
   - 1.7MB〜1.9MB の完全静的 Musl バイナリ（glibc 不要）；
   - **OpenWrt**、極路由 X（HC5961 / ARMv7）、**TrueNAS**、**CasaOS**、**Unraid**、各種市販 NAS でそのまま動作；
   - 常駐メモリ（RSS）15MB 未満で、安価なルーターを高性能ディスクレスサーバーへ変貌させます。

---

### アーキテクチャと主な特徴

- **Windows 11 / 10 VHDX**：Pure Rust 組み込み NBD + COW 差分により、クライアントの RAM を消費せずネイティブ iBFT/SANBOOT 起動。
- **Linux 各種（142GB COS / Artix 等）**：Mode 2 GRUB HTTP 206 Loopback により、クライアントが必要なセクタのみを随時ストリーミング取得。
- **WinPE レスキューイメージ**：iPXE HTTP Range 206 経由で bootmgr の `0xc000000e` 死鎖を回避し、高解像度デスクトップへ直通。

---

### 軽量 NAS および OpenWrt への導入

```bash
# OpenWrt ルーターへの転送と起動
scp iventoymersed-armv7 root@192.168.1.1:/mnt/sda1/iventoy/iventoymersed
chmod +x /mnt/sda1/iventoy/iventoymersed
COW_DIR=/mnt/sda1/cow /mnt/sda1/iventoy/iventoymersed --dir /mnt/sda1/iso --port 26001 &
```

---

### クイックスタート

```bash
tar -xzf iventoymersed-v3.3.0-linux-x86_64.tar.gz
chmod +x iventoymersed
./iventoymersed --dir /mnt/disks/vdisks --port 26001
```

---

## 🤝 開源社群協作與反饋 (Community & Feedback)

本專案致力於開拓邊緣計算與網絡無盤引導的技術極限。由於各廠牌主板韌體（UEFI/BIOS）、網卡 PXE ROM 以及網絡環境千差萬別，我們由衷期待來自開源社群朋友的反饋！

- **問題回報與硬體相容性登記**：歡迎前往 [GitHub Issues](https://github.com/obhasashare2024-namo/iventoymersed/issues) 分享您的主板與網卡測試數據；
- **架構探討與功能建言**：歡迎在 [Discussions](https://github.com/obhasashare2024-namo/iventoymersed/discussions) 中與我們交流；
- **貢獻代碼**：熱烈歡迎 Pull Request！

*感謝每一位探索無盤與虛擬化邊界的開源同行者！*
