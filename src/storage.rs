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

pub fn sanitize_iscsi_iqn_key(key: &str) -> String {
    let clean: String = key
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
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
    pub backend_type: String,
    #[serde(skip)]
    pub proc_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alive: Option<bool>,
    pub has_partitions: bool,
    pub root_dev: String,
    pub format: String,
    pub is_windows: bool,
}

pub enum NbdBackend {
    Embedded(crate::nbd_server::EmbeddedNbdServer),
    External(Child),
}

pub struct StorageManager {
    pub config: Arc<RwLock<Config>>,
    targets: Mutex<HashMap<String, Target>>,
    used_tids: Mutex<HashSet<u32>>,
    used_ports: Mutex<HashSet<u16>>,
}

struct Target {
    info: TargetInfo,
    backend: Option<NbdBackend>,
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

    pub async fn set_cluster_config(
        &self,
        role: String,
        storage_node: Option<String>,
        storage_port: Option<u16>,
    ) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        let role_clean = role.trim().to_ascii_lowercase();
        cfg.role = role_clean.clone();
        cfg.storage_node = storage_node.clone();
        cfg.storage_port = storage_port;
        cfg.json_cfg.role = Some(role_clean);
        cfg.json_cfg.storage_node = storage_node;
        cfg.json_cfg.storage_port = storage_port;
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn set_menu_timeout(&self, timeout_secs: u32) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        cfg.json_cfg.menu_timeout = Some(timeout_secs);
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn set_mac_binding(
        &self,
        mac: String,
        img: String,
        ip: Option<String>,
    ) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        let clean_mac = mac.trim().to_ascii_lowercase().replace('-', ":");
        cfg.json_cfg.mac_bindings.insert(clean_mac.clone(), img);
        if let Some(clean_ip) = ip.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
            cfg.json_cfg.mac_ips.insert(clean_mac, clean_ip);
        } else {
            cfg.json_cfg.mac_ips.remove(&clean_mac);
        }
        crate::config::save_iventoy_json(&cfg.vtoy_dir, &cfg.json_cfg)
    }

    pub async fn remove_mac_binding(&self, mac: &str) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        let clean_mac = mac.trim().to_ascii_lowercase().replace('-', ":");
        cfg.json_cfg.mac_bindings.remove(&clean_mac);
        cfg.json_cfg.mac_ips.remove(&clean_mac);
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
        log(&format!(
            "Exporting iSCSI: TID={} IQN={} Backing={}",
            tid, iqn, path
        ));
        run_cmd(&[
            "tgtadm",
            "--lld",
            "iscsi",
            "--op",
            "new",
            "--mode",
            "target",
            "--tid",
            &tid.to_string(),
            "-T",
            iqn,
        ])
        .await;
        run_cmd(&[
            "tgtadm",
            "--lld",
            "iscsi",
            "--op",
            "new",
            "--mode",
            "logicalunit",
            "--tid",
            &tid.to_string(),
            "--lun",
            "1",
            "-b",
            path,
        ])
        .await;
        run_cmd(&[
            "tgtadm",
            "--lld",
            "iscsi",
            "--op",
            "bind",
            "--mode",
            "target",
            "--tid",
            &tid.to_string(),
            "-I",
            "ALL",
        ])
        .await;
    }

    async fn export_qemu_nbd(&self, port: u16, path: &str, fmt: DiskFormat) -> Option<Child> {
        let fmt_arg = fmt.as_qemu_arg();
        log(&format!(
            "Exporting via qemu-nbd on port {}: Backing={} (format={})",
            port, path, fmt_arg
        ));
        let mut cmd = Command::new("qemu-nbd");
        cmd.arg("-t")
            .arg("-e")
            .arg("16")
            .arg("-p")
            .arg(port.to_string())
            .arg("-x")
            .arg("vtoy")
            .arg("--cache=writeback")
            .arg("--aio=threads")
            .arg("--discard=unmap")
            .arg("-f")
            .arg(fmt_arg);
        cmd.arg(path);

        match cmd.spawn() {
            Ok(child) => {
                log(&format!(
                    "Spawned qemu-nbd PID={:?} on port {}",
                    child.id(),
                    port
                ));
                Some(child)
            }
            Err(e) => {
                eprintln!("[ERR] Failed to spawn qemu-nbd on port {}: {}", port, e);
                None
            }
        }
    }

    async fn export_nbd(
        &self,
        port: u16,
        path: &str,
        fmt: DiskFormat,
    ) -> (Option<NbdBackend>, String) {
        if fmt == DiskFormat::Raw
            || fmt == DiskFormat::Iso
            || fmt == DiskFormat::Vhdx
            || fmt == DiskFormat::Vmdk
            || fmt == DiskFormat::Vdi
            || fmt == DiskFormat::Qcow2
            || fmt == DiskFormat::Vpc
        {
            let diff_opt = std::env::var("COW_DIR").ok().map(|dir| {
                let p = std::path::Path::new(path);
                let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("disk");
                let clean_name = fname
                    .replace(".img.vtoy", "")
                    .replace(".vtoy", "")
                    .replace(".img", "")
                    .replace(".raw", "")
                    .replace(".vhdx", "")
                    .replace(".vhd", "")
                    .replace(".vmdk", "")
                    .replace(".vdi", "")
                    .replace(".qcow2", "");
                format!("{}/{}.diff", dir, clean_name)
            });

            if let Some(ref diff_path) = diff_opt {
                log(&format!("Exporting via In-Tree Pure Rust NBD Server (COW .diff) on port {}: Backing={}, Diff={}", port, path, diff_path));
                match crate::nbd_server::EmbeddedNbdServer::start_cow(port, path, diff_path, "vtoy")
                    .await
                {
                    Ok(srv) => (
                        Some(NbdBackend::Embedded(srv)),
                        "in-tree-rust-nbd-cow".to_string(),
                    ),
                    Err(e) => {
                        eprintln!("[ERR] Failed to start In-Tree NBD server with COW on port {}: {}. Falling back to direct raw.", port, e);
                        match crate::nbd_server::EmbeddedNbdServer::start(port, path, "vtoy").await
                        {
                            Ok(srv) => (
                                Some(NbdBackend::Embedded(srv)),
                                "in-tree-rust-nbd".to_string(),
                            ),
                            Err(e2) => {
                                eprintln!("[ERR] Direct start also failed: {}.", e2);
                                (None, "failed".to_string())
                            }
                        }
                    }
                }
            } else {
                log(&format!(
                    "Exporting via In-Tree Pure Rust NBD Server on port {}: Backing={}",
                    port, path
                ));
                match crate::nbd_server::EmbeddedNbdServer::start(port, path, "vtoy").await {
                    Ok(srv) => (
                        Some(NbdBackend::Embedded(srv)),
                        "in-tree-rust-nbd".to_string(),
                    ),
                    Err(e) => {
                        eprintln!("[ERR] Failed to start In-Tree NBD server on port {}: {}. Falling back to qemu-nbd.", port, e);
                        let c = self.export_qemu_nbd(port, path, fmt).await;
                        (c.map(NbdBackend::External), "qemu-nbd".to_string())
                    }
                }
            }
        } else {
            let c = self.export_qemu_nbd(port, path, fmt).await;
            (c.map(NbdBackend::External), "qemu-nbd".to_string())
        }
    }

    async fn unexport_iscsi(&self, tid: u32) {
        log(&format!("Unexporting iSCSI TID={}", tid));
        let t = tid.to_string();
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "unbind", "--mode", "target", "--tid", &t, "-I",
            "ALL",
        ])
        .await;
        run_cmd(&[
            "tgtadm",
            "--lld",
            "iscsi",
            "--op",
            "delete",
            "--mode",
            "logicalunit",
            "--tid",
            &t,
            "--lun",
            "1",
        ])
        .await;
        run_cmd(&[
            "tgtadm", "--lld", "iscsi", "--op", "delete", "--mode", "target", "--tid", &t,
        ])
        .await;
    }

    async fn unexport_one(&self, t: &mut Target, boot_mode: BootMode) {
        if boot_mode == BootMode::Iscsi || t.info.is_windows {
            self.unexport_iscsi(t.info.tid).await;
        }
        if let Some(backend) = t.backend.take() {
            match backend {
                NbdBackend::Embedded(mut srv) => srv.shutdown(),
                NbdBackend::External(mut c) => {
                    let _ = c.kill().await;
                    let _ = c.wait().await;
                }
            }
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
        let (vtoy_dir, iqn_prefix, boot_mode, json_cfg, role, storage_node, storage_port) = {
            let cfg = self.config.read().await;
            (
                cfg.vtoy_dir.clone(),
                cfg.iqn_prefix.clone(),
                cfg.boot_mode,
                cfg.json_cfg.clone(),
                cfg.role.clone(),
                cfg.storage_node.clone(),
                cfg.storage_port.unwrap_or(26001),
            )
        };

        // Orchestrator syncs targets from remote Storage Node
        if role == "orchestrator" {
            if let Some(ref sn) = storage_node {
                let url = format!("http://{}:{}/api/status", sn, storage_port);
                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_millis(1500))
                    .build()
                {
                    Ok(c) => c,
                    Err(_) => reqwest::Client::new(),
                };
                if let Ok(resp) = client.get(&url).send().await {
                    if let Ok(txt) = resp.text().await {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&txt) {
                            if let Some(targets_obj) = val.get("targets").and_then(|v| v.as_object()) {
                            let mut map = self.targets.lock().await;
                            map.clear();
                            for (id, t) in targets_obj {
                                let file = t
                                    .get("file")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(id)
                                    .to_string();
                                let path = t
                                    .get("path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let iqn = t
                                    .get("iqn")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let nbd_port = t
                                    .get("nbd_port")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(10810) as u16;
                                let backend = t
                                    .get("backend")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("remote-nbd")
                                    .to_string();
                                let root_dev = t
                                    .get("root_dev")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let format = t
                                    .get("format")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("raw")
                                    .to_string();
                                let is_windows =
                                    t.get("is_windows").and_then(|v| v.as_bool()).unwrap_or(false);
                                let has_partitions = t
                                    .get("has_partitions")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(true);
                                let alive = t.get("alive").and_then(|v| v.as_bool());

                                let info = TargetInfo {
                                    id: id.clone(),
                                    file,
                                    path,
                                    tid: 100,
                                    iqn,
                                    nbd_port,
                                    backend_type: backend,
                                    proc_pid: None,
                                    alive,
                                    has_partitions,
                                    root_dev,
                                    format,
                                    is_windows,
                                };
                                map.insert(id.clone(), Target { info, backend: None });
                            }
                            return;
                        }
                    }
                }
            }
        }
    }

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
                    || lower.ends_with(".vdi")
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
                let iqn = format!("{}:{}", iqn_prefix, sanitize_iscsi_iqn_key(&vtoy_id));
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
                                println!(
                                    "[config] Overriding root_dev for {} -> {}",
                                    filename, root_dev
                                );
                            }
                        }
                    }
                }

                let fmt = detect_disk_format(&fullpath).await;
                let is_win = is_windows_image(&filename);

                if is_win {
                    if let Ok(Some(gpt)) = crate::bcd::inspect_disk_gpt(&fullpath) {
                        log(&format!(
                            "[gpt] Windows image '{}': Disk GUID={}, ESP present={}",
                            filename,
                            gpt.disk_guid.to_string(),
                            gpt.has_esp
                        ));
                        if !gpt.has_esp {
                            if let Some(ref w_part) = gpt.windows_partition {
                                log(&format!("[gpt] Target lacks ESP partition. Windows Part GUID={}. Dynamic BCD generator ready.", w_part.unique_guid.to_string()));
                            }
                        }
                    }
                }

                let (backend, b_type) =
                    if !is_win || fmt == DiskFormat::Vhdx || boot_mode == BootMode::Nbd {
                        self.export_nbd(port, &fullpath, fmt).await
                    } else {
                        (None, "iscsi-ibft".to_string())
                    };

                let info = TargetInfo {
                    id: vtoy_id.clone(),
                    file: filename.clone(),
                    path: fullpath.clone(),
                    tid,
                    iqn: iqn.clone(),
                    nbd_port: port,
                    backend_type: b_type,
                    proc_pid: None,
                    alive: None,
                    has_partitions: partitioned,
                    root_dev,
                    format: fmt.as_qemu_arg().to_string(),
                    is_windows: is_win,
                };

                if (is_win || boot_mode == BootMode::Iscsi) && fmt != DiskFormat::Vhdx {
                    self.export_iscsi(tid, &iqn, &fullpath).await;
                }

                map.insert(vtoy_id.clone(), Target { info, backend });
            } else {
                let t = map.get_mut(&vtoy_id).unwrap();
                if boot_mode == BootMode::Nbd || t.backend.is_some() {
                    let need_respawn = match t.backend.as_mut() {
                        Some(NbdBackend::Embedded(_)) => false,
                        Some(NbdBackend::External(c)) => c.try_wait().ok().flatten().is_some(),
                        None => true,
                    };
                    if need_respawn {
                        let port = t.info.nbd_port;
                        log(&format!(
                            "External qemu-nbd died, respawning on port {}",
                            port
                        ));
                        let path = t.info.path.clone();
                        let fmt = detect_disk_format(&path).await;
                        let (b, bt) = self.export_nbd(port, &path, fmt).await;
                        t.backend = b;
                        t.info.backend_type = bt;
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
            if let Some(backend) = t.backend.as_mut() {
                match backend {
                    NbdBackend::Embedded(_) => {
                        info.alive = Some(true);
                        info.proc_pid = None;
                    }
                    NbdBackend::External(c) => {
                        info.alive = Some(c.try_wait().ok().flatten().is_none());
                        info.proc_pid = c.id();
                    }
                }
            } else if boot_mode == BootMode::Iscsi || info.is_windows {
                info.alive = Some(true);
            } else {
                info.alive = Some(false);
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

        let storage_node = cfg
            .storage_node
            .clone()
            .unwrap_or_else(|| "${next-server}".to_string());
        let storage_http_port = cfg.storage_port.unwrap_or(cfg.bridge_port);

        let mut script = String::new();

        script.push_str("#!ipxe\n\n");
        script.push_str("# Styling & Palette (Ventoy Cyber Theme)\n");
        script.push_str("cpair --foreground 7 --background 0 0 ||\n");
        script.push_str("cpair --foreground 6 --background 0 1 ||\n");
        script.push_str("cpair --foreground 3 --background 0 2 ||\n\n");

        let timeout_secs = cfg.json_cfg.menu_timeout.unwrap_or(15);
        let timeout_ms = timeout_secs * 1000;
        script.push_str(&format!("set menu-timeout {}\n", timeout_ms));
        script.push_str(&format!("set submenu-timeout {}\n\n", timeout_ms));

        // Determine default item based on MAC binding -> global default -> first item
        let clean_mac = client_mac.map(|m| m.trim().to_ascii_lowercase().replace('-', ":"));
        if let Some(ref mac) = clean_mac {
            if let Some(ip) = cfg.json_cfg.mac_ips.get(mac) {
                script.push_str(&format!(
                    "# Static IP binding configured for MAC {}\nset ip {}\n\n",
                    mac, ip
                ));
            }
        }

        let mac_matched_image = clean_mac
            .as_ref()
            .and_then(|m| cfg.json_cfg.mac_bindings.get(m));

        let default_item = if let Some(target_file) = mac_matched_image {
            // Find target by file name
            snapshot.iter().find(|i| &i.file == target_file).map(|i| {
                if i.is_windows {
                    format!("win_menu_{}", sanitize_ipxe_key(&i.id))
                } else if i.file.to_ascii_lowercase().ends_with(".iso") {
                    format!("iso_{}", sanitize_ipxe_key(&i.id))
                } else {
                    format!("vdisk_{}", sanitize_ipxe_key(&i.id))
                }
            })
        } else if let Some(ref def_img) = cfg.json_cfg.default_image {
            snapshot.iter().find(|i| &i.file == def_img).map(|i| {
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
            snapshot
                .iter()
                .find(|i| i.is_windows)
                .map(|i| format!("win_menu_{}", sanitize_ipxe_key(&i.id)))
                .or_else(|| {
                    snapshot
                        .first()
                        .map(|i| format!("vdisk_{}", sanitize_ipxe_key(&i.id)))
                })
                .unwrap_or_else(|| "shell".into())
        });

        script.push_str(&format!("set menu-default {}\n\n", final_default));

        script.push_str(":start\n");
        script.push_str(
            "menu iVentoy-Rust Network Boot Menu (Arrow keys to navigate, Enter to boot)\n\n",
        );

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
                win_items.push(format!(
                    "item {} [WIN] {} (Press Enter for Boot Modes)",
                    menu_key, label
                ));

                let iscsi_key = format!("win_iscsi_{}", safe_id);
                let http_key = format!("win_http_{}", safe_id);
                let preferred_win_key = if win_cfg.default_mode == "http" {
                    &http_key
                } else {
                    &iscsi_key
                };

                // Build Ventoy-style Submenu block
                let mut sm = String::new();
                sm.push_str(&format!(":{}\n", menu_key));
                sm.push_str(&format!("menu Select Boot Mode for {}:\n", label));
                sm.push_str(&format!(
                    "item {} [1] Native iSCSI / iBFT SANBOOT (Fastest, zero driver change)\n",
                    iscsi_key
                ));
                sm.push_str(&format!(
                    "item {} [2] HTTP Range Direct SANBOOT (HTTP 206 protocol)\n",
                    http_key
                ));
                sm.push_str("item start               [3] <-- Back to Main Menu\n\n");
                sm.push_str(&format!("choose --timeout ${{submenu-timeout}} --default {} win_choice_{} || goto start\n", preferred_win_key, safe_id));
                sm.push_str(&format!("goto ${{win_choice_{}}}\n", safe_id));
                submenus.push(sm);

                // Submenu Handler 1: Native iSCSI / iBFT SANBOOT
                let iqn = sanitize_ipxe_text(&info.iqn);
                handlers.push(format!(
                    ":{}\necho Booting {} via Native iSCSI Target ({}) ...\nset root-path iscsi:{}:::1:{}\nsanboot iscsi:{}:::1:{} || goto boot_failed\n",
                    iscsi_key, label, iqn, storage_node, iqn, storage_node, iqn
                ));

                // Submenu Handler 2: HTTP Range Direct SANBOOT
                handlers.push(format!(
                    ":{}\necho Booting {} via HTTP Range Direct SANBOOT ...\nsanboot http://{}:{}/iso/{} || goto boot_failed\n",
                    http_key, label, storage_node, storage_http_port, info.file
                ));
            } else if info.file.to_ascii_lowercase().ends_with(".iso") {
                let item_key = format!("iso_{}", safe_id);
                iso_items.push(format!("item {} [ISO] {}", item_key, label));
                handlers.push(format!(
                    ":{}\necho Booting {} via HTTP Direct sanboot ...\nsanboot http://{}:{}/iso/{} || goto boot_failed\n",
                    item_key, label, storage_node, storage_http_port, info.file
                ));
            } else {
                let item_key = format!("vdisk_{}", safe_id);
                vdisk_items.push(format!(
                    "item {} [{}] {}",
                    item_key,
                    info.format.to_uppercase(),
                    label
                ));

                handlers.push(format!(
                    ":{}\necho Booting {} via Pure Rust NBD (Port {}, Root {}) ...\nkernel {} initrd=initramfs.img console=ttyS0 console=tty0 ip=dhcp nbdroot={} nbdport={} nbdmaxpart=16 root={} rw nosplash verbose\ninitrd {}\nboot || goto boot_failed\n",
                    item_key, label, info.nbd_port, info.root_dev, cfg.vtoy_kernel_url, storage_node, info.nbd_port, info.root_dev, cfg.vtoy_initrd_url
                ));
            }
        }

        if !win_items.is_empty() {
            script.push_str(
                "item --gap -- ==================== Windows Systems ====================\n",
            );
            for it in win_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        if !vdisk_items.is_empty() {
            script.push_str(
                "\nitem --gap -- ==================== Linux & Virtual Disks ==============\n",
            );
            for it in vdisk_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        if !iso_items.is_empty() {
            script.push_str(
                "\nitem --gap -- ==================== ISO Direct Install =================\n",
            );
            for it in iso_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        script.push_str(
            "\nitem --gap -- ==================== Operations & Tools =================\n",
        );
        script.push_str("item shell  Drop into iPXE Shell\n");
        script.push_str("item reboot Reboot Computer\n");
        script.push_str("item exit   Exit to Local BIOS/UEFI\n\n");

        script.push_str(
            "choose --timeout ${menu-timeout} --default ${menu-default} selected || goto shell\n",
        );
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

    /// Builds the top-level unified OS boot menu (eliminating dual-engine selector delay)
    pub async fn build_top_level_ipxe_menu(&self, client_mac: Option<&str>) -> String {
        let cfg = self.config.read().await;
        let port = cfg.bridge_port;
        let storage_port = cfg.storage_port.unwrap_or(port);
        let srv_default = if let Some(ref sn) = cfg.storage_node {
            sn.clone()
        } else {
            "${next-server}".to_string()
        };
        let snapshot = self.health_targets().await;
        let mut script = String::new();
        script.push_str("#!ipxe\n\n");
        script.push_str("# Styling & Palette (Ventoy Cyber Theme)\n");
        script.push_str("cpair --foreground 7 --background 0 0 ||\n");
        script.push_str("cpair --foreground 6 --background 0 1 ||\n");
        script.push_str("cpair --foreground 3 --background 0 2 ||\n\n");

        let timeout_secs = cfg.json_cfg.menu_timeout.unwrap_or(10);
        let timeout_ms = timeout_secs * 1000;
        script.push_str(&format!("set menu-timeout {}\n", timeout_ms));
        script.push_str(&format!("isset ${{srv}} || set srv {}\n", srv_default));

        let clean_mac = client_mac.map(|m| m.trim().to_ascii_lowercase().replace('-', ":"));

        // Static IP binding logic
        if let Some(ref mac) = clean_mac {
            if let Some(ip) = cfg.json_cfg.mac_ips.get(mac) {
                script.push_str(&format!(
                    "# Static IP binding configured for client MAC {}\nset ip {}\n\n",
                    mac, ip
                ));
            }
        } else if !cfg.json_cfg.mac_ips.is_empty() {
            script.push_str("# Dynamic Client Static IP overrides\n");
            for (mac, ip) in &cfg.json_cfg.mac_ips {
                script.push_str(&format!(
                    "iseq ${{net0/mac}} {} && set ip {} ||\n",
                    mac.to_ascii_lowercase(),
                    ip
                ));
            }
            script.push_str("\n");
        }

        // Determine default item
        let mut preferred_default = "shell".to_string();
        if let Some(ref mac) = clean_mac {
            if let Some(target_file) = cfg.json_cfg.mac_bindings.get(mac) {
                if let Some(item) = snapshot.iter().find(|i| &i.file == target_file || &i.id == target_file) {
                    let safe_id = sanitize_ipxe_key(&item.id);
                    preferred_default = if item.is_windows {
                        format!("win_iscsi_{}", safe_id)
                    } else if item.file.to_ascii_lowercase().ends_with(".iso") {
                        format!("iso_iscsi_{}", safe_id)
                    } else {
                        format!("vdisk_{}", safe_id)
                    };
                }
            }
        }
        if preferred_default == "shell" {
            if let Some(ref def_img) = cfg.json_cfg.default_image {
                if let Some(item) = snapshot.iter().find(|i| &i.file == def_img || &i.id == def_img) {
                    let safe_id = sanitize_ipxe_key(&item.id);
                    preferred_default = if item.is_windows {
                        format!("win_iscsi_{}", safe_id)
                    } else if item.file.to_ascii_lowercase().ends_with(".iso") {
                        format!("iso_iscsi_{}", safe_id)
                    } else {
                        format!("vdisk_{}", safe_id)
                    };
                }
            }
        }
        if preferred_default == "shell" {
            if let Some(first_win) = snapshot.iter().find(|i| i.is_windows) {
                preferred_default = format!("win_iscsi_{}", sanitize_ipxe_key(&first_win.id));
            } else if let Some(first) = snapshot.first() {
                preferred_default = format!("vdisk_{}", sanitize_ipxe_key(&first.id));
            }
        }

        script.push_str(&format!(
            "isset ${{menu-default}} || set menu-default {}\n\n",
            preferred_default
        ));

        // MAC-based pre-selection overrides (retaining timeout countdown and manual selection freedom)
        if !cfg.json_cfg.mac_bindings.is_empty() {
            script.push_str("# MAC-based Pre-Selection Overrides (With Countdown & Manual Override)\n");
            for (mac, target) in &cfg.json_cfg.mac_bindings {
                let target_key = if let Some(item) = snapshot.iter().find(|i| &i.file == target || &i.id == target) {
                    let safe_id = sanitize_ipxe_key(&item.id);
                    if item.is_windows {
                        format!("win_iscsi_{}", safe_id)
                    } else if item.file.to_ascii_lowercase().ends_with(".iso") {
                        format!("iso_iscsi_{}", safe_id)
                    } else {
                        format!("vdisk_{}", safe_id)
                    }
                } else {
                    sanitize_ipxe_key(target)
                };
                script.push_str(&format!(
                    "iseq ${{net0/mac}} {} && set menu-default {} ||\n",
                    mac.to_ascii_lowercase(),
                    target_key
                ));
            }
            script.push_str("\n");
        }

        script.push_str(":start\n");
        script.push_str("menu iventoymersed Unified OS Boot Menu\n\n");

        let mut win_items = Vec::new();
        let mut linux_items = Vec::new();
        let mut iso_items = Vec::new();
        let mut handlers = Vec::new();

        for item in &snapshot {
            let safe_id = sanitize_ipxe_key(&item.id);
            let label = sanitize_ipxe_text(&item.file);
            let iqn = sanitize_ipxe_text(&item.iqn);

            if item.is_windows {
                let key = format!("win_iscsi_{}", safe_id);
                let key_http = format!("win_http_{}", safe_id);
                win_items.push(format!(
                    "item {} [WIN-iSCSI] {} [Path 1: ARMv7 tgt iSCSI]",
                    key, label
                ));
                win_items.push(format!(
                    "item {} [WIN-HTTP] {} [Path 2: HTTP Range SANBOOT]",
                    key_http, label
                ));
                handlers.push(format!(
                    ":{}\necho Booting {} via Path 1 (Native iSCSI/iBFT SANBOOT)...\nset root-path iscsi:${{srv}}:::1:{}\nsanboot iscsi:${{srv}}:::1:{} || goto boot_failed\n",
                    key, label, iqn, iqn
                ));
                handlers.push(format!(
                    ":{}\necho Booting {} via Path 2 (HTTP Range SANBOOT)...\nsanboot http://${{srv}}:{}/iso/{} || goto boot_failed\n",
                    key_http, label, storage_port, item.file
                ));
                handlers.push(format!(
                    ":{}\ngoto {}\n",
                    sanitize_ipxe_key(&item.file),
                    key
                ));
                handlers.push(format!(":{}\ngoto {}\n", safe_id, key));
            } else if item.file.to_ascii_lowercase().ends_with(".iso") {
                let key_a = format!("iso_http_{}", safe_id);
                let key_iscsi = format!("iso_iscsi_{}", safe_id);
                let key_b = format!("iso_grub_{}", safe_id);
                iso_items.push(format!(
                    "item {} [ISO-iSCSI] {} [Path 1: ARMv7 tgt iSCSI]",
                    key_iscsi, label
                ));
                iso_items.push(format!(
                    "item {} [ISO-HTTP] {} [Path 2: HTTP Range SANBOOT]",
                    key_a, label
                ));
                iso_items.push(format!(
                    "item {} [ISO-GRUB] {} [Scheme B: NetLoop GRUB]",
                    key_b, label
                ));

                handlers.push(format!(
                    ":{}\necho Booting {} via Path 1 (ARMv7 tgt iSCSI Target)...\nset root-path iscsi:${{srv}}:::1:{}\nsanboot iscsi:${{srv}}:::1:{} || goto boot_failed\n",
                    key_iscsi, label, iqn, iqn
                ));
                handlers.push(format!(
                    ":{}\necho Booting {} via Path 2 (iPXE HTTP Range SANBOOT)...\nsanboot http://${{srv}}:{}/iso/{} || goto boot_failed\n",
                    key_a, label, storage_port, item.file
                ));
                handlers.push(format!(
                    ":{}\necho Handing off {} to Scheme B (NetLoop GRUB Subsystem)...\niseq ${{platform}} efi && chain http://${{srv}}:{}/grub/grubnet.efi || chain http://${{srv}}:{}/grub/core.0 || goto boot_failed\n",
                    key_b, label, storage_port, storage_port
                ));
                handlers.push(format!(
                    ":{}\ngoto {}\n",
                    sanitize_ipxe_key(&item.file),
                    key_iscsi
                ));
                handlers.push(format!(":{}\ngoto {}\n", safe_id, key_iscsi));
            } else {
                let key_b = format!("vdisk_{}", safe_id);
                let key_a = format!("vdisk_nbd_{}", safe_id);
                let key_iscsi = format!("vdisk_iscsi_{}", safe_id);
                linux_items.push(format!(
                    "item {} [{}] {} (Default: Scheme B NetLoop GRUB)",
                    key_b,
                    item.format.to_uppercase(),
                    label
                ));
                linux_items.push(format!(
                    "item {} [{}] {} [Scheme A: iVentoy NBD Hook]",
                    key_a,
                    item.format.to_uppercase(),
                    label
                ));
                linux_items.push(format!(
                    "item {} [{}] {} [Scheme A: ARMv7 tgt iSCSI]",
                    key_iscsi,
                    item.format.to_uppercase(),
                    label
                ));

                handlers.push(format!(
                    ":{}\necho Handing off {} to Scheme B (NetLoop GRUB Subsystem)...\niseq ${{platform}} efi && chain http://${{srv}}:{}/grub/grubnet.efi || chain http://${{srv}}:{}/grub/core.0 || goto boot_failed\n",
                    key_b, label, storage_port, storage_port
                ));
                handlers.push(format!(
                    ":{}\necho Booting {} via Scheme A (iVentoy NBD Hook, Port {})...\nkernel http://${{srv}}:{}/vtoy/vmlinuz initrd=initramfs.img console=ttyS0 console=tty0 ip=dhcp nbdroot=${{srv}} nbdport={} nbdmaxpart=16 root={} rw nosplash verbose\ninitrd http://${{srv}}:{}/vtoy/initramfs.img\nboot || goto boot_failed\n",
                    key_a, label, item.nbd_port, storage_port, item.nbd_port, item.root_dev, storage_port
                ));
                handlers.push(format!(
                    ":{}\necho Booting {} via Scheme A (ARMv7 tgt iSCSI Target)...\nset root-path iscsi:${{srv}}:::1:{}\nsanboot iscsi:${{srv}}:::1:{} || goto boot_failed\n",
                    key_iscsi, label, iqn, iqn
                ));
                handlers.push(format!(
                    ":{}\ngoto {}\n",
                    sanitize_ipxe_key(&item.file),
                    key_b
                ));
                handlers.push(format!(":{}\ngoto {}\n", safe_id, key_b));
            }
        }

        for target in cfg.json_cfg.mac_bindings.values() {
            let safe_target = sanitize_ipxe_key(target);
            if !snapshot.iter().any(|i| {
                sanitize_ipxe_key(&i.file) == safe_target || sanitize_ipxe_key(&i.id) == safe_target
            }) {
                handlers.push(format!(
                    ":{}\necho Target image '{}' is not currently exported!\ngoto boot_failed\n",
                    safe_target, target
                ));
            }
        }

        if !win_items.is_empty() {
            script.push_str("item --gap -- ==================== Windows Systems (Scheme A Only) ====================\n");
            for it in win_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        if !linux_items.is_empty() {
            script.push_str("\nitem --gap -- ==================== Linux Systems (Default Scheme B) ====================\n");
            for it in linux_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        if !iso_items.is_empty() {
            script.push_str(
                "\nitem --gap -- ==================== ISO / WinPE Systems ====================\n",
            );
            for it in iso_items {
                script.push_str(&format!("{}\n", it));
            }
        }

        script.push_str(
            "\nitem --gap -- ==================== Operations & Tools =================\n",
        );
        script.push_str("item shell  Drop into iPXE Shell\n");
        script.push_str("item reboot Reboot Computer\n");
        script.push_str("item exit   Exit to Local BIOS/UEFI\n\n");

        script.push_str(
            "choose --timeout ${menu-timeout} --default ${menu-default} selected || goto shell\n",
        );
        script.push_str("goto ${selected}\n\n");

        script.push_str(":shell\nshell\n\n:reboot\nreboot\n\n:exit\nexit\n\n");
        script.push_str(":boot_failed\necho Boot failed or cancelled! Dropping to shell in 5 seconds...\nsleep 5\ngoto shell\n\n");

        for h in handlers {
            script.push_str(&h);
            script.push_str("\n");
        }

        script
    }

    /// Builds dynamic GRUB configuration for Engine B (Pure NetLoop HTTP 206)
    pub async fn build_netloop_grub_cfg(&self) -> String {
        let (port, storage_node) = {
            let c = self.config.read().await;
            let node = c
                .storage_node
                .clone()
                .unwrap_or_else(|| "${net_default_server}".to_string());
            (c.bridge_port, node)
        };
        let mut cfg = String::new();
        cfg.push_str("set default=0\nset timeout=0\n\n");
        cfg.push_str("menuentry \"Track B: CachyOS NetLoop Direct Boot\" {\n");
        cfg.push_str("    insmod serial\n    insmod terminal\n    serial --unit=0 --speed=115200\n    terminal_output console serial\n    terminal_input console serial\n\n");
        cfg.push_str("    echo \"=== Track B: Pure NetLoop GRUB Direct Boot ===\"\n");
        cfg.push_str("    insmod http\n    insmod loopback\n    insmod part_gpt\n    insmod ext2\n    insmod linux\n\n");
        cfg.push_str(&format!(
            "    echo \"[STEP 1/4] Establishing HTTP 206 Loopback to 0001COSext4.vhd.vtoy...\"\n    loopback loop (http,{}:{port})/iso/0001COSext4.vhd.vtoy\n\n\
                 echo \"[STEP 2/4] Parsing internal GPT table and extracting CachyOS kernel...\"\n    linux (loop,gpt2)/boot/vmlinuz-linux-cachyos console=ttyS0,115200 console=tty0 earlycon=uart8250,io,0x3f8,115200 ip=dhcp nbdroot={} nbdport=10810 nbdmaxpart=16 root=/dev/nbd0p2 rw nosplash verbose fsck.mode=skip systemd.mask=boot-efi.mount systemd.default_timeout_start_sec=10s\n\n\
                 echo \"[STEP 3/4] Extracting CachyOS native initramfs from loopback block device...\"\n    initrd (loop,gpt2)/boot/initramfs-linux-cachyos.img\n\n\
                 echo \"[STEP 4/4] Launching kernel (ExitBootServices)...\"\n    boot\n",
            storage_node, storage_node
        ));
        cfg.push_str("}\n");
        cfg
    }
}
