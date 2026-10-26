# iventoymersed (Dual-Engine Unified Netboot Server)

**Merged & Immersed: 雙軌融合 iVentoy/iPXE + NetLoop GRUB 全格式網絡啟動一體機**  
*Pure Rust (Tokio + Axum) · 單一靜態二進制 (1.7MB) · 雙引擎智慧路由 · 內建動態 VHDX 引擎 · 內建純 Rust GPT/BCD 自動注入 · Linux 多重 Initrd 動態疊加零侵入無盤 · 模式 2 超大檔案穩定串流 · 輕量級 NAS / 路由器開箱即用*

[![Release](https://img.shields.io/github/v/release/obhasashare2024-namo/iventoymersed?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed/releases)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-x86__64%20%7C%20armv7%20%7C%20aarch64-orange?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed/releases)
[![Language](https://img.shields.io/badge/language-100%25%20Pure%20Rust-blueviolet?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed)
[![Musl Static](https://img.shields.io/badge/libc-musl%20(static)-green?style=flat-square)](https://github.com/obhasashare2024-namo/iventoymersed)

---

### 📚 三語保姆式圖文教程 / Step-by-Step Tutorials / 保守・導入チュートリアル

> **最新實測保姆式圖文教程已全量發布！涵蓋旁路由解耦集群、零硬編碼、純 Web 0 命令行操控、客戶端 MAC/靜態 IP 預選與 Win11/Linux 雙實機開機驗證：**  
> - 🇹🇼 **[繁體中文保姆式圖文教程 (完整圖文步驟 · 解耦集群 · 0 CLI 純 Web 操控 · 實測驗收)](docs/TUTORIAL.md)**  
> - 🇺🇸 **[English Step-by-Step Tutorial (Decoupled Cluster · Zero-CLI Web Control · Live Verification)](docs/TUTORIAL_EN.md)**  
> - 🇯🇵 **[日本語詳細チュートリアル (分離クラスタ · 完全 Web 管理 · 実機検証)](docs/TUTORIAL_JA.md)**

---

## 目录 / Table of Contents / 目次

- [📚 三語保姆式圖文教程 / Tutorials / チュートリアル](#-三語保姆式圖文教程--step-by-step-tutorials--保守導入チュートリアル)
- [🇹🇼 繁體中文說明 (Default Traditional Chinese)](#-繁體中文說明-default-traditional-chinese)
  - [🚀 v3.5.0 重大突破與核心特色](#-v350-重大突破與核心特色)
  - [💎 核心特性深入解析](#-核心特性深入解析)
    - [1. 純 Rust 內建全格式虛擬磁盤與 COW 引擎 (Raw, VHD, VHDX, VMDK, VDI, QCOW2)](#1-純-rust-內建全格式虛擬磁盤與-cow-引擎-raw-vhd-vhdx-vmdk-vdi-qcow2)
    - [2. 純 Rust 原生 GPT 審計與 BCD Hive 二進制動態重寫 (In-Tree GPT & BCD Auto-Patcher)](#2-純-rust-原生-gpt-審計與-bcd-hive-二進制動態重寫-in-tree-gpt--bcd-auto-patcher)
    - [3. Linux 模式 2「多重 Initrd 動態疊加（Multiple Initrd Dynamic Overlay）」零侵入無盤](#3-linux-模式-2多重-initrd-動態疊加multiple-initrd-dynamic-overlay零侵入無盤)
    - [4. 多操作系統實測驗收矩陣 (antiX Linux / OpenBSD / OpenWrt / Win11)](#4-多操作系統實測驗收矩陣-antix-linux--openbsd--openwrt--win11)
    - [5. 輕量級 NAS 與嵌入式路由器專屬優化 (OpenWrt / TrueNAS / CasaOS)](#5-輕量級-nas-與嵌入式路由器專屬優化-openwrt--truenas--casaos)
  - [🔀 雙引擎智慧路由架構 (Dual-Engine Smart Routing)](#-雙引擎智慧路由架構-dual-engine-smart-routing)
  - [📊 引擎方案對比速查表](#-引擎方案對比速查表)
  - [📦 快速部署指南 (Quick Start)](#-快速部署指南-quick-start)
  - [🛠️ 輕量 NAS 與 OpenWrt 部署實踐](#️-輕量-nas-與-openwrt-部署實踐)
- [🇺🇸 English Documentation](#-english-documentation)
  - [Overview & v3.5.0 Breakthroughs](#overview--v350-breakthroughs)
  - [Architecture & Key Highlights](#architecture--key-highlights)
  - [Pure Rust Multi-Format Virtual Disk Engine](#pure-rust-multi-format-virtual-disk-engine)
  - [Pure Rust In-Tree GPT & BCD Engine](#pure-rust-in-tree-gpt--bcd-engine)
  - [Multiple Initrd Dynamic Overlay for Linux](#multiple-initrd-dynamic-overlay-for-linux)
  - [Lightweight NAS & OpenWrt Edge Deployment](#lightweight-nas--openwrt-edge-deployment)
  - [Quick Start](#quick-start)
- [🇯🇵 日本語ドキュメント](#-日本語ドキュメント)
  - [概要および v3.5.0 の主要な進化点](#概要および-v350-の主要な進化点)
  - [アーキテクチャと主な特徴](#アーキテクチャと主な特徴)
  - [Pure Rust 組み込み全仮想ディスク対応 (Raw, VHD, VHDX, VMDK, VDI, QCOW2)](#pure-rust-組み込み全仮想ディスク対応-raw-vhd-vhdx-vmdk-vdi-qcow2)
  - [Pure Rust 組み込み GPT / BCD 自動注入](#pure-rust-組み込み-gpt--bcd-自動注入)
  - [Linux 複数 Initrd 動的オーバーレイ（ゼロ侵入ディスクレス）](#linux-複数-initrd-動的オーバーレイゼロ侵入ディスクレス)
  - [軽量 NAS および OpenWrt への導入](#軽量-nas-および-openwrt-への導入)
  - [クイックスタート](#クイックスタート)
- [🤝 開源社群協作與反饋 (Community & Feedback)](#-開源社群協作與反饋-community--feedback)

---

## 🇹🇼 繁體中文說明 (Default Traditional Chinese)

### 🚀 v3.5.1 重大突破：全動態自適應網絡架構（Zero-Hardcoded IP）與旁路由集群解耦調度

> [!TIP]
> 📖 **新手推薦閱讀**：想要一步到位配置旁路由與 NAS 分離集群？請直接查閱 🇹🇼 **[繁體中文保姆式圖文教程](docs/TUTORIAL.md)**，內含手把手步驟、拓撲全景圖、Web UI 實機截圖與 Windows 11/antiX Linux 開機實測驗收！

在最新發布的 **v3.5.1** 中，我們進一步實現了大規模網絡引導環境下的**控制面與數據面物理級分離**，徹底終結代碼與前端中任何寫死 IP 的痛點：

1. **控制面與數據面解耦集群 (Decoupled Cluster Architecture)**：
   - **調度大腦 (Orchestrator)**：超輕量部署於 OpenWrt 旁路由（如 91.11, ARMv7 musl 靜態二進制，RSS < 2MB），掌管 Web 控制台、客戶端 MAC/IP 預選綁定與動態 iPXE 引導腳本；
   - **存儲節點 (Storage Node)**：部署於 NAS 或實體工作站（如 artix3070），承載 2TB+ 虛擬磁盤（Raw / VHD / VHDX / VMDK / QCOW2）之 In-Tree NBD 與 iSCSI 塊設備；
   - **熱切換與自適應**：前端 Web 控制台支援手動選擇角色（Orchestrator / Standalone / Storage Node），並可自由填寫遠端數據節點 IP 及 Port，或留空自適應 `${next-server}`。
2. **客戶端 MAC 預選與客戶端專屬靜態 IP 綁定 (Client MAC & Static IP Overrides)**：
   - 支援為指定客戶端網卡 MAC 預先綁定目標系統鏡像，並可指定專屬客戶端 IP；
   - **iPXE 雙通道注入**：Query 傳 MAC 時伺服器直接注入 `# Static IP override\nset ip <ip>`；通用菜單則注入 `iseq ${net0/mac} <mac> && set ip <ip> ||` 運行時動態求值；
   - 規則全數持久化至 `iventoy.json`，客戶端開機自動精準命中。
3. **動態引導倒數與客戶端手工挑選自由 (Dynamic Boot Timeout & Manual Override Freedom)**：
   - 頂部導航欄提供動態倒數秒數配置（`POST /api/config/timeout`，1~300 秒，預設 10 秒）；
   - **痛點修正**：徹底移除傳統 `goto <target>` 剝奪現場管理員手選權限的硬性跳轉，改為 `iseq ${net0/mac} <mac> && set menu-default <target> ||` 搭配 `choose --timeout`；
   - **雙軌行為**：無人值守時倒數歸零自動開進預選母盤；倒數期間現場一旦觸發鍵盤方向鍵（Up/Down），**計時瞬間中止並常駐選單**，用戶保留 100% 自由挑選救援 WinPE、Linux 或其他母盤的權限！
4. **解耦集群遠端磁盤目標動態嗅探 (Decoupled Target Discovery)**：
   - 旁路由調度大腦主動向遠端存儲節點（如 `192.168.91.29:26001`）拉取 `/api/status` 目標池；
   - 即時將遠端母盤同步至 Web 控制台下拉選單與 iPXE 選單樹，旁路由維持零磁碟佔用與極低記憶體開銷（RSS < 2.5MB）。
5. **全動態主機與 IP 感知 (Zero-Hardcoded IP)**：
   - 前端 Web 控制台實時依據 `window.location.host` 渲染，絕不假定特定網段；
   - 後端引導腳本全面以 `${srv}` 與 `${net_default_server}` 動態替換，杜絕歷史硬編碼。
6. **前端現代化非阻塞體驗加固 (Non-Blocking Web UI)**：
   - 全面以非阻塞浮動通知（`showToast()`）取代傳統阻斷式 `window.alert()`；
   - 目標鏡像輸入框升級為 HTML5 `<input list="...">` 雙模封裝，兼顧本地自動補全與純調度大腦模式下的手填自訂。

### 🚀 v3.5.0 核心特色回顧

`iventoymersed` 是專為現代無盤網絡啟動、虛擬化維護與邊緣儲存打造的**純 Rust 雙軌融合網絡引導伺服器**：

1. **純 Rust 內建全格式虛擬磁盤解析與 COW 差分引擎**：
   - 徹底終結對宿主機 `qemu-nbd`、`kpartx`、`dmsetup` 等外部重型工具的依賴；
   - 原生支援 **Raw / ISO、Fixed VHD、Dynamic VHD (VPC)、Dynamic VHDX、VMware VMDK (monolithicSparse)、VirtualBox VDI (dynamic) 以及 QEMU QCOW2 (v2/v3)**；
   - 所有格式均無縫接入內建的 `CowStorage` 寫時複製層，母盤物理唯讀不可篡改，客戶端重啟即秒級還原；
   - 完美適配 32 位元 ARMv7 指令集與 64 位元 x86/ARM，防止扇區定址溢位。
2. **多操作系統矩陣實測跑通 (Multi-OS Boot Verification Matrix)**：
   - **antiX Linux (Desktop)**：經 VMDK 格式引導跑通 IceWM 桌面，`verify-desktop-ocr` 驗收 100% 通過；
   - **OpenBSD 7.7 (BSD Minimal)**：經 5.6MB 稀疏 img、QCOW2 與 Dynamic VHD 順利加載內存盤根分區 `rd0a` 並進入安裝控制台；
   - **OpenWrt 23.05 (Appliance)**：經 QCOW2 格式秒級完成內核解包，`procd` 初始化成功，`br-lan` 網絡就緒；
   - **Windows 8 / 10 / 11 (Desktop & PE)**：純 Rust GPT/BCD 引擎動態重寫 16 位元組 Disk/Partition GUID，實現零侵入無盤秒啟。
3. **純 Rust 內建 GPT 分區審計與 BCD Hive 二進制動態重寫引擎**：
   - 徹底移除對外部 Python 腳本、`7z`、`sgdisk` 的依賴，倉庫 100% 純 Rust 化；
   - 原生二進制解析 GPT 標頭（LBA 1, `EFI PART`）與分區表（LBA 2），毫秒級識別 ESP 分區是否存在，並精確提取 Disk GUID 與目標 Windows 分區 Unique GUID；
   - 內嵌微軟 BCD 註冊表 Hive 二進制替換邏輯，自動重寫 16 位元組混合序 GUID（5 處 Disk + 5 處 Partition 精準替換）；
   - 提供純 Rust CLI 命令 (`iventoymersed inspect` / `iventoymersed patch-bcd`) 以及動態 HTTP API (`GET /vtoy/bcd?image=<target>`)。
4. **Linux 模式 2「多重 Initrd 動態疊加（Multiple Initrd Dynamic Overlay）」無盤零侵入啟動技術**：
   - 徹底解決 Linux 母盤原生無盤開機頑疾：母盤徹底免跑 `vtoyboot.sh`，100% 保持原生母盤只讀與乾淨；
   - 巧妙利用 Linux 內核與現代 GRUB2 的「多重 Initrd（Multiple Initrd）」記憶體解包覆蓋特性；
   - `initrd (loop,gpt2)/boot/initramfs-linux.img (http,${srv}:26001)/vtoy/vtoy_overlay.cpio`：開機瞬間由伺服器單程序動態載入輕量級通用網絡鉤子，支援 `initramfs-tools`、`dracut` 與 `mkinitcpio` 三大體系。
5. **全平台輕量 NAS 與開源路由器開箱即用支援**：
   - 靜態編譯 Musl 二進制（體積僅 1.7MB ~ 2.0MB），零 glibc 依賴；
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

#### 2. 純 Rust 原生 GPT 審計與 BCD Hive 二進制動態重寫 (In-Tree GPT & BCD Auto-Patcher)

當使用者導入沒有獨立 EFI 系統分區 (ESP) 的 Windows 映像，或從實體硬碟轉儲的單分區 VHDX 時，Windows Boot Manager 會因 BCD 中的 Disk GUID 與 Partition GUID 不匹配而報錯 `0xc000000e`。

傳統方案需依賴 Python 腳本或外置登錄檔編輯器進行修補。`iventoymersed` 將此過程全部收歸純 Rust 內核：
- **原生 GPT 解析器**：在不解包、不依賴外部工具的情況下，直接解析 LBA 1 GPT Header 與 LBA 2 分區表數組，提取 Disk GUID 與分區類型（ESP, Microsoft Basic Data 等）；
- **微軟 BCD 註冊表 Hive 二進制替換**：在記憶體中直接定位登錄檔單元（Element `11000001` OSDevice），將 GUID 按照微軟混合序（前三個欄位 Little-Endian，後 8 個位元組 Big-Endian）進行精準位元組替換；
- **多端調用直通**：
  - 終端檢查：`./iventoymersed inspect <image_path>`
  - 終端補丁：`./iventoymersed patch-bcd <image_path> [out.bcd]`
  - HTTP 動態獲取：客戶端發起 `GET /vtoy/bcd?image=25h2.vhdx`，伺服器即時合成並串流下發 20KB 微軟標準 BCD 檔案。

#### 3. Linux 模式 2「多重 Initrd 動態疊加（Multiple Initrd Dynamic Overlay）」零侵入無盤

##### 為什麼傳統無盤 Linux 母盤需要 `vtoyboot.sh`？
- **Stage 1 (GRUB 階段)**：完全運行在 UEFI/BIOS 網絡棧之上，GRUB2 透過 HTTP 206 協議在記憶體中掛載遠端虛擬磁盤，讀取內核與 initramfs，此階段零依賴；
- **Stage 2 (Linux 內核接管)**：當 GRUB 執行 `boot` 交付給 Linux 內核（`ExitBootServices`）後，GRUB 的 HTTP 網絡棧就被銷毀了。母盤原生自帶的 initramfs 如果未曾注入過 `vtoyboot` 網絡鉤子，就只會在本地 SATA/NVMe 總線上尋找 Root UUID，導致超時報錯 `ALERT! UUID does not exist`。

##### 伺服器單程序如何「零侵入」動態注入？
Linux 內核與現代 GRUB2 原生支援**同時載入多個 initrd**（內核在記憶體中依序解包並自動覆蓋合併同名檔案與鉤子目錄）：
```grub
menuentry "Native Linux (Zero-Touch Multiple Initrd Overlay)" {
    echo "Connecting loopback via HTTP Range..."
    loopback loop (http,${srv}:26001)/iso/archlinux.img
    
    echo "Loading Native Kernel..."
    linux (loop,gpt2)/boot/vmlinuz-linux root=/dev/nbd0p2 rw ip=dhcp nbdroot=${srv} nbdport=10810
    
    echo "Loading Dual Initrd (Native Mother Initrd + Dynamic Server Overlay)..."
    initrd (loop,gpt2)/boot/initramfs-linux.img (http,${srv}:26001)/vtoy/vtoy_overlay.cpio
}
```
`iventoymersed` 內置動態下發 `vtoy_overlay.cpio`，包含靜態 `vtoy-nbd` 連線工具與跨發行版 Hook（相容 `initramfs-tools`、`dracut`、`mkinitcpio`）。母盤無需做任何修改，即可實現完美的開箱即用無盤開機！

#### 4. 模式 2（NetLoop GRUB HTTP 206 Loopback）超大檔案穩定支援

**模式 2（Engine B - Pure NetLoop）** 徹底顛覆了傳統無盤「必須在伺服器端維持長連線塊設備」的模式：
- **伺服器端純無狀態（Stateless）**：伺服器只是一個標準的 HTTP 1.1 伺服器，利用 `RFC 7233 / Byte-Range 206` 進行切片串流。即便有 100 台客戶端同時啟動，伺服器也無需維護任何塊設備會話，網絡抖動也不會引發連線卡死；
- **客戶端 GRUB 原生解析**：客戶端啟動 `grubnet.efi` 後，直接執行：
  ```grub
  loopback loop (http,${net_default_server}:26001)/iso/0001COSext4.vhd.vtoy
  linux (loop,gpt2)/boot/vmlinuz-linux-cachyos root=/dev/nbd0p2 rw
  initrd (loop,gpt2)/boot/initramfs-linux-cachyos.img
  ```
- **極限吞吐實測**：千兆局域網隨機 Byte-Range 讀取速度高達 **186 MB/s**。

---

### 🔀 雙引擎智慧路由架構 (Dual-Engine Smart Routing)

```mermaid
flowchart TD
    Client[客戶端 PXE / UEFI 啟動] --> DHCP[DHCP 獲取網路與下一跳]
    DHCP --> M0[統一導引端點: /boot.ipxe]
    
    M0 -->|Windows 11/10 VHDX / iBFT| WinTrack[引擎 A: 純 Rust 內建 NBD + COW 差分\n(零宿主依賴 / 零 RAM 損耗 / 內建 BCD 注入)]
    M0 -->|Linux 虛擬盤 142GB COS / Artix| LinuxTrack[引擎 B: NetLoop GRUB Loopback\n(純無狀態 HTTP 206 / 多重 Initrd 零侵入疊加)]
    M0 -->|WinPE 維護鏡像| PETrack[引擎 B 代理: iPXE HTTP Range SANBOOT\n(直通 2K 桌面 / 規避 0xc000000e 韌體死鎖)]
```

---

### 📊 引擎方案對比速查表

| 指標 / 方案 | 官方 iVentoy (商業版) | 方案 A: 純 Rust In-Tree NBD | 方案 B: NetLoop HTTP 206 Loopback |
| :--- | :--- | :--- | :--- |
| **代碼語言** | 閉源 C/C++ 專有二進制 | **100% 開源純 Rust** | **100% 開源純 Rust (HTTP 206)** |
| **外部工具依賴** | 依賴閉源驅動模組 | **零依賴 (單一靜態二進制)** | **零依賴 (標準 HTTP 伺服器)** |
| **動態 VHDX 支援** | 僅支援部分 Fixed 格式 | **原生支援 (BAT 記憶體常駐 ~296KB)** | **原生支援 (HTTP 隨機扇區串流)** |
| **母盤侵入性** | 需手動跑腳本 | 支援 COW 差分保護 | **多重 Initrd 動態疊加 (100% 零侵入)** |
| **BCD 補丁機制** | 無 (需手動處理) | **內建純 Rust 自動重寫 (5 處替換)** | **動態 HTTP /vtoy/bcd 下發** |
| **伺服器連線狀態** | 有狀態 (長連線會話) | 有狀態 (COW 差分管線) | **純無狀態 (Stateless, 零併發上限)** |
| **超大盤實測** | >30GB 常發死鎖 | 實測 75GB Win11 穩定秒開 | **實測 142GB COS / 75GB Win11 秒開** |
| **記憶體佔用 (RSS)**| >120 MB | **< 25 MB** | **< 15 MB** |
| **OpenWrt 適配** | 極度困難 (架構閉源) | **開箱即用 (1.7MB armv7 musl)** | **開箱即用 (任何邊緣設備)** |

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
./iventoymersed --config config.yaml
```

---

### 🛠️ 輕量 NAS 與 OpenWrt 部署實踐

> [!TIP]
> **全動態自適應與零硬編碼架構（Dynamic Host & Zero-Hardcoded Gate）**：
> 本專案架構全面具備 **100% 網段與 IP 自適應能力**：
> 1. **前端 Web 控制台**：完全依據瀏覽器當前訪問的 `window.location.host` 動態感知控制面地址與端口，並動態推導數據面連線，絕不寫死任何特定 IP；
> 2. **iPXE / GRUB 引導鏈**：全面採用協議級動態宏變數 `${next-server}` 與 `${net_default_server}`，客戶端從哪個網關獲取引導即自動直連該節點；
> 3. **文檔中的 IP 說明**：下文中所提及之 `192.168.91.11` 僅為實際生產環境的實體旁路由範例（亦實測過 `192.168.188.0/24` 等任意虛擬子網），在任何 `192.168.x.x`、`10.x.x.x` 或企業網段中皆能全自動隨插即用！

#### 1. 外部終端 Web GUI 管理界面 (Control Deck)
`iventoymersed` 雖然支援一行命令背景極速常駐，但其內核**原生整合了現代暗黑科幻風格的 HTML5 Control Deck 網頁控制台**：
- **訪問位址**：在局域網任意設備（電腦/手機/平板）瀏覽器開啟 `http://<伺服器IP>:26001/`（例如 `http://192.168.91.11:26001/` 或任意網段的 `http://<Router-IP>:26001/`）；
- **核心功能看板**：
  - **集群拓撲實時看板**：動態展示當前控制面節點 IP、數據面節點 IP、角色定位（Orchestrator vs Storage）、Universal COW 寫時保護狀態；
  - **實時健康狀態**：常駐 RSS 記憶體佔用（< 15MB）、在線虛擬磁盤總數、開機運行時長；
  - **全格式鏡像矩陣**：即時列出 Raw、Fixed VHD、Dynamic VPC、VHDX、VMDK、VDI、QCOW2 各檔案，顯示格式標籤、`:1081x` NBD 映射端口與 `⚡ In-Tree Rust` 核心狀態；
  - **一鍵遠端調度**：支援網頁端點選「設為默認引導」、「一鍵重新掃描磁碟目錄（`/api/rescan`）」；
  - **MAC 特定綁定**：可為指定客戶端 MAC 地址單獨綁定專屬系統映像（如遊戲機載入 Win11，運維機載入 antiX）；
  - **手選菜單模擬器**：實時預覽終端客戶端將會目擊到的 iPXE 選擇列表，並支援「一鍵複製 Boot URL」。

#### 2. OpenWrt 旁路由（Side Router）實戰部署 SOP（以 192.168.91.11 為例，泛用於任意網段）
在典型家用或企業局域網中，主路由（如 `192.168.91.1` 或 `192.168.1.1`）負責 PPPoE 撥號與 WAN 訪問，而專用旁路由（如 `192.168.91.11`，極路由 X C526A / IPQ40xx 四核 ARMv7，512MB RAM）則作為專用無盤與邊緣算力調度大腦。

##### 步驟一：上傳與常駐運行
將靜態編譯的 `iventoymersed-armv7` 上傳至旁路由（如外接隨身碟 `/mnt/sda1/`）：
```bash
# 本地推送至旁路由
scp dist/iventoymersed-armv7 root@<旁路由IP>:/mnt/sda1/iventoymersed

# 在旁路由上賦予執行權限並一行指令背景啟動
chmod +x /mnt/sda1/iventoymersed
COW_DIR=/tmp/cow VTOY_DIR=/mnt/iso /mnt/sda1/iventoymersed > /var/log/iventoymersed.log 2>&1 &
```

##### 步驟二：配置 Proxy-DHCP（零干擾主路由，免改動主網關）
無須在主路由上更動任何 DHCP 參數，直接在旁路由的 `/etc/dnsmasq.conf` 末尾追加 PXE 代理配置（將 `<旁路由IP>` 與 `<網段>` 替換為實際網絡參數）：
```dnsmasq
# 僅監聽並響應網內 PXE 引導廣播，不干擾主路由發放普通 IP：
dhcp-range=<網段例如192.168.91.0>,proxy
dhcp-match=set:bios,option:client-arch,0
dhcp-boot=tag:bios,undionly.kpxe,,<旁路由IP>
dhcp-match=set:efi64,option:client-arch,7
dhcp-match=set:efi64,option:client-arch,9
dhcp-boot=tag:efi64,ipxe.efi,,<旁路由IP>
```
重啟 dnsmasq 即刻生效：`/etc/init.d/dnsmasq restart`。客戶端開機按下 F12 即自動命中旁路由並拉起選單！

#### 3. 映像檔不儲存於旁路由時的無盤調度架構（只讀說明）
旁路由器本機通常只有微小 Flash（128MB~512MB），絕無法容納 10GB~70GB 的大型 VHDX/VMDK 鏡像。在實際生產中，推薦採用以下兩種**零本地磁盤佔用**的架構：

##### 【架構 A：NFS / CIFS 網路磁區只讀直掛（NAS Storage Backing）】
- **母盤存儲端**：鏡像存儲於大容量 NAS（如遠端 NAS `/mnt/R2D/iso` 或 `/mnt/ax9/`）；
- **旁路由掛載**：旁路由開機時以**物理只讀（`ro`）**模式掛載 NAS 共享：
  ```bash
  mkdir -p /mnt/iso
  # 掛載 NFSv4（推薦，效能極高）
  mount -t nfs 192.168.91.12:/mnt/R2D/iso /mnt/iso -o ro,nolock
  # 或掛載 CIFS/Samba
  # mount -t cifs //192.168.91.175/ax9/iso /mnt/iso -o ro,guest
  ```
- **只讀安全機制**：
  - `iventoymersed` 運行時指定 `VTOY_DIR=/mnt/iso`；
  - 指定 `COW_DIR=/tmp/cow`：`/tmp` 在 OpenWrt 上為記憶體虛擬磁區（tmpfs），客戶端的所有寫入全部攔截在 91.11 的 RAM 暫存中；
  - **91.11 本機 Flash 磁盤佔用為 0 Byte**，客戶端重啟即秒級復原，遠端 NAS 上的黃金母盤 100% 免疫污染。

##### 【架構 B：控制面與數據面解耦引導（Decoupled Orchestrator & Cross-Host Chainload）】
- **控制面（Control Plane / 旁路由）**：僅作為**「全域引導調度大腦」**（跑 Web Deck 控制台、監聽 26001、分發 DHCP Option 66/67 與 iPXE 菜單）；
  - **v3.5.1 原生解耦參數**：在 `config.yaml` 或環境變數中宣告：
    ```yaml
    ROLE: "orchestrator"
    STORAGE_NODE: "192.168.188.20"     # 遠端 NAS 數據節點 IP
    STORAGE_PORT: 26001                # 遠端 NAS 服務埠
    ```
    啟動時亦可直接帶入 CLI 旗標：`./iventoymersed --role orchestrator --storage-node 192.168.188.20`。
  - **Linux 網絡命名空間 (netns) 零衝突隔離**：
    在複雜網絡或測試環境下，旁路由可置於獨立命名空間（如 `ip netns add ns_siderouter`）運行，徹底消除與宿主機的端口衝突（如 `0.0.0.0:26001` vs `192.168.188.11:26001`）與網絡棧污染。
- **數據面（Data Plane / 虛擬 NAS）**：巨型磁盤檔案（VHDX / VMDK / VDI / Raw）與 NBD (10810-10823)、iSCSI (3260)、HTTP 206 串流實體部署在高吞吐算力機（如 188.20 NVMe 或 91.175 萬兆 NAS）；
  - **COW 差分目錄最佳實踐**：強烈推薦將 `COW_DIR` 指定於原生 Linux 檔案系統（如 Btrfs / ext4，路徑如 `/var/tmp/cow`）。若存於 NTFS 磁區，必須以 `prealloc=0,sparse` 掛載，杜絕預設 `prealloc` 引發的稀疏差分檔 `EINVAL` (不適用的引數) 崩潰。
- **跨節點調度**：旁路由發布的動態引導腳本中，將客戶端開機請求直接重定向（Chainload）至真實數據節點：
  ```ipxe
  # 客戶端自旁路由取得選單，但數十 GB 的虛擬磁盤流量直連遠端 NAS 數據節點
  kernel http://192.168.188.20:26001/vtoy/vmlinuz ... nbdroot=192.168.188.20 nbdport=10815 ...
  initrd http://192.168.188.20:26001/vtoy/initramfs.img
  # 或 Windows iSCSI 直連:
  set root-path iscsi:192.168.188.20:::1:iqn.2026-10.lan:vtoy:win8-golden.img
  sanhook --drive 0x80 iscsi:192.168.188.20:::1:iqn.2026-10.lan:vtoy:win8-golden.img
  ```
- **核心優勢**：旁路由本身完全不介入大型磁盤 I/O，**幾十台機器並發開機的幾百 GB 流量直接在核心交換機與 NAS 之間傳輸**，完全不佔用旁路由器的 CPU 與記憶體！經非 91 隔離虛擬網段（`192.168.188.0/24`）實機壓測，antiX IceWM 桌面與 Windows 11 25H2 桌面雙雙通過 `verify-desktop-ocr` 驗收。

---

## 🇺🇸 English Documentation

### Overview & v3.5.1 Breakthroughs

> [!TIP]
> 📖 **Quick Start Guide**: Looking for an illustrated walkthrough on setting up decoupled edge clusters? Check out the 🇺🇸 **[English Step-by-Step Tutorial](docs/TUTORIAL_EN.md)** covering cluster setup, zero-CLI Web control, dynamic MAC bindings, and live Windows 11/antiX Linux verifications!

`iventoymersed` is an ultra-lightweight, high-performance **dual-engine netboot server written in 100% Pure Rust (Tokio + Axum)**. In version **v3.5.1**, it achieves physical decoupling of control and data planes with a zero-hardcoded dynamic IP architecture:

1. **Decoupled Cluster Architecture (Orchestrator vs Storage Node)**:
   - **Orchestrator**: Ultra-lightweight deployment on OpenWrt side-routers (e.g. ARMv7 musl static binary, RSS < 2MB), managing Web UI, client MAC/IP pre-bindings, and dynamic iPXE scripts;
   - **Storage Node**: Deployed on NAS or host workstations, serving 2TB+ virtual disks (Raw / VHD / VHDX / VMDK / QCOW2) over In-Tree COW NBD and iSCSI SANBOOT;
   - **Hot Topology Switching**: Web UI supports switching roles on the fly and custom remote storage node IP/port configuration, or empty for automatic `${next-server}` adaptation.
2. **Client MAC Pre-selection & Dedicated Static IP Overrides**:
   - Allows binding specific client NIC MAC addresses to target OS images with optional dedicated static IP;
   - **Dual-Channel iPXE Injection**: Directly emits `# Static IP override\nset ip <ip>` when MAC is queried, or injects runtime `iseq ${net0/mac} <mac> && set ip <ip> ||` for generic menus;
   - Persisted in `iventoy.json` across reboots.
3. **Zero-Hardcoded IP Dynamic Networking**:
   - Web frontend renders dynamically based on `window.location.host`, never assuming hardcoded LAN subnets;
   - Backend boot scripts dynamically interpolate `${srv}` and `${net_default_server}`.
4. **Non-Blocking Modern Web UI Experience**:
   - Replaces blocking modal `window.alert()` with non-blocking toast notifications (`showToast()`);
   - Upgrades target image input with HTML5 `<input list="...">` dual-mode datalist supporting both local auto-completion and remote manual entry.

### v3.5.0 Core Innovations Overview

1. **In-Tree Pure Rust Dynamic VHDX & COW Engine**:
   - Zero external dependencies: completely eliminates `qemu-nbd`, `kpartx`, and `dmsetup`;
   - Full Microsoft VHDX specification decoder with in-memory BAT (Block Allocation Table) caching (~296 KB RAM footprint for a 75GB disk);
   - Transparent COW (Copy-On-Write) difference overlay (`.diff`), guaranteeing immutable golden master protection and instant reboot recovery;
   - 32-bit ARMv7 (Cortex-A7) arithmetic optimization: full 64-bit integer indexing avoids addressing overflows, verified with Windows 11 25H2 live boot and OCR visual pass.
2. **Pure Rust In-Tree GPT & BCD Auto-Patcher**:
   - Zero Python dependencies: full native binary parser for GPT headers (LBA 1, `EFI PART`) and partition tables (LBA 2);
   - Automatically detects ESP existence and extracts target Windows partition Unique GUIDs;
   - Rewrites Microsoft BCD registry hives directly in memory with mixed-endian GUID replacements (5 Disk refs + 5 Partition refs updated in milliseconds);
   - Built-in CLI subcommands (`inspect`, `patch-bcd`) and dynamic HTTP endpoint (`GET /vtoy/bcd?image=...`).
3. **Multiple Initrd Dynamic Overlay for Linux Zero-Touch Diskless**:
   - Eliminates the need for running `vtoyboot.sh` inside native Linux disk images;
   - Leverages Linux kernel multiple initrd memory unpacking:
     `initrd (loop,gpt2)/boot/initramfs-linux.img (http,${srv}:26001)/vtoy/vtoy_overlay.cpio`
   - Keeps mother disk 100% clean and read-only while injecting universal early-stage network hooks (`initramfs-tools`, `dracut`, `mkinitcpio`).
4. **Mode 2 (NetLoop GRUB HTTP 206 Loopback) Large File Stability**:
   - Production-tested with massive virtual images: **142GB COS (CachyOS) / 75GB Win11 VHDX / 60GB Artix / 8GB antiX**;
   - Case-insensitive HTTP Header normalization for `content-range` and `content-length`, fixing critical GRUB HTTP parser crash bugs;
   - Automatic Fixed VHD detection and 512-byte Conectix trailer stripping;
   - 100% Stateless server design: zero persistent socket overhead, delivering 186 MB/s random byte-range throughput.
5. **Turnkey Support for Lightweight NAS & OpenWrt Routers**:
   - Compact static Musl binaries (1.7 MB ~ 1.9 MB), zero glibc runtime requirements;
   - Tested and supported on **OpenWrt (op)**, Extreme Router X (Gee K2P/HC5961 / Cortex-A7), **TrueNAS**, **CasaOS**, **Unraid**, and Synology/QNAP/UGREEN NAS;
   - Runtime RSS memory footprint < 15MB.

---

### Pure Rust In-Tree GPT & BCD Engine

```bash
# Inspect GPT partition layout on any virtual disk:
./iventoymersed inspect /mnt/disks/win11.vhdx

# Patch BCD hive binary tailored for target disk:
./iventoymersed patch-bcd /mnt/disks/win11.vhdx /tmp/win11_uefi.bcd
```

---

### Multiple Initrd Dynamic Overlay for Linux

```grub
menuentry "Native Linux (Zero-Touch Multiple Initrd Overlay)" {
    loopback loop (http,${srv}:26001)/iso/cachyos.img
    linux (loop,gpt2)/boot/vmlinuz-linux root=/dev/nbd0p2 rw ip=dhcp nbdroot=${srv} nbdport=10810
    initrd (loop,gpt2)/boot/initramfs-linux.img (http,${srv}:26001)/vtoy/vtoy_overlay.cpio
}
```

---

### Lightweight NAS & OpenWrt Edge Deployment

#### 1. Embedded Web GUI Control Deck
`iventoymersed` features a built-in sci-fi dark-themed HTML5 Control Deck:
- **URL**: Open `http://<server-ip>:26001/` in any browser (e.g. `http://192.168.91.11:26001/`);
- **Features**: Live RSS memory usage (< 15MB), multi-format virtual disk monitor, NBD ports (`:1081x`), one-click "Set as Default", directory rescan (`/api/rescan`), MAC-specific image binding, and live iPXE menu preview.

#### 2. OpenWrt Side Router (e.g. 192.168.91.11 / ARMv7) Setup SOP
In a typical network where the main gateway is `192.168.91.1`, the OpenWrt router (`192.168.91.11`) acts as a dedicated Netboot Side Router.
- **Run Daemon**:
  ```bash
  chmod +x /mnt/sda1/iventoymersed
  COW_DIR=/tmp/cow VTOY_DIR=/mnt/iso /mnt/sda1/iventoymersed > /var/log/iventoymersed.log 2>&1 &
  ```
- **Proxy-DHCP Configuration (Zero changes to main router)**:
  Add to `/etc/dnsmasq.conf` on `192.168.91.11`:
  ```dnsmasq
  dhcp-range=192.168.91.0,proxy
  dhcp-match=set:bios,option:client-arch,0
  dhcp-boot=tag:bios,undionly.kpxe,,192.168.91.11
  dhcp-match=set:efi64,option:client-arch,7
  dhcp-match=set:efi64,option:client-arch,9
  dhcp-boot=tag:efi64,ipxe.efi,,192.168.91.11
  ```
  Restart dnsmasq: `/etc/init.d/dnsmasq restart`. Clients pressing F12 boot directly from `192.168.91.11`.

#### 3. Remote Storage Orchestration (0 Byte Local Disk Footprint)
Since embedded routers have tiny flash (128M~512M), images (10G~70G) reside on external NAS/PC:
- **Architecture A (NFS/CIFS Read-Only Mount)**: Mount remote NAS storage read-only at `/mnt/iso` on 91.11. Client writes are trapped in `/tmp/cow` (RAM tmpfs). 0 byte local flash consumed.
- **Architecture B (Control/Data Decoupling & Cross-Host Chainload)**:
  - **Control Plane**: The side router serves exclusively as the Control Plane orchestrator (Web Deck & iPXE dispatch).
  - **v3.5.1 Native Parameters**: Configure in `config.yaml` or environment:
    ```yaml
    ROLE: "orchestrator"
    STORAGE_NODE: "192.168.188.20"     # Remote Storage Node IP
    STORAGE_PORT: 26001                # Remote Storage Node Port
    ```
    Or via CLI: `./iventoymersed --role orchestrator --storage-node 192.168.188.20`.
  - **Linux Network Namespace (`ip netns`) Isolation**: In multi-node testbeds or edge appliances, the orchestrator can run inside an isolated network namespace (`ns_siderouter`), eliminating socket binding conflicts (`0.0.0.0:26001` vs `192.168.188.11:26001`) with host services.
  - **Data Plane (Virtual NAS)**: Massive virtual disk I/O streams directly between client machines and high-throughput storage servers (`192.168.188.20` NBD/iSCSI/HTTP).
  - **COW Filesystem Best Practice**: Strongly recommend hosting `COW_DIR` on native Linux filesystems (Btrfs / ext4, e.g. `/var/tmp/cow`). If placed on NTFS, mount with `prealloc=0,sparse` to prevent kernel `EINVAL` crashes on sparse difference files.
  - **Verification**: Verified under an isolated non-91 subnet (`192.168.188.0/24`) with dual antiX IceWM and Windows 11 25H2 desktop sessions passing OCR validation.

---

### Quick Start

```bash
# Download and untar release
tar -xzf iventoymersed-v3.3.0-linux-x86_64.tar.gz
chmod +x iventoymersed

# Start server
./iventoymersed --config config.yaml
```

---

## 🇯🇵 日本語ドキュメント

### 概要および v3.5.1 の主要な進化点

> [!TIP]
> 📖 **導入ガイド**：傍受ルーターと NAS の分離クラスタ環境をすぐに構築したい方は、🇯🇵 **[日本語詳細チュートリアル](docs/TUTORIAL_JA.md)** をご覧ください。トポロジー解説、Web UI 実機画像、MAC/静的 IP 割り当て、Windows 11 / Linux 実機起動手順を網羅しています！

`iventoymersed` は、Pure Rust（Tokio + Axum）で開発された**デュアルエンジン統合型ネットワークブート（PXE/Netboot）サーバー**です。最新の **v3.5.1** では、制御プレーンとデータプレーンの物理分離とゼロハードコード動的 IP アーキテクチャを確立しました：

1. **制御面とデータ面の分離クラスターアーキテクチャ（Orchestrator vs Storage Node）**：
   - **オーケストレーター (Orchestrator)**：OpenWrt 副ルーター（ARMv7 musl、RSS < 2MB）などの軽量機器に展開。Web 管理画面、クライアント MAC/静的 IP マッピング、動的 iPXE スクリプト生成を担当；
   - **ストレージノード (Storage Node)**：NAS やホストワークステーションに展開。2TB 超の大容量仮想ディスク（Raw / VHD / VHDX / VMDK / QCOW2）を In-Tree COW NBD や iSCSI SANBOOT 経由で提供；
   - **トポロジーの即時切り替え**：Web 管理画面から役割（Orchestrator / Standalone / Storage Node）の切り替えや、リモートストレージノード IP・ポートの指定、または空欄での自動 `${next-server}` 追従が可能。
2. **クライアント MAC 事前バインドおよび専用静的 IP 指定**：
   - 特定のクライアント MAC アドレスに対して起動イメージおよび専用静的 IP を事前割り当て可能；
   - **iPXE デュアルチャネル注入**：MAC クエリ時は直接 `# Static IP override\nset ip <ip>` を注入、汎用メニュー時は `iseq ${net0/mac} <mac> && set ip <ip> ||` を動的評価；
   - 設定は `iventoy.json` に永続化され、再起動後も自動適用。
3. **完全動的ホスト認識（Zero-Hardcoded IP）**：
   - フロントエンド Web 画面は `window.location.host` に基づき動的レンダリング（固定 IP の仮定を完全排除）；
   - バックエンド起動スクリプトは `${srv}` および `${net_default_server}` を自動補完。
4. **ノンブロッキングな最新 Web UI**：
   - ブロッキングな `window.alert()` を非同期トースト通知（`showToast()`）に完全刷新；
   - 対象イメージ選択を HTML5 `<input list="...">` によるデュアルモード化（ローカル自動補完とリモート直接入力の双方に対応）。

### v3.5.0 コア機能の振り返り

1. **Pure Rust 組み込み動的 VHDX 解析および COW 差分エンジン**：
   - `qemu-nbd` や `kpartx` などの外部ホスト依存を完全に排除；
   - マイクロソフト VHDX 仕様の完全デコードと BAT（ブロック割り当てテーブル）のインメモリ展開により、メモリ消費を極小化（75GB VHDX でもわずか約 296KB の RAM 使用量）；
   - COW（コピーオンライト）差分レイヤー（`.diff`）をネイティブサポートし、マスターイメージを完全に保護したまま再起動で初期状態へ復元可能；
   - 32 ビット ARMv7（Cortex-A7）最適化により、アドレス計算のオーバーフローを徹底防止。Windows 11 25H2 の実機および QEMU 双方でのデスクトップ起動・OCR 認識を 100% 達成。
2. **Pure Rust 組み込み GPT 監査および BCD レジストリ Hive 動的パッチエンジン**：
   - Python スクリプトや外部ツール依存を完全排除し、リポジトリを 100% Pure Rust 化；
   - GPT ヘッダー（LBA 1, `EFI PART`）とパーティションテーブル（LBA 2）をバイナリレベルで高速解析し、ESP パーティションの有無と対象 Windows パーティションの Unique GUID をミリ秒単位で検出；
   - BCD レジストリ Hive 内の GUID（混合エンディアン）をメモリ上で正確に直接置換（Disk GUID 5 箇所、Partition GUID 5 箇所）；
   - CLI サブコマンド (`inspect`, `patch-bcd`) および動的 HTTP API (`GET /vtoy/bcd?image=...`) を提供。
3. **Linux Mode 2「複数 Initrd 動的オーバーレイ（Multiple Initrd Dynamic Overlay）」ゼロ侵入ディスクレス**：
   - Linux マスターイメージ内で `vtoyboot.sh` を事前に実行する必要性を完全解消；
   - Linux カーネルの複数 Initrd メモリ自動展開・オーバーレイ機能を活用：
     `initrd (loop,gpt2)/boot/initramfs-linux.img (http,${srv}:26001)/vtoy/vtoy_overlay.cpio`
   - マスターイメージを 100% 変更せず、サーバー側から軽量なネットワークフックを動的注入（`initramfs-tools`, `dracut`, `mkinitcpio` に完全対応）。
4. **Mode 2（NetLoop GRUB HTTP 206 Loopback）大容量ファイルの安定性向上**：
   - **142GB COS (CachyOS) / 75GB Win11 VHDX / 60GB Artix / 8GB antiX** などの超大容量イメージの安定起動を実証；
   - 大文字・小文字を区別しない HTTP ヘッダー（`content-range` / `content-length`）の正常処理により、GRUB HTTP モジュールのパーサークラッシュを解消；
   - 固定 VHD（Fixed VHD）の自動検出と 512 バイト Conectix トレーラーの自動除外により、セクタ境界のズレを根絶；
   - ステートレス設計（サーバー側のセッション維持ゼロ）により、ギガビット LAN で 186 MB/s の高速ランダムストリーミングを実現。
5. **軽量 NAS および OpenWrt ルーターへの即時導入**：
   - 1.7MB〜1.9MB の完全静的 Musl バイナリ（glibc 不要）；
   - **OpenWrt**、極路由 X（HC5961 / ARMv7）、**TrueNAS**、**CasaOS**、**Unraid**、各種市販 NAS でそのまま動作；
   - 常駐メモリ（RSS）15MB 未満で、安価なルーターを高性能ディスクレスサーバーへ変貌させます。

---

### 軽量 NAS および OpenWrt への導入

#### 1. 組み込み Web GUI 管理画面 (Control Deck)
`iventoymersed` は、ブラウザから操作可能なダークテーマの HTML5 Control Deck を内蔵しています：
- **アクセス URL**: ブラウザで `http://<サーバーIP>:26001/` を開きます（例: `http://192.168.91.11:26001/`）；
- **主な機能**: 常駐メモリ使用量（< 15MB）、仮想ディスク（Raw/VHD/VHDX/VMDK/VDI/QCOW2）ステータス一覧、NBD ポート番号（`:1081x`）、デフォルト起動イメージ設定、ディレクトリ再スキャン（`/api/rescan`）、MAC アドレス固有のイメージ紐付け、iPXE メニューシミュレーター。

#### 2. OpenWrt 旁ルーター（Side Router、例: 192.168.91.11）導入手順
メインルーター（`192.168.91.1`）の設定を変更することなく、OpenWrt 旁ルーター（`192.168.91.11`）をディスクレスサーバーとして稼働させます。
- **起動**:
  ```bash
  chmod +x /mnt/sda1/iventoymersed
  COW_DIR=/tmp/cow VTOY_DIR=/mnt/iso /mnt/sda1/iventoymersed > /var/log/iventoymersed.log 2>&1 &
  ```
- **Proxy-DHCP 設定（メインルーター無変更で共存）**:
  `192.168.91.11` の `/etc/dnsmasq.conf` に追加：
  ```dnsmasq
  dhcp-range=192.168.91.0,proxy
  dhcp-match=set:bios,option:client-arch,0
  dhcp-boot=tag:bios,undionly.kpxe,,192.168.91.11
  dhcp-match=set:efi64,option:client-arch,7
  dhcp-match=set:efi64,option:client-arch,9
  dhcp-boot=tag:efi64,ipxe.efi,,192.168.91.11
  ```
  dnsmasq を再起動：`/etc/init.d/dnsmasq restart`。

#### 3. ルーター本体にイメージを保存しないリモート起動方式（読み取り専用）
ルーターの内蔵 Flash は極めて小さいため、大容量イメージ（10GB〜70GB）は外部 NAS や PC に保持します：
- **方式 A（NFS/CIFS 読み取り専用マウント）**: リモート NAS を `/mnt/iso` に `ro` でマウント。クライアントの書き込みはルーターの `/tmp/cow`（RAM tmpfs）で遮断され、ルーター内蔵 Flash の消費は **0 バイト**。
- **方式 B（制御面とデータ面の完全分離・クロスホストチェーンロード）**:
  - **制御面（Control Plane / サイドルーター）**: Web Deck 管理画面と iPXE メニュー配布に専念。
  - **v3.5.1 ネイティブ設定パラメータ**: `config.yaml` または環境変数で簡単に指定可能：
    ```yaml
    ROLE: "orchestrator"
    STORAGE_NODE: "192.168.188.20"     # リモート NAS / ストレージノード IP
    STORAGE_PORT: 26001                # ストレージノードのポート
    ```
    コマンドラインからも直接指定可能：`./iventoymersed --role orchestrator --storage-node 192.168.188.20`。
  - **Linux ネットワーク名前空間 (`ip netns`) 衝突防止技術**: サイドルータープロセスを独立した名前空間（`ns_siderouter`）で起動することで、ホスト側のポートバインド（`0.0.0.0:26001` と `192.168.188.11:26001`）の重複を根本的に排除。
  - **データ面（Data Plane / 仮想 NAS）**: 数十 GB の仮想ディスク通信（VHDX/VMDK/Raw）は、クライアントと高性能ストレージサーバー（`192.168.188.20` NBD/iSCSI/HTTP）間で直接ストリーミングされます。
  - **COW 差分ファイルシステムのベストプラクティス**: `COW_DIR` は Linux ネイティブファイルシステム（Btrfs / ext4、例: `/var/tmp/cow`）上に配置することを強く推奨します。NTFS 上に配置する場合は、スパースファイルの `EINVAL` エラーを防止するため必ず `prealloc=0,sparse` オプションでマウントしてください。
  - **実機検証完了**: 物理環境に影響を与えない非 91 隔離仮想サブネット（`192.168.188.0/24`）にて、antiX IceWM デスクトップおよび Windows 11 25H2 デスクトップ双方の起動・OCR 検証パスを実証済み。

---

### クイックスタート

```bash
tar -xzf iventoymersed-v3.3.0-linux-x86_64.tar.gz
chmod +x iventoymersed
./iventoymersed --config config.yaml
```

---

## 🤝 開源社群協作與反饋 (Community & Feedback)

本專案致力於開拓邊緣計算與網絡無盤引導的技術極限。由於各廠牌主板韌體（UEFI/BIOS）、網卡 PXE ROM 以及網絡環境千差萬別，我們由衷期待來自開源社群朋友的反饋！

- **問題回報與硬體相容性登記**：歡迎前往 [GitHub Issues](https://github.com/obhasashare2024-namo/iventoymersed/issues) 分享您的主板與網卡測試數據；
- **架構探討與功能建言**：歡迎在 [Discussions](https://github.com/obhasashare2024-namo/iventoymersed/discussions) 中與我們交流；
- **貢獻代碼**：熱烈歡迎 Pull Request！

*感謝每一位探索無盤與虛擬化邊界的開源同行者！*
