use crate::config::{BootMode, Config};
use crate::image::{detect_disk_format, has_partition_table, is_windows_image, DiskFormat};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};

pub fn sanitize_ipxe_key(key: &str) -> String {
    let clean: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.is_empty() {
        "item".into()
    } else {
        clean
    }
}

pub fn sanitize_ipxe_text(text: &str) -> String {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if c == '$' { '_' } else { c })
        .collect();
    clean.trim().to_string()
}

#[derive(Clone, Serialize, Debug)]
pub struct TargetInfo {
    pub id: String,
    pub file: String,
    pub path: String,
    pub tid: u32,
    pub iqn: String,
    pub nbd_port: u16,
    #[serde(skip)]
    pub proc_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alive: Option<bool>,
    pub has_partitions: bool,
    pub root_dev: String,
    pub format: String,
    pub is_windows: bool,
}

pub struct StorageManager {
    pub config: Arc<RwLock<Config>>,
    targets: Mutex<HashMap<String, Target>>,
    used_tids: Mutex<HashSet<u32>>,
    used_ports: Mutex<HashSet<u16>>,
}

struct Target {
    info: TargetInfo,
    child: Option<Child>,
}

fn log(msg: &str) {
    println!("[storage] {}", msg);
}

async fn run_cmd(args: &[&str]) {
    let _ = Command::new(args[0]).args(&args[1..]).output().await;
}

impl StorageManager {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            targets: Mutex::new(HashMap::new()),
            used_tids: Mutex::new(HashSet::new()),
            used_ports: Mutex::new(HashSet::new()),
        }
    }

    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }

    pub async fn set_default_win_mode(&self, mode: &str) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        cfg.json_cfg.windows_boot.default_mode = mode.to_string();
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn set_default_image(&self, img: Option<String>) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        cfg.json_cfg.default_image = img;
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn set_mac_binding(&self, mac: String, img: String) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        let clean_mac = mac.trim().to_ascii_lowercase().replace('-', ":");
        cfg.json_cfg.mac_bindings.insert(clean_mac, img);
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn remove_mac_binding(&self, mac: &str) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        let clean_mac = mac.trim().to_ascii_lowercase().replace('-', ":");
        cfg.json_cfg.mac_bindings.remove(&clean_mac);
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn alloc_tid(&self) -> u32 {
        let base = self.config.read().await.tgt_base_tid;
        let mut used = self.used_tids.lock().await;
        let mut tid = base;
        while used.contains(&tid) {
            tid += 1;
        }
        used.insert(tid);
        tid
    }

    pub async fn alloc_port(&self) -> u16 {
        let base = self.config.read().await.nbd_base_port;
        let mut used = self.used_ports.lock().await;
        let mut p = base;
        while used.contains(&p) {
            p += 1;
        }
        used.insert(p);
        p
    }

    async fn export_iscsi(&self, tid: u32, iqn: &str, path: &str) {
        log(&format!("Exporting iSCSI: TID={} IQN={} Backing={}", tid, iqn, path));
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "new", "--mode", "target",
            "--tid", &tid.to_string(), "-T", iqn,
        ])
        .await;
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "new", "--mode", "logicalunit",
            "--tid", &tid.to_string(), "--lun", "1", "-b", path,
        ])
        .await;
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "bind", "--mode", "target",
            "--tid", &tid.to_string(), "-I", "ALL",
        ])
        .await;
    }

    async fn export_nbd(&self, port: u16, path: &str, fmt: DiskFormat) -> Option<Child> {
        let fmt_arg = fmt.as_qemu_arg();
        log(&format!("Exporting NBD on port {}: Backing={} (format={})", port, path, fmt_arg));
        let mut cmd = Command::new("qemu-nbd");
        cmd.arg("-t")
            .arg("-e")
            .arg("16")
            .arg("-p")
            .arg(port.to_string())
            .arg("-x")
            .arg("vtoy")
            .arg("-f")
            .arg(fmt_arg);
        cmd.arg(path);

        match cmd.spawn() {
            Ok(child) => {
                log(&format!("Spawned qemu-nbd PID={:?} on port {}", child.id(), port));
                Some(child)
            }
            Err(e) => {
                eprintln!("[ERR] Failed to spawn qemu-nbd on port {}: {}", port, e);
                None
            }
        }
    }

    async fn unexport_iscsi(&self, tid: u32) {
        log(&format!("Unexporting iSCSI TID={}", tid));
        let t = tid.to_string();
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "unbind", "--mode", "target",
            "--tid", &t, "-I", "ALL",
        ])
        .await;
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "delete", "--mode", "logicalunit",
            "--tid", &t, "--lun", "1",
        ])
        .await;
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "delete", "--mode", "target",
            "--tid", &t,
        ])
        .await;
    }

    async fn unexport_one(&self, t: &mut Target, boot_mode: BootMode) {
        if boot_mode == BootMode::Iscsi || t.info.is_windows {
            self.unexport_iscsi(t.info.tid).await;
        }
        if let Some(mut c) = t.child.take() {
            let _ = c.kill().await;
            let _ = c.wait().await;
        }
    }

    pub async fn shutdown_all(&self) {
        let boot_mode = self.config.read().await.boot_mode;
        let mut map = self.targets.lock().await;
        for (_, t) in map.iter_mut() {
            self.unexport_one(t, boot_mode).await;
        }
        map.clear();
    }

    /// Re-scan VTOY_DIR; exports new images, respawns dead qemu-nbd, unexports vanished ones.
    pub async fn scan_and_export(&self) {
        let (vtoy_dir, iqn_prefix, boot_mode, json_cfg) = {
            let cfg = self.config.read().await;
            (cfg.vtoy_dir.clone(), cfg.iqn_prefix.clone(), cfg.boot_mode, cfg.json_cfg.clone())
        };

        if !tokio::fs::metadata(&vtoy_dir).await.is_ok() {
            if let Err(e) = tokio::fs::create_dir_all(&vtoy_dir).await {
                eprintln!("[ERR] Could not create {}: {}", vtoy_dir, e);
                return;
            }
        }
        let mut files: Vec<String> = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(&vtoy_dir).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".vtoy")
                    || lower.ends_with(".img")
                    || lower.ends_with(".raw")
                    || lower.ends_with(".qcow2")
                    || lower.ends_with(".vhd")
                    || lower.ends_with(".vhdx")
                    || lower.ends_with(".vmdk")
                    || lower.ends_with(".iso")
                {
                    files.push(name);
                }
            }
        }
        files.sort();

        let mut active_ids: HashSet<String> = HashSet::new();

        for filename in files {
            let vtoy_id = filename.clone();
            active_ids.insert(vtoy_id.clone());

            let mut map = self.targets.lock().await;
            if !map.contains_key(&vtoy_id) {
                let fullpath = format!("{}/{}", vtoy_dir, filename);
                let tid = self.alloc_tid().await;
                let port = self.alloc_port().await;
                let iqn = format!("{}:{}", iqn_prefix, sanitize_ipxe_key(&vtoy_id));
                let partitioned = has_partition_table(&fullpath).await;
                
                // Allow manual override from iventoy.json images config
                let mut root_dev = if partitioned {
                    if filename.contains("anitix") {
                        "/dev/nbd0p3".to_string()
                    } else {
                        "/dev/nbd0p2".to_string()
                    }
                } else {
                    "/dev/nbd0".to_string()
                };

                for ctrl in &json_cfg.control {
                    if let (Some(f), Some(r)) = (ctrl.get("file"), ctrl.get("root_dev")) {
                        if f.as_str() == Some(&filename) {
                            if let Some(r_str) = r.as_str() {
                                root_dev = r_str.to_string();
                                println!("[config] Overriding root_dev for {} -> {}", filename, root_dev);
                            }
                        }
                    }
                }

                let fmt = detect_disk_format(&fullpath).await;
                let is_win = is_windows_image(&filename);

                let info = TargetInfo {
                    id: vtoy_id.clone(),
                    file: filename.clone(),
                    path: fullpath.clone(),
                    tid,
                    iqn: iqn.clone(),
                    nbd_port: port,
                    proc_pid: None,
                    alive: None,
                    has_partitions: partitioned,
                    root_dev,
                    format: fmt.as_qemu_arg().to_string(),
                    is_windows: is_win,
                };

                let mut child = None;
                if is_win || boot_mode == BootMode::Iscsi {
                    self.export_iscsi(tid, &iqn, &fullpath).await;
                }
                if !is_win || boot_mode == BootMode::Nbd {
                    child = self.export_nbd(port, &fullpath, fmt).await;
                }

                map.insert(vtoy_id.clone(), Target { info, child });
            } else {
                let t = map.get_mut(&vtoy_id).unwrap();
                if boot_mode == BootMode::Nbd || (!t.info.is_windows && t.child.is_some()) {
                    let need_respawn = match t.child.as_mut() {
                        Some(c) => c.try_wait().ok().flatten().is_some(),
                        None => true,
                    };
                    if need_respawn {
                        let port = t.info.nbd_port;
                        log(&format!("qemu-nbd died, respawning on port {}", port));
                        let path = t.info.path.clone();
                        let fmt = detect_disk_format(&path).await;
                        t.child = self.export_nbd(port, &path, fmt).await;
                    }
                }
            }
        }

        let mut map = self.targets.lock().await;
        let dead_keys: Vec<String> = map
            .keys()
            .filter(|k| !active_ids.contains(*k))
            .cloned()
            .collect();
        for k in dead_keys {
            if let Some(mut t) = map.remove(&k) {
                log(&format!("Unexporting vanished image: {}", k));
                self.unexport_one(&mut t, boot_mode).await;
            }
        }
    }

    pub async fn health_targets(&self) -> Vec<TargetInfo> {
        let boot_mode = self.config.read().await.boot_mode;
        let mut map = self.targets.lock().await;
        let mut out = Vec::new();
        for (_, t) in map.iter_mut() {
            let mut info = t.info.clone();
            if boot_mode == BootMode::Nbd || t.child.is_some() {
                info.alive = Some(match t.child.as_mut() {
                    Some(c) => c.try_wait().ok().flatten().is_none(),
                    None => false,
                });
                info.proc_pid = t.child.as_ref().and_then(|c| c.id());
            } else {
                info.alive = Some(true);
            }
            out.push(info);
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Builds 100% standalone, independent iPXE menu with Ventoy-parity On-Screen Submenus
    pub async fn build_standalone_ipxe_menu(&self, client_mac: Option<&str>) -> String {
        let snapshot = self.health_targets().await;
        let cfg = self.config.read().await.clone();
        let win_cfg = &cfg.json_cfg.windows_boot;

        let mut script = String::new();

        script.push_str("#!ipxe\n\n");
        script.push_str("# Styling & Palette (Ventoy Cyber Theme)\n");
        script.push_str("cpair --foreground 7 --background 0 0 ||\n");
        script.push_str("cpair --foreground 6 --background 0 1 ||\n");
        script.push_str("cpair --foreground 3 --background 0 2 ||\n\n");

        script.push_str("set menu-timeout 15000\n");
        script.push_str("set submenu-timeout 15000\n\n");

        // Determine default item based on MAC binding -> global default -> first item
        let clean_mac = client_mac.map(|m| m.trim().to_ascii_lowercase().replace('-', ":"));
        let mac_matched_image = clean_mac.as_ref().and_then(|m| cfg.json_cfg.mac_bindings.get(m));

        let default_item = if let Some(target_file) = mac_matched_image {
            // Find target by file name
            snapshot.iter().find(|i| &i.file == target_file)
                .map(|i| {
                    if i.is_windows {
                        format!("win_menu_{}", sanitize_ipxe_key(&i.id))
                    } else if i.file.to_ascii_lowercase().ends_with(".iso") {
                        format!("iso_{}", sanitize_ipxe_key(&i.id))
                    } else {
                        format!("vdisk_{}", sanitize_ipxe_key(&i.id))
                    }
                })
        } else if let Some(ref def_img) = cfg.json_cfg.default_image {
            snapshot.iter().find(|i| &i.file == def_img)
                .map(|i| {
                    if i.is_windows {
                        format!("win_menu_{}", sanitize_ipxe_key(&i.id))
                    } else if i.file.to_ascii_lowercase().ends_with(".iso") {
                        format!("iso_{}", sanitize_ipxe_key(&i.id))
                    } else {
                        format!("vdisk_{}", sanitize_ipxe_key(&i.id))
                    }
                })
        } else {
            None
        };

        let final_default = default_item.unwrap_or_else(|| {
            snapshot.iter().find(|i| i.is_windows)
                .map(|i| format!("win_menu_{}", sanitize_ipxe_key(&i.id)))
                .or_else(|| snapshot.first().map(|i| format!("vdisk_{}", sanitize_ipxe_key(&i.id))))
                .unwrap_or_else(|| "shell".into())
        });

        script.push_str(&format!("isset ${{menu-default}} || set menu-default {}\n\n", final_default));

        script.push_str(":start\n");
        script.push_str("menu iVentoy-Rust Network Boot Menu (Arrow keys to navigate, Enter to boot)\n\n");

        let mut win_items = Vec::new();
        let mut vdisk_items = Vec::new();
        let mut iso_items = Vec::new();
        let mut submenus = Vec::new();
        let mut handlers = Vec::new();

        for info in &snapshot {
            let safe_id = sanitize_ipxe_key(&info.id);
            let label = sanitize_ipxe_text(&info.file);

            if info.is_windows {
                // Windows Ventoy-style Submenu Item on Main Menu
                let menu_key = format!("win_menu_{}", safe_id);
                win_items.push(format!("item {} [WIN] {} (Press Enter for Boot Modes)", menu_key, label));

                let iscsi_key = format!("win_iscsi_{}", safe_id);
                let vtoy_key = format!("win_vtoy_{}", safe_id);
                let preferred_win_key = if win_cfg.default_mode == "vtoyboot" { &vtoy_key } else { &iscsi_key };

                // Build Ventoy-style Submenu block
                let mut sm = String::new();
                sm.push_str(&format!(":{}\n", menu_key));
                sm.push_str(&format!("menu Select Boot Mode for {}:\n", label));
                sm.push_str(&format!("item {} [1] Native iSCSI SANBOOT (Fastest, zero driver change)\n", iscsi_key));
                sm.push_str(&format!("item {} [2] Ventoy vtoyboot NBD Hook (Virtual disk driver hook)\n", vtoy_key));
                sm.push_str("item start               [3] <-- Back to Main Menu\n\n");
                sm.push_str(&format!("choose --timeout ${{submenu-timeout}} --default {} win_choice_{} || goto start\n", preferred_win_key, safe_id));
                sm.push_str(&format!("goto ${{win_choice_{}}}\n", safe_id));
                submenus.push(sm);

                // Submenu Handler: iSCSI
                let iqn = sanitize_ipxe_text(&info.iqn);
                handlers.push(format!(
                    ":{}\necho Booting {} via Native iSCSI Target ({}) ...\nset root-path iscsi:${{next-server}}::::{}\nsanboot iscsi:${{next-server}}::::{} || goto boot_failed\n",
                    iscsi_key, label, iqn, iqn, iqn
                ));

                // Submenu Handler: vtoyboot Hook
                handlers.push(format!(
                    ":{}\necho Booting {} via Ventoy NBD Hook (Port {}) ...\nkernel {} initrd=initramfs.img console=ttyS0 console=tty0 ip=dhcp nbdroot=${{next-server}} nbdport={} nbdmaxpart=16 root={} rw nosplash verbose\ninitrd {}\nboot || goto boot_failed\n",
                    vtoy_key, label, info.nbd_port, cfg.vtoy_kernel_url, info.nbd_port, info.root_dev, cfg.vtoy_initrd_url
                ));
            } else if info.file.to_ascii_lowercase().ends_with(".iso") {
                let item_key = format!("iso_{}", safe_id);
                iso_items.push(format!("item {} [ISO] {}", item_key, label));
                handlers.push(format!(
                    ":{}\necho Booting {} via HTTP Direct sanboot ...\nsanboot http://${{next-server}}:{}/iso/{} || goto boot_failed\n",
                    item_key, label, cfg.bridge_port, info.file
                ));
            } else {
                let item_key = format!("vdisk_{}", safe_id);
                vdisk_items.push(format!("item {} [{}] {}", item_key, info.format.to_uppercase(), label));

                handlers.push(format!(
                    ":{}\necho Booting {} via NBD (Port {}, Root {}) ...\nkernel {} initrd=initramfs.img console=ttyS0 console=tty0 ip=dhcp nbdroot=${{next-server}} nbdport={} nbdmaxpart=16 root={} rw nosplash verbose\ninitrd {}\nboot || goto boot_failed\n",
                    item_key, label, info.nbd_port, info.root_dev, cfg.vtoy_kernel_url, info.nbd_port, info.root_dev, cfg.vtoy_initrd_url
                ));
            }
        }

        if !win_items.is_empty() {
            script.push_str("item --gap -- ==================== Windows Systems ====================\n");
            for it in win_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        if !vdisk_items.is_empty() {
            script.push_str("\nitem --gap -- ==================== Linux & Virtual Disks ==============\n");
            for it in vdisk_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        if !iso_items.is_empty() {
            script.push_str("\nitem --gap -- ==================== ISO Direct Install =================\n");
            for it in iso_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        script.push_str("\nitem --gap -- ==================== Operations & Tools =================\n");
        script.push_str("item shell  Drop into iPXE Shell\n");
        script.push_str("item reboot Reboot Computer\n");
        script.push_str("item exit   Exit to Local BIOS/UEFI\n\n");

        script.push_str("choose --timeout ${menu-timeout} --default ${menu-default} selected || goto shell\n");
        script.push_str("goto ${selected}\n\n");

        script.push_str(":shell\nshell\n\n:reboot\nreboot\n\n:exit\nexit\n\n");
        script.push_str(":boot_failed\necho Boot failed or cancelled! Dropping to shell in 5 seconds...\nsleep 5\ngoto shell\n\n");

        // Submenus
        for sm in submenus {
            script.push_str(&sm);
            script.push_str("\n");
        }

        // Action Handlers
        for h in handlers {
            script.push_str(&h);
            script.push_str("\n");
        }

        script
    }

    /// Builds the top-level dual-engine selector menu with configurable timeout
    pub async fn build_top_level_ipxe_menu(&self, _client_mac: Option<&str>) -> String {
        let mut script = String::new();
        script.push_str("#!ipxe\n\n");
        script.push_str("# Styling & Palette (Ventoy Cyber Theme)\n");
        script.push_str("cpair --foreground 7 --background 0 0 ||\n");
        script.push_str("cpair --foreground 6 --background 0 1 ||\n");
        script.push_str("cpair --foreground 3 --background 0 2 ||\n\n");

        script.push_str("set menu-timeout 5000\n");
        script.push_str("isset ${menu-default} || set menu-default engine_track_a\n\n");

        script.push_str(":start\n");
        script.push_str("menu iventoymersed Dual-Engine Selector (Auto-booting in 5s)\n\n");
        script.push_str("item --gap -- ================= Select Boot Engine =================\n");
        script.push_str("item engine_track_a [Engine A] iVentoy-iPXE NBD Hook Mode (Fast Client, Low RAM)\n");
        script.push_str("item engine_track_b [Engine B] Pure NetLoop HTTP 206 Mode (Zero Server State)\n\n");
        script.push_str("item --gap -- ================= Operations & Tools =================\n");
        script.push_str("item shell  Drop into iPXE Shell\n");
        script.push_str("item reboot Reboot Computer\n");
        script.push_str("item exit   Exit to Local BIOS/UEFI\n\n");

        script.push_str("choose --timeout ${menu-timeout} --default ${menu-default} selected || goto engine_track_a\n");
        script.push_str("goto ${selected}\n\n");

        script.push_str(":engine_track_a\n");
        script.push_str("echo Loading Engine A (iVentoy-iPXE Subsystem)...\n");
        script.push_str("chain http://${next-server}:26001/ipxe/menu-a || goto boot_failed\n\n");

        script.push_str(":engine_track_b\n");
        script.push_str("echo Loading Engine B (Pure NetLoop GRUB Subsystem)...\n");
        script.push_str("chain http://${next-server}:26001/grub/core.0 || goto boot_failed\n\n");

        script.push_str(":shell\nshell\n\n:reboot\nreboot\n\n:exit\nexit\n\n");
        script.push_str(":boot_failed\necho Boot failed or cancelled! Dropping to shell in 5 seconds...\nsleep 5\ngoto shell\n\n");
        script
    }

    /// Builds dynamic GRUB configuration for Engine B (Pure NetLoop HTTP 206)
    pub async fn build_netloop_grub_cfg(&self) -> String {
        let snapshot = self.health_targets().await;
        let mut cfg = String::new();
        cfg.push_str("set timeout=3
");
        cfg.push_str(r#"if [ -z "$net_default_server" ]; then set net_default_server=10.0.2.2; fi"#);
        cfg.push_str("
");
        cfg.push_str(r#"set default=">> [Pure NetLoop] CachyOS 142GB Ext4 (0001COSext4.vhd.vtoy) <<""#);
        cfg.push_str("

");
        cfg.push_str("set menu_color_normal=white/black
set menu_color_highlight=black/cyan

");
        cfg.push_str("terminal_output vga_text console serial

");
        for item in snapshot {
            if item.file.ends_with(".iso") {
                cfg.push_str(&format!(
                    "menuentry \">> [NetLoop ISO] {} <<\" {{\n    insmod http\n    insmod loopback\n    insmod iso9660\n    loopback loop (http,${{net_default_server}}:26001)/iso/{}\n    boot\n}}\n\n",
                    item.file, item.file
                ));
            } else if item.file.contains("COS") {
                cfg.push_str(&format!(
                    "menuentry \">> [Pure NetLoop] CachyOS 142GB Ext4 ({}) <<\" {{\n    insmod http\n    insmod loopback\n    insmod part_gpt\n    insmod ext2\n    insmod linux\n    echo \"[1/4] Establishing HTTP 206 Loopback to {}...\"\n    loopback loop (http,${{net_default_server}}:26001)/iso/{}\n    echo \"[2/4] Parsing internal GPT table and extracting CachyOS kernel...\"\n    linux (loop,gpt2)/boot/vmlinuz-linux-cachyos console=ttyS0 console=tty0 root=UUID=f80dc16b-e328-40f4-8db8-f30faf11a445 ro verbose\n    echo \"[3/4] Extracting CachyOS native initramfs from loopback block device...\"\n    initrd (loop,gpt2)/boot/initramfs-linux-cachyos.img\n    echo \"[4/4] Executing kernel and launching CachyOS...\"\n    boot\n}}\n\n",
                    item.file, item.file, item.file
                ));
            } else if item.file.contains("anitix") {
                cfg.push_str(&format!(
                    "menuentry \">> [Pure NetLoop] antiX Linux 8GB Btrfs ({}) <<\" {{\n    insmod http\n    insmod loopback\n    insmod part_gpt\n    insmod btrfs\n    insmod linux\n    echo \"[1/4] Establishing HTTP 206 Loopback to {}...\"\n    loopback loop (http,${{net_default_server}}:26001)/iso/{}\n    echo \"[2/4] Parsing GPT and extracting antiX kernel...\"\n    linux (loop,gpt3)/boot/vmlinuz-6.6.119-antix.1-amd64-smp console=ttyS0 console=tty0 root=UUID=d1b65f8f-9d38-4e06-a902-8ff9240921c5 ro verbose\n    echo \"[3/4] Extracting antiX native initramfs...\"\n    initrd (loop,gpt3)/boot/initrd.img-6.6.119-antix.1-amd64-smp\n    echo \"[4/4] Executing kernel...\"\n    boot\n}}\n\n",
                    item.file, item.file, item.file
                ));
            } else {
                cfg.push_str(&format!(
                    "menuentry \">> [NetLoop VDisk] {} (Port {}) <<\" {{\n    insmod http\n    insmod loopback\n    insmod part_gpt\n    loopback loop (http,${{net_default_server}}:26001)/iso/{}\n    boot\n}}\n\n",
                    item.file, item.nbd_port, item.file
                ));
            }
        }
        cfg
    }
}
