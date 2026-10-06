# iventoymersed (Dual-Engine Unified Netboot Server)

**Merged & Immersed: 雙軌融合 iVentoy/iPXE + NetLoop GRUB 全格式網絡啟動一體機**  
*Pure Rust (Tokio + Axum) · 單一二進制 · 雙引擎分流 · 客戶端自動劃分子系統*

---

## 🚀 項目概述 (Overview)

`iventoymersed`（名稱取自 **iVentoy Merged & Immersed**，象徵雙引擎無縫融合與沈浸式體驗）將先前的兩種網絡引導技術流派徹底融合為**單一二進制程序**（1.7 MB）：

1. **Engine A（iVentoy iPXE 輕量網絡引導）**：
   - 伺服器端負責多端口動態掛載與塊設備導出（QEMU NBD 10810~10819 / tgt iSCSI 3260）。
   - 客戶端直接執行原語級 `sanboot --drive 0x80` 或 `nbd://`，客戶端內核負擔極低，適合 Windows 全虛擬磁盤（VHD/VHDX）與輕量無盤工作站。
2. **Engine B（NetLoop GRUB 深度循環掛載引導）**：
   - 伺服器端僅需標準 HTTP 206 Byte-Range 串流，零額外背景守護進程，伺服器負載接近零。
   - 客戶端 GRUB 利用 `loopback loop (http,server:26001)/disks/xxx.vhd.vtoy` 於客戶端記憶體掛載磁盤分區，直接抓取內核與 initramfs，適合多發行版 Linux 深度安裝與救援。

---

## ⏱️ 開機時序與精確時間點實測 (Timing & Screenshots)

在 QEMU / KVM 實機與虛擬網卡（virtio-net-pci）測試中，各階段耗時與前端選單渲染節點精確記錄如下：

| 時間點 (s) | 系統階段 | 客戶端行為 / 畫面表現 | 產物截圖對應 |
| :--- | :--- | :--- | :--- |
| **0.0s** | Power On | BIOS/UEFI POST 執行，加載 iPXE ROM | - |
| **2.5s ~ 4.5s** | DHCP & Link | 網卡鏈路就緒，DHCP 獲取 IP (`10.0.2.15`) | - |
| **5.0s** | Fetch Boot Menu | 客戶端向 `:26001/boot.ipxe` 發起 GET 請求 | - |
| **6.6s 精確點** | **首級雙軌選單彈出** | 螢幕顯示 `iventoymersed Dual-Engine Selector`，帶 5 秒倒數計時 `(5)`，預設選中 Engine A | `dual_engine_selector_prompt.png` |
| **7.2s** | 按鍵交互 / 切換 | 按下方向鍵下，高亮切換為 `[Engine B] NetLoop GRUB Loopback Engine` | `dual_engine_highlight_b.png` |
| **選定瞬間** | **自動劃分子系統** | 客戶端根據選擇自動 chain 對應端點，完成前端界面自動劃分 | `dual_engine_partitioned_a.png` |

---

## 🔀 前端自動劃分機制 (Automatic Subsystem Partitioning)

在首級雙軌選單做出選擇後，系統採用 **iPXE Dynamic Chain-loading** 實現前端子界面的自動劃分與交接，無須客戶端重啟或重新 DHCP：

```mermaid
flowchart TD
    Client[客戶端 PXE 開機] --> DHCP[DHCP 獲取 IP]
    DHCP --> M0[首級選單: /boot.ipxe\n(倒數 5 秒自動引導)]
    
    M0 -->|選擇 Engine A 或超時| AutoA[自動劃分進入: /ipxe/menu-a]
    M0 -->|選擇 Engine B| AutoB[自動劃分進入: /grub/core.0]
    
    subgraph Engine A: iVentoy iPXE 子系統
        AutoA --> MA[Engine A 專屬子選單\n(動態列出 VHD/VHDX/RAW/ISO)]
        MA --> BootNBD[NBD / iSCSI 原語級 sanboot 啟動]
    end
    
    subgraph Engine B: NetLoop GRUB 子系統
        AutoB --> MB[GRUB netboot 核心加載]
        MB --> CFG[動態讀取 /boot/grub/grub.cfg]
        CFG --> LoopHTTP[HTTP 206 Byte-Range 深度分區循環掛載]
    end
```

### 1. 選擇 Engine A (iVentoy iPXE)
客戶端收到指令 `chain http://${next-server}:26001/ipxe/menu-a`。
iPXE 即刻清除首級選單，並在相同終端控制台繪製 **Engine A 二級子選單**：
- 自動嗅探的虛擬磁盤列表（如 `0001COSext4.vhd.vtoy`、`vdisk_win10.vhd` 等）。
- 底部附帶返回首級選單按鍵 `[99] Return to Dual-Engine Selector`。

### 2. 選擇 Engine B (NetLoop GRUB)
客戶端收到指令 `chain http://${next-server}:26001/grub/core.0`（或 UEFI 模式 `grubnet.efi`）。
iPXE 將機器控制權無縫移交給 GRUB 網絡啟動核心，GRUB 隨後接管螢幕並繪製全功能 GRUB2 藍底界面，執行分區循環掛載。

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

