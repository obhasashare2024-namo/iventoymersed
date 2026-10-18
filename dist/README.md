# iventoymersed (Dual-Engine Unified Netboot Server)

**Merged & Immersed: 雙軌融合 iVentoy/iPXE + NetLoop GRUB 全格式網絡啟動一體機**  
*Pure Rust (Tokio + Axum) · 單一二進制 · 雙引擎分流 · 客戶端自動劃分子系統*

---

## 🚀 項目概述 (Overview)

`iventoymersed`（名稱取自 **iVentoy Merged & Immersed**，象徵雙引擎無縫融合與沈浸式體驗）將兩種網絡引導技術流派徹底融合為**單一二進制程序**（1.7 MB）：

1. **Engine A（iVentoy iPXE 原生塊設備引導）**：
   - 伺服器端負責多端口動態掛載與塊設備導出（QEMU NBD 10810~10819 / tgt iSCSI 3260）。
   - 客戶端直接執行原語級 `sanboot iscsi:${next-server}:::1:...` 或 `nbd://`，客戶端內核負擔極低，**Windows 11 VHDX 強制走此軌道**，保證原生讀寫與 0 RAM 耗損。
2. **Engine B（NetLoop GRUB HTTP 206 循環掛載引導）**：
   - 伺服器端僅需標準 HTTP 206 Byte-Range 串流，零額外背景守護進程，伺服器負載接近零。
   - 客戶端 GRUB 利用 `loopback loop (http,server:26001)/disks/xxx.vhd.vtoy` 於客戶端記憶體掛載磁盤分區，直接抓取內核與 initramfs，**Linux 原生 ISO / VHD 預設走此軌道**。
3. **WinPE ISO 原生 HTTP Range 接管（Engine B 延伸）**：
   - 針對 WinPE ISO，GRUB2 EFI 缺乏主機板 BlockIO 鉤子會導致微軟 bootmgr 報 `0xc000000e` 死鎖；
   - 系統在選單中預設採用 iPXE HTTP Range 206 `sanboot` 接管 WinPE，實測直通進入 2048x2048 桌面。

---

## 🔀 統一作業系統選單機制 (Unified OS Boot Menu)

自 **v3.1.0** 起，徹底取消了首級 5 秒 Dual-Engine 選擇器延遲，直接進入統一作業系統選單（Unified OS Boot Menu），各作業系統依特性自動路由至最佳引擎：

```mermaid
flowchart TD
    Client[客戶端 PXE 開機] --> DHCP[DHCP 獲取 IP]
    DHCP --> M0[統一選單: /boot.ipxe\n(Unified OS Boot Menu)]
    
    M0 -->|Windows 11 VHDX| WinTrack[Engine A: Native iSCSI/iBFT SANBOOT\n(原生可讀寫 / 0 RAM 耗損)]
    M0 -->|Linux VHD / ISO| LinuxTrack[Engine B: NetLoop GRUB Loopback\n(預設 HTTP 206 / 可切換 NBD Hook)]
    M0 -->|WinPE ISO| PETrack[Engine B 接管: iPXE HTTP Range SANBOOT\n(杜絕 0xc000000e / 直通桌面)]
```

### 1. Windows 11 (VHDX) 專屬路由
直接通過 iPXE `sanboot iscsi:${next-server}:::1:${iqn}` 原生掛載 tgt iSCSI Target，建立標準 iBFT 映射表交付 Windows 內核接管，實測可讀寫且不消耗任何客戶端記憶體。

### 2. Linux (VHD / ISO) 雙引擎支援
預設引導 NetLoop GRUB2 核心，透過 HTTP 206 Byte-Range 於記憶體解析 GPT 表並抽取內核與 initramfs；若需完整 NBD 塊設備，亦可在選單中一鍵切換 iVentoy NBD Hook 模式。

### 3. WinPE (ISO) 韌體 BlockIO 支援
鑑於 UEFI 韌體中 GRUB2 chainloader 卸載後缺乏韌體 BlockIO 驅動導致微軟 bootmgr 報 `0xc000000e` 死鎖，系統自動指派 iPXE HTTP Range 206 `sanboot` 接管 WinPE，完美進入桌面。

---

## 📦 部署與執行 (Quick Start)

### 啟動服務
```bash
# 單一二進制直接運行，監聽 26001 端口
./iventoymersed --dir /mnt/disks/vdisks --port 26001
```

### DHCP / TFTP 鏈接配置
在網關 DHCP（如 dnsmasq / ISC DHCP）中配置：
```dnsmasq
dhcp-boot=undionly.kpxe
# iPXE 預設引導 URL 指向 iventoymersed
dhcp-option-force=209,http://192.168.1.29:26001/boot.ipxe
```

---

## 📊 方案對比速查表

| 指標 | Engine A (iVentoy iPXE) | Engine B (NetLoop GRUB) |
| :--- | :--- | :--- |
| **伺服器資源佔用** | 需常駐 qemu-nbd / tgt 進程 | 僅常駐 HTTP 206 靜態服務（極低） |
| **客戶端資源佔用** | 極低（由 BIOS/iPXE INT13 仿真直讀） | 需客戶端 RAM 掛載磁盤分區解析內核 |
| **Windows 支援** | 完美支援（原生 sanboot iSCSI/NBD） | 需 WinPE 二次注入加載（較繁瑣） |
| **Linux 支援** | 支援標準 Dracut/initramfs NBD 掛載 | 完美支援（直接 loopback 內核直起） |

---

## 🤝 誠摯邀請社群測試與寶貴建言 (Community Testing & Call for Feedback)

本專案是在探索網絡啟動與無盤系統邊界過程中的一次開源嘗試。由於網絡引導涉及眾多硬體平台、各家網卡 ROM（Realtek、Intel、Broadcom、Mellanox 等）、不同 UEFI/BIOS 韌體實作以及多樣的作業系統鏡像格式，難免存在未涵蓋的相容性場景或潛在邊界缺陷。

我們懷著無比誠懇與謙遜的心態，熱切期盼各位開發者、系統管理員與無盤技術愛好者能夠在您的實體環境或虛擬化平台中進行嘗試，並賜予我們寶貴的指導與回饋！

---

### 🇨🇳 中文 (Chinese)

> **致親愛的開源社群朋友與網絡引導愛好者：**
> 
> 非常感謝您關注 `iventoymersed` 雙軌融合網絡啟動專案！
> 
> 這是我們嘗試將 **iVentoy / iPXE 塊設備引導** 與 **NetLoop GRUB HTTP 206 循環掛載** 融為一體的探索性成果。我們深知，網絡引導技術的生命力完全來自於多樣化真實硬體與網路環境的檢驗。
> 
> **誠邀各位進行實地測試：**
> - 無論您是在實體無盤工作站、家庭 NAS、企業伺服器，還是 KVM / Proxmox / ESXi 虛擬化環境；
> - 無論您引導的是 Windows VHD、各種 Linux 發行版（Arch、Debian、Ubuntu、Fedora、openSUSE 等），還是各類維護 WinPE / LiveCD；
> - **如果啟動成功**：非常歡迎您在 [Issues](https://github.com/obhasashare2024-namo/iventoymersed/issues) 或 [Discussions](https://github.com/obhasashare2024-namo/iventoymersed/discussions) 中分享您的主板型號、網卡型號與耗時數據，這將極大鼓舞專案的持續改進；
> - **如果遇到任何報錯、選單卡死或異常**：懇請您不吝反饋日誌與截圖，我們定將第一時間跟進分析與修復；
> - **任何架構設計與功能改進建議**：非常歡迎提出您的真知灼見，或直接提交 Pull Request 與我們共同完善！
> 
> 再次感謝大家的寬容、支持與無私付出！

---

### 🇺🇸 English

> **To our dear open-source community members and netboot enthusiasts:**
> 
> Thank you so much for checking out the `iventoymersed` unified netboot project!
> 
> This project represents an exploratory effort to seamlessly bridge **iVentoy / iPXE block-level boot** with **NetLoop GRUB HTTP 206 loopback streaming** into a single, cohesive binary. Because network booting inherently depends on countless permutations of motherboard firmware, NIC PXE ROMs (Realtek, Intel, Mellanox, etc.), UEFI implementations, and kernel initramfs configs, real-world community testing is vital to its maturity.
> 
> **We warmly invite you to test this project in your environment:**
> - Whether on bare-metal diskless rigs, enterprise servers, home labs, or virtualized environments (KVM / QEMU, Proxmox, ESXi, Hyper-V);
> - Whether booting Windows VHDs, diverse Linux distributions (Arch, Debian, Alpine, Fedora, etc.), or rescue WinPE / LiveCD images;
> - **If it works well**: Please consider sharing your hardware specs and boot timing in our [Issues](https://github.com/obhasashare2024-namo/iventoymersed/issues) or [Discussions](https://github.com/obhasashare2024-namo/iventoymersed/discussions). Your success stories provide immense encouragement!
> - **If you encounter errors, hangs, or unexpected behavior**: Please kindly open an issue with your client NIC details and server logs. We will investigate and address them promptly.
> - **Feature suggestions & Architectural feedback**: We warmly welcome all ideas, constructive criticism, and Pull Requests to make this tool more resilient and friendly.
> 
> Thank you wholeheartedly for your time, generosity, and support!

---

### 🇯🇵 日本語 (Japanese)

> **オープンソースコミュニティの皆様、ネットワークブート愛好家の皆様へ：**
> 
> `iventoymersed` プロジェクトに関心をお寄せいただき、誠にありがとうございます！
> 
> 本プロジェクトは、**iVentoy / iPXE によるブロックレベル起動** と **NetLoop GRUB HTTP 206 ループバック配信** という2つの異なる技術アプローチを単一バイナリに統合する試みです。ネットワークブートの世界は、マザーボードのファームウェア、NIC の PXE ROM、UEFI 実装、OS の initramfs など極めて多岐にわたる環境に依存するため、皆様の実際の現場での検証が何よりも不可欠です。
> 
> **皆様の環境での実機テスト・ご意見を心よりお待ちしております：**
> - 実機のディスクレス端末、家庭内サーバー、または仮想環境（KVM / Proxmox / ESXi 等）での動作テスト；
> - Windows VHD、各種 Linux ディストリビューション、WinPE / LiveCD のブート検証；
> - **正常に起動した場合**：ぜひマザーボードや NIC の型番、起動時間などを [Issues](https://github.com/obhasashare2024-namo/iventoymersed/issues) や [Discussions](https://github.com/obhasashare2024-namo/iventoymersed/discussions) で共有していただけますと、開発の大きな励みになります；
> - **起動失敗やハングアップ、不具合が発生した場合**：ログや画面キャプチャを添えてお気軽にご報告ください。迅速に原因究明と修正に努めます；
> - **設計改善や機能追加のご提案**：改善案や Pull Request など、建設的なフィードバックをいつでも歓迎いたします！
> 
> 皆様の温かいご支援とご協力に、心より深く感謝申し上げます。

