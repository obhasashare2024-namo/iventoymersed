mod bcd;
mod config;
mod image;
mod nbd_server;
mod server;
mod storage;
mod timefmt;

use config::{load_simple_yaml, BootMode, Config};
use std::sync::Arc;
use std::time::Instant;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_usage() {
    println!(
        "iventoy-ipxe v{} - Pure Rust Standalone All-in-One iVentoy / iPXE Boot Server\n\nUsage: iventoy-ipxe [OPTIONS]\n\n  --config <PATH>  config file (default /etc/iventoy-ipxe/config.yaml or iventoy.json)\n  --host <HOST>    override BRIDGE_HOST\n  --port <PORT>    override BRIDGE_PORT\n  --version        print version\n  --help           this help",
        VERSION
    );
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exe_name = args.first().cloned().unwrap_or_default();
    if exe_name.ends_with("vtoy-nbd") || exe_name.ends_with("nbd-client") {
        run_nbd_client(&args[1..]);
        return;
    }

    if args.len() > 1 {
        match args[1].as_str() {
            "nbd" => {
                run_standalone_nbd(&args[2..]).await;
                return;
            }
            "nbd-client" => {
                run_nbd_client(&args[2..]);
                return;
            }
            "inspect" => {
                run_inspect(&args[2..]);
                return;
            }
            "patch-bcd" => {
                run_patch_bcd(&args[2..]);
                return;
            }
            _ => {}
        }
    }

    let mut cfg_path = std::env::var("BRIDGE_CONFIG")
        .unwrap_or_else(|_| "/etc/iventoy-ipxe/config.yaml".to_string());
    let mut host_override: Option<String> = None;
    let mut port_override: Option<u16> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--config" => {
                i += 1;
                if i < args.len() {
                    cfg_path = args[i].clone();
                }
            }
            "--host" => {
                i += 1;
                if i < args.len() {
                    host_override = Some(args[i].clone());
                }
            }
            "--port" => {
                i += 1;
                if i < args.len() {
                    port_override = args[i].parse().ok();
                }
            }
            "--version" => {
                println!("iventoy-ipxe v{}", VERSION);
                return;
            }
            "--help" | "-h" => {
                print_usage();
                return;
            }
            other => {
                eprintln!("unknown argument: {}", other);
                print_usage();
                std::process::exit(2);
            }
        }
        i += 1;
    }

    let mut cfg: Config = load_simple_yaml(&cfg_path).unwrap_or_default();
    if let Some(h) = host_override {
        cfg.bridge_host = h;
    }
    if let Some(p) = port_override {
        cfg.bridge_port = p;
    }

    let state = Arc::new(server::AppState {
        storage: Arc::new(storage::StorageManager::new(cfg.clone())),
        start: Instant::now(),
    });

    state.storage.scan_and_export().await;

    let addr = format!("{}:{}", cfg.bridge_host, cfg.bridge_port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[ERR] failed to bind {}: {}", addr, e);
            std::process::exit(1);
        }
    };
    println!(
        "[*] iventoy-ipxe v{} (Pure Rust Standalone) running on {} mode={} pid={}",
        VERSION,
        addr,
        match cfg.boot_mode {
            BootMode::Iscsi => "iscsi",
            BootMode::Nbd => "nbd",
        },
        std::process::id()
    );

    let app = server::build_router(state.clone());
    let shutdown_state = state.clone();
    let shutdown = async move {
        wait_for_signal().await;
        println!("\n[*] Shutting down {} (draining connections)...", env!("CARGO_PKG_NAME"));
    };

    if let Err(e) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    {
        eprintln!("[ERR] server error: {}", e);
    }
    shutdown_state.storage.shutdown_all().await;
}

async fn wait_for_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    tokio::select! {
        _ = term.recv() => { println!("[*] Received SIGTERM, shutting down Bridge..."); }
        _ = int.recv() => { println!("[*] Received SIGINT, shutting down Bridge..."); }
    }
}

async fn run_standalone_nbd(args: &[String]) {
    let mut port: u16 = 9111;
    let mut base_path = String::new();
    let mut diff_path: Option<String> = None;
    let mut export_name = "vtoy".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--port" | "-p" => {
                i += 1;
                if i < args.len() {
                    port = args[i].parse().unwrap_or(9111);
                }
            }
            "--base" | "-b" => {
                i += 1;
                if i < args.len() {
                    base_path = args[i].clone();
                }
            }
            "--diff" | "-d" => {
                i += 1;
                if i < args.len() {
                    diff_path = Some(args[i].clone());
                }
            }
            "--export" | "-e" => {
                i += 1;
                if i < args.len() {
                    export_name = args[i].clone();
                }
            }
            "--help" | "-h" => {
                println!("Usage: iventoymersed nbd --port <PORT> --base <PATH> [--diff <PATH>] [--export <NAME>]");
                return;
            }
            other => {
                eprintln!("[nbd] Unknown option: {}", other);
                return;
            }
        }
        i += 1;
    }

    if base_path.is_empty() {
        eprintln!("[nbd] Error: --base <PATH> is required.");
        return;
    }

    println!("[*] Starting iventoymersed In-Tree Pure Rust NBD Server (v{})", VERSION);
    println!("    Port:        {}", port);
    println!("    Export Name: {}", export_name);
    println!("    Base (RO):   {}", base_path);
    if let Some(ref d) = diff_path {
        println!("    Diff (COW):  {} (Raw Sparse .diff)", d);
    } else {
        println!("    Mode:        Direct Read-Only / Read-Write");
    }

    let mut server = match diff_path {
        Some(d) => match nbd_server::EmbeddedNbdServer::start_cow(port, &base_path, &d, &export_name).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ERR] Failed to start COW NBD server: {}", e);
                std::process::exit(1);
            }
        },
        None => match nbd_server::EmbeddedNbdServer::start(port, &base_path, &export_name).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[ERR] Failed to start Direct NBD server: {}", e);
                std::process::exit(1);
            }
        },
    };

    wait_for_signal().await;
    server.shutdown();
}

fn run_inspect(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: iventoymersed inspect <image_path>");
        std::process::exit(1);
    }
    let path = &args[0];
    println!("[*] Inspecting GPT partition table for '{}'...", path);
    match bcd::inspect_disk_gpt(path) {
        Ok(Some(info)) => {
            println!("    Disk GUID:       {}", info.disk_guid.to_string());
            println!("    Has ESP (EFI):   {}", info.has_esp);
            println!("    Partition Count: {}", info.partitions.len());
            for p in &info.partitions {
                println!("      [{}] Type: {} | Unique: {} | LBA: {}..{} | Name: '{}'",
                         p.index, p.type_guid.to_string(), p.unique_guid.to_string(), p.start_lba, p.end_lba, p.name);
            }
            if let Some(ref win) = info.windows_partition {
                println!("    -> Detected Windows Partition: Index={}, Unique GUID={}", win.index, win.unique_guid.to_string());
            } else {
                println!("    -> No Microsoft Basic Data partition detected.");
            }
        }
        Ok(None) => {
            println!("[!] No valid GPT partition table found on '{}' (may be MBR, RAW, or unsupported format).", path);
        }
        Err(e) => {
            eprintln!("[ERR] Failed to read disk: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_patch_bcd(args: &[String]) {
    if args.is_empty() {
        eprintln!("Usage: iventoymersed patch-bcd <image_path> [output_bcd_file]");
        std::process::exit(1);
    }
    let image_path = &args[0];
    let out_path = if args.len() > 1 {
        args[1].clone()
    } else {
        let p = std::path::Path::new(image_path);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("patched");
        format!("{}_uefi.bcd", stem)
    };

    println!("[*] Synthesizing patched BCD for '{}' -> '{}'...", image_path, out_path);
    match bcd::generate_patched_bcd_for_image(image_path) {
        Ok((data, disk_guid, part_guid)) => {
            if let Err(e) = std::fs::write(&out_path, &data) {
                eprintln!("[ERR] Failed to write output file '{}': {}", out_path, e);
                std::process::exit(1);
            }
            println!("[+] Successfully generated {} bytes patched BCD hive!", data.len());
            println!("    Target Disk GUID:      {}", disk_guid.to_string());
            println!("    Target Partition GUID: {}", part_guid.to_string());
            println!("    Output File:           {}", out_path);
        }
        Err(e) => {
            eprintln!("[ERR] BCD generation failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_nbd_client(args: &[String]) {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::os::unix::io::AsRawFd;

    if args.len() < 3 {
        eprintln!("Usage: iventoymersed nbd-client <host> <port> <device> [export_name]");
        std::process::exit(1);
    }
    let host = &args[0];
    let port: u16 = args[1].parse().unwrap_or(10810);
    let dev_path = &args[2];
    let export_name = if args.len() > 3 { &args[3] } else { "vtoy" };

    println!("[nbd-client] Connecting to {}:{} for export '{}' -> {}...", host, port, export_name, dev_path);

    let mut stream = match TcpStream::connect((host.as_str(), port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[nbd-client] TCP connection failed: {}", e);
            std::process::exit(1);
        }
    };

    let mut init = [0u8; 18];
    if let Err(e) = stream.read_exact(&mut init) {
        eprintln!("[nbd-client] Handshake read failed: {}", e);
        std::process::exit(1);
    }
    let magic = u64::from_be_bytes(init[0..8].try_into().unwrap());
    let opts_magic = u64::from_be_bytes(init[8..16].try_into().unwrap());
    if magic != 0x4e42444d41474943 || opts_magic != 0x49484156454F5054 {
        eprintln!("[nbd-client] Invalid NBD magic: {:x}/{:x}", magic, opts_magic);
        std::process::exit(1);
    }

    let cflags = 3u32.to_be_bytes();
    if let Err(e) = stream.write_all(&cflags) {
        eprintln!("[nbd-client] Failed to write client flags: {}", e);
        std::process::exit(1);
    }

    let exp_bytes = export_name.as_bytes();
    let mut opt_req = Vec::new();
    opt_req.extend_from_slice(&0x49484156454F5054u64.to_be_bytes());
    opt_req.extend_from_slice(&1u32.to_be_bytes());
    opt_req.extend_from_slice(&(exp_bytes.len() as u32).to_be_bytes());
    opt_req.extend_from_slice(exp_bytes);
    if let Err(e) = stream.write_all(&opt_req) {
        eprintln!("[nbd-client] Failed to send export name option: {}", e);
        std::process::exit(1);
    }

    let mut rep = [0u8; 10];
    if let Err(e) = stream.read_exact(&mut rep) {
        eprintln!("[nbd-client] Failed to read export reply: {}", e);
        std::process::exit(1);
    }
    let export_size = u64::from_be_bytes(rep[0..8].try_into().unwrap());
    let flags = u16::from_be_bytes(rep[8..10].try_into().unwrap());
    println!("[nbd-client] Connected! Export size: {} MB, Flags: 0x{:04x}", export_size / (1024 * 1024), flags);

    let nbd_file = match std::fs::OpenOptions::new().read(true).write(true).open(dev_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[nbd-client] Failed to open {}: {}", dev_path, e);
            std::process::exit(1);
        }
    };
    let nbd_fd = nbd_file.as_raw_fd();
    let sock_fd = stream.as_raw_fd();

    const NBD_SET_SOCK: libc::c_int = 0xab00;
    const NBD_SET_BLKSIZE: libc::c_int = 0xab01;
    const NBD_SET_SIZE_BLOCKS: libc::c_int = 0xab07;
    const NBD_DO_IT: libc::c_int = 0xab03;
    const NBD_CLEAR_SOCK: libc::c_int = 0xab04;

    unsafe {
        libc::ioctl(nbd_fd, NBD_CLEAR_SOCK, 0);
        libc::ioctl(nbd_fd, NBD_SET_SOCK, sock_fd);
        libc::ioctl(nbd_fd, NBD_SET_BLKSIZE, 512);
        libc::ioctl(nbd_fd, NBD_SET_SIZE_BLOCKS, export_size / 512);
        println!("[nbd-client] Entered kernel driver loop for {}!", dev_path);
        libc::ioctl(nbd_fd, NBD_DO_IT);
        libc::ioctl(nbd_fd, NBD_CLEAR_SOCK, 0);
    }
    println!("[nbd-client] Disconnected from {}.", dev_path);
}



