# Release Notes: iventoymersed v3.5.0

**全格式虛擬磁盤純 Rust 原生解析 · 全操作系統矩陣實測跑通 · 零外部依賴單一靜態二進制**

---

### 🌟 v3.5.0 重大更新亮點 (Highlights)

在 v3.5.0 中，`iventoymersed` 迎來了虛擬存儲底層與多操作系統相容性的歷史性飛躍，全面達成**全虛擬磁盤格式支援**與**多 OS 實體驗證通關**：

#### 1. 100% 純 Rust 全格式虛擬磁盤解析引擎 (Pure Rust Multi-Format Virtual Disk Engine)
不再依賴宿主機的 `qemu-nbd`、`kpartx` 或 `nbdkit`，伺服器單二進制程式直接在記憶體中解構各大主流虛擬化格式的二進制元數據與塊映射表：
- **Raw / ISO (.img, .raw, .iso, .vtoy)**：零開銷直接映射；
- **Fixed VHD (.vhd)**：自動偵測尾部 512B `conectix` 頁腳並安全剝離，無縫直通原始扇區；
- **Dynamic VHD / VPC (.vhd)**：原生解析 512B `cxsparse` 動態頭部、區塊大小（預設 2MB）與 BAT 塊分配表，精確定位數據扇區與未分配全零塊；
- **Dynamic VHDX (.vhdx)**：原生解析微軟 VHDX 規範頭部、區域表（Region Table）與二級 BAT，支援 1MB~64MB 自訂 Block Size；
- **VMware VMDK (.vmdk monolithicSparse)**：原生解碼 512B Sparse Header、Grain Directory (GD) 與 Grain Table (GT) 兩級表結構；
- **VirtualBox VDI (.vdi dynamic)**：原生解析 VDI 0xBEDA107F 頭部、1MB 塊大小與 BAT 一級表，精準識別未分配與顯式零塊；
- **QEMU QCOW2 (.qcow2 v2 / v3)**：原生解析 `QFI\xfb` Big-Endian 規範、Cluster Bits、兩級 L1/L2 表、標準 Cluster 掩碼與 Zero Cluster 標誌。

#### 2. 全格式無差別支援 COW (寫時複製) 差分保護層
所有上述格式（Raw、VHD、VHDX、VMDK、VDI、QCOW2）皆無縫接入內建的 `CowStorage` 引擎：
- 母盤永遠處於物理唯讀（`O_RDONLY`）保護狀態，即使母盤遭多台客戶端並行存取亦絕不損壞；
- 客戶端的寫入請求全部即時截獲並記錄於獨立的稀疏 `.diff` 檔案與記憶體點陣圖（Bitmask）中；
- 客戶端重啟或重置時，母盤毫秒級復原，達成網吧、機房與實驗室環境的「黃金母盤零污染」。

#### 3. 多操作系統矩陣實體引導跑通 (Multi-OS Boot Verification Matrix)
全量操作系統在 QEMU/KVM 與 NBD 模式下完成開機測試與實體驗收：
1. **Desktop Linux (antiX Linux)**：
   - 經 VMDK 格式引導，UEFI OVMF 模式啟動；
   - 通過 `verify-desktop-ocr` 驗收：Conky 性能儀表、IceWM 桌面與 Runit 服務就緒。
2. **BSD 家族 (OpenBSD 7.7 Minimal Sparse)**：
   - 經 `miniroot77.img`（5.6MB）稀疏 img、QCOW2 與 Dynamic VHD 啟動；
   - 內核完成硬體枚舉，順利掛載內存盤根分區 `rd0a` 並進入安裝控制台。
3. **固件/路由器 Linux (OpenWrt 23.05 x86_64)**：
   - 經 `openwrt.qcow2` 引導，3 秒內完成內核解包；
   - `mount_root` 成功掛載 `/dev/root`，`procd` 初始化完成，`br-lan` 網卡就緒。
4. **Windows 生態系 (Windows 8 / 10 / 11 PE & VHDX)**：
   - 經純 Rust 原生 GPT / BCD 注入引擎動態重寫 16 位元組 Disk/Partition GUID，實現零侵入無盤秒啟。

---

### 📦 下載與二進制校驗 (Downloads & Checksums)

全版本均基於 Musl Libc 100% 靜態鏈接編譯，無任何 Glibc 版本依賴：

| 架構 / 檔案名稱 | 說明 | SHA256 校驗碼 |
| :--- | :--- | :--- |
| `iventoymersed-v3.5.0-linux-x86_64.tar.gz` | x86_64 現代 PC / 伺服器 | `88853610fe073cf77eeb8fa17999d146c8843ec9df9ca931968bfbba3b9351de` |
| `iventoymersed-v3.5.0-linux-armv7.tar.gz` | 32-bit ARM 路由器 (MT7981 / BCM) | `e43216623aec19859ee2fd0132c282e951899549cd061c1decbd594fb2e11e9b` |
| `iventoymersed-v3.5.0-linux-aarch64.tar.gz` | 64-bit ARM 軟路由 / 樹莓派 | `fadb7be631e15a95b1c8358962d4bea1e2d6a1e97c768fb675558c5d1e3a43c1` |
