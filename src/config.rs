use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Config {
    pub iventoy_real: String,
    pub bridge_host: String,
    pub bridge_port: u16,
    pub vtoy_dir: String,
    pub boot_mode: BootMode,
    pub tgt_base_tid: u32,
    pub iqn_prefix: String,
    pub nbd_base_port: u16,
    pub vtoy_kernel_url: String,
    pub vtoy_initrd_url: String,
    pub kernel_path: String,
    pub initrd_path: String,
    pub role: String,
    pub storage_node: Option<String>,
    pub storage_port: Option<u16>,
    pub json_cfg: IVentoyJson,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BootMode {
    Iscsi,
    Nbd,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct IVentoyJson {
    #[serde(default)]
    pub control: Vec<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub windows_boot: WindowsBootConfig,
    #[serde(default)]
    pub image_override: Vec<ImageOverride>,
    #[serde(default)]
    pub default_engine: Option<String>,
    #[serde(default)]
    pub default_image: Option<String>,
    #[serde(default)]
    pub mac_bindings: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowsBootConfig {
    #[serde(default = "default_win_mode")]
    pub default_mode: String, // "iscsi" or "vtoyboot"
    #[serde(default = "default_iscsi_port")]
    pub iscsi_portal_port: u16,
    #[serde(default = "default_true")]
    pub auto_ram_cow: bool,
    #[serde(default = "default_cow_size")]
    pub cow_size_mb: u32,
}

fn default_win_mode() -> String {
    "iscsi".into()
}
fn default_iscsi_port() -> u16 {
    3260
}
fn default_true() -> bool {
    true
}
fn default_cow_size() -> u32 {
    4096
}

impl Default for WindowsBootConfig {
    fn default() -> Self {
        Self {
            default_mode: default_win_mode(),
            iscsi_portal_port: default_iscsi_port(),
            auto_ram_cow: default_true(),
            cow_size_mb: default_cow_size(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageOverride {
    pub image: String,
    #[serde(default)]
    pub win_mode: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            iventoy_real: "127.0.0.1:16001".into(),
            bridge_host: "0.0.0.0".into(),
            bridge_port: 16000,
            vtoy_dir: "/srv/vtoy/iso".into(),
            boot_mode: BootMode::Iscsi,
            tgt_base_tid: 100,
            iqn_prefix: "iqn.2026-10.lan:vtoy".into(),
            nbd_base_port: 10810,
            vtoy_kernel_url: "http://${next-server}:16000/vtoy/vmlinuz".into(),
            vtoy_initrd_url: "http://${next-server}:16000/vtoy/initramfs.img".into(),
            kernel_path: "/srv/vtoy/boot/vmlinuz".into(),
            initrd_path: "/srv/vtoy/boot/initramfs.img".into(),
            role: "standalone".into(),
            storage_node: None,
            storage_port: None,
            json_cfg: IVentoyJson::default(),
        }
    }
}

fn parse_value(v: &str) -> String {
    v.trim().trim_matches('"').trim_matches('\'').to_string()
}

pub fn load_iventoy_json(vtoy_dir: &str) -> IVentoyJson {
    let candidates = [
        format!("{}/iventoy.json", vtoy_dir),
        format!("{}/ventoy.json", vtoy_dir),
        "iventoy.json".into(),
        "ventoy.json".into(),
    ];
    for p in &candidates {
        if Path::new(p).exists() {
            if let Ok(content) = std::fs::read_to_string(p) {
                if let Ok(cfg) = serde_json::from_str::<IVentoyJson>(&content) {
                    println!("[config] Loaded json config from: {}", p);
                    return cfg;
                }
            }
        }
    }
    println!("[config] No iventoy.json found, using defaults");
    IVentoyJson::default()
}

pub fn save_iventoy_json(vtoy_dir: &str, cfg: &IVentoyJson) -> Result<(), String> {
    let p = format!("{}/iventoy.json", vtoy_dir);
    let s = serde_json::to_string_pretty(cfg).map_err(|e| format!("Failed to serialize config: {}", e))?;
    std::fs::write(&p, s).map_err(|e| format!("Failed to write {}: {}", p, e))?;
    println!("[config] Saved updated config to {}", p);
    Ok(())
}

pub fn load_simple_yaml(path: &str) -> Result<Config, String> {
    let mut cfg = Config::default();
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => {
            cfg.json_cfg = load_iventoy_json(&cfg.vtoy_dir);
            return Ok(cfg);
        }
    };
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim();
            let v = parse_value(v);
            match k {
                "IVENTOY_REAL" => cfg.iventoy_real = v,
                "BRIDGE_HOST" => cfg.bridge_host = v,
                "BRIDGE_PORT" => cfg.bridge_port = v.parse().unwrap_or(cfg.bridge_port),
                "VTOY_DIR" => cfg.vtoy_dir = v,
                "BOOT_MODE" => {
                    cfg.boot_mode = if v.eq_ignore_ascii_case("nbd") {
                        BootMode::Nbd
                    } else {
                        BootMode::Iscsi
                    }
                }
                "TGT_BASE_TID" => cfg.tgt_base_tid = v.parse().unwrap_or(cfg.tgt_base_tid),
                "IQN_PREFIX" => cfg.iqn_prefix = v,
                "NBD_BASE_PORT" => cfg.nbd_base_port = v.parse().unwrap_or(cfg.nbd_base_port),
                "VTOY_KERNEL_URL" => cfg.vtoy_kernel_url = v,
                "VTOY_INITRD_URL" => cfg.vtoy_initrd_url = v,
                "KERNEL_PATH" => cfg.kernel_path = v,
                "INITRD_PATH" => cfg.initrd_path = v,
                "ROLE" => cfg.role = v.to_ascii_lowercase(),
                "STORAGE_NODE" => cfg.storage_node = Some(v),
                "STORAGE_PORT" => cfg.storage_port = v.parse().ok(),
                _ => {}
            }
        }
    }
    // Environment variable overrides
    if let Ok(v) = std::env::var("ROLE") {
        if !v.trim().is_empty() {
            cfg.role = v.trim().to_ascii_lowercase();
        }
    }
    if let Ok(v) = std::env::var("STORAGE_NODE") {
        if !v.trim().is_empty() {
            cfg.storage_node = Some(v.trim().to_string());
        }
    }
    if let Ok(v) = std::env::var("STORAGE_PORT") {
        if let Ok(p) = v.trim().parse() {
            cfg.storage_port = Some(p);
        }
    }
    cfg.json_cfg = load_iventoy_json(&cfg.vtoy_dir);
    Ok(cfg)
}
