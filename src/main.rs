mod config;
mod image;
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
