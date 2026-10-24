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
- [🇯🇵 日本語ドキュメント](#-日本語ドキュメント)
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

### 📦 快速部署指南 (Quick Start)

#### 啟動服務
```bash
# 啟動服務（指定磁盤鏡像目錄與監聽端口，預設 26001）
./iventoymersed --dir /mnt/disks/vdisks --port 26001
```

#### 配置網關 DHCP / TFTP
在局域網 DHCP 伺服器（如 OpenWrt dnsmasq 或 ISC DHCP）中添加配置：
```dnsmasq
dhcp-boot=undionly.kpxe
dhcp-option-force=209,http://192.168.1.1:26001/boot.ipxe
```

---

## 🇺🇸 English Documentation

### Overview & Highlights
`iventoymersed` v3.3.0 is an ultra-lightweight, high-performance dual-engine netboot server written in 100% Pure Rust. It features an in-tree pure Rust Dynamic VHDX & COW engine, Mode 2 (NetLoop GRUB HTTP 206) streaming for large virtual disks (142GB COS, 75GB Win11 VHDX), and turnkey support for lightweight NAS and OpenWrt routers.

---

## 🇯🇵 日本語ドキュメント

### 概要およびハイライト
`iventoymersed` v3.3.0 は、Pure Rust で開発された統合型ネットワークブートサーバーです。Pure Rust 組み込み動的 VHDX 解析および COW 差分エンジン、Mode 2（NetLoop GRUB HTTP 206）による大容量仮想ディスク（142GB COS、75GB Win11 VHDX）の安定起動、そして OpenWrt や軽量 NAS への即時導入をサポートしています。
