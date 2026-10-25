use crate::storage::{sanitize_ipxe_key, StorageManager};
use axum::body::Body;
use axum::extract::{ConnectInfo, Path as AxumPath, Query, Request, State};
use axum::http::{header, Method, Response, StatusCode};
use axum::routing::{any, get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::io::ReaderStream;

const DASHBOARD_HTML: &str = include_str!("static/index.html");

pub struct AppState {
    pub storage: Arc<StorageManager>,
    pub start: Instant,
}

fn http_ts() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    crate::timefmt::format_datetime(secs)
}

fn req_log(client: &SocketAddr, line: &str) {
    println!("[{}] {} - {}", http_ts(), client.ip(), line);
}

fn get_rss_mb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = statm.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(resident_pages) = parts[1].parse::<f64>() {
                    return (resident_pages * 4096.0) / (1024.0 * 1024.0);
                }
            }
        }
    }
    4.4
}

async fn api_status(State(state): State<Arc<AppState>>) -> Response<Body> {
    #[derive(Serialize)]
    struct TargetOut {
        file: String,
        path: String,
        iqn: String,
        nbd_port: u16,
        backend: String,
        root_dev: String,
        format: String,
        is_windows: bool,
        has_partitions: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        alive: Option<bool>,
    }

    #[derive(Serialize)]
    struct StatusOut {
        status: &'static str,
        version: &'static str,
        pid: u32,
        uptime_seconds: f64,
        rss_mb: f64,
        role: String,
        storage_node: Option<String>,
        storage_port: Option<u16>,
        boot_mode: String,
        default_win_mode: String,
        default_image: Option<String>,
        mac_bindings: HashMap<String, String>,
        targets: std::collections::BTreeMap<String, TargetOut>,
    }

    let cfg = state.storage.get_config().await;
    let mut targets = std::collections::BTreeMap::new();
    for t in state.storage.health_targets().await {
        targets.insert(
            sanitize_ipxe_key(&t.id),
            TargetOut {
                file: t.file.clone(),
                path: t.path.clone(),
                iqn: t.iqn.clone(),
                nbd_port: t.nbd_port,
                backend: t.backend_type.clone(),
                root_dev: t.root_dev.clone(),
                format: t.format.clone(),
                is_windows: t.is_windows,
                has_partitions: t.has_partitions,
                alive: t.alive,
            },
        );
    }

    let uptime = state.start.elapsed().as_secs_f64();
    let body = serde_json::to_string_pretty(&StatusOut {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        pid: std::process::id(),
        uptime_seconds: uptime,
        rss_mb: get_rss_mb(),
        role: cfg.role.clone(),
        storage_node: cfg.storage_node.clone(),
        storage_port: cfg.storage_port,
        boot_mode: format!("{:?}", cfg.boot_mode).to_lowercase(),
        default_win_mode: cfg.json_cfg.windows_boot.default_mode,
        default_image: cfg.json_cfg.default_image,
        mac_bindings: cfg.json_cfg.mac_bindings,
        targets,
    })
    .unwrap_or_else(|_| "{}".into());

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::CONTENT_LENGTH, body.len())
        .body(Body::from(body))
        .unwrap()
}

async fn serve_static(path: &str, is_head: bool, range: Option<&header::HeaderValue>) -> Response<Body> {
    match tokio::fs::File::open(path).await {
        Ok(mut file) => {
            let meta = match file.metadata().await {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("[ERR] Failed to stat static file {}: {}", path, e);
                    return Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(format!("Failed to stat file: {}", e)))
                        .unwrap();
                }
            };
            let len = meta.len();

            // HTTP 206 Partial Content (Range Support)
            if let Some(range_val) = range.and_then(|r| r.to_str().ok()) {
                if let Some(spec) = range_val.strip_prefix("bytes=") {
                    let parts: Vec<&str> = spec.split('-').collect();
                    if parts.len() == 2 {
                        let start = parts[0].parse::<u64>().ok();
                        let end = parts[1].parse::<u64>().ok();
                        if let Some(start_pos) = start {
                            let end_pos = end.unwrap_or(len.saturating_sub(1)).min(len.saturating_sub(1));
                            if start_pos <= end_pos && start_pos < len {
                                let chunk_len = end_pos - start_pos + 1;
                                use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
                                if file.seek(SeekFrom::Start(start_pos)).await.is_ok() {
                                    let body = if is_head {
                                        Body::empty()
                                    } else if chunk_len <= 1024 * 1024 {
                                        let mut buf = vec![0u8; chunk_len as usize];
                                        match file.read_exact(&mut buf).await {
                                            Ok(_) => Body::from(buf),
                                            Err(_) => {
                                                let stream = ReaderStream::new(file.take(chunk_len));
                                                Body::from_stream(stream)
                                            }
                                        }
                                    } else {
                                        let stream = ReaderStream::new(file.take(chunk_len));
                                        Body::from_stream(stream)
                                    };

                                    return Response::builder()
                                        .status(StatusCode::PARTIAL_CONTENT)
                                        .header(header::CONTENT_TYPE, "application/octet-stream")
                                        .header(header::CONTENT_LENGTH, chunk_len)
                                        .header(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start_pos, end_pos, len))
                                        .header(header::ACCEPT_RANGES, "bytes")
                                        .header(header::CONNECTION, "keep-alive")
                                        .body(body)
                                        .unwrap();
                                }
                            }
                        }
                    }
                }
            }

            if is_head {
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, len)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(header::CONNECTION, "keep-alive")
                    .body(Body::empty())
                    .unwrap()
            } else if len < 10 * 1024 * 1024 {
                // In-memory buffer for small files (kernel, wimboot, initrd) to eliminate streaming latency
                if let Ok(bytes) = tokio::fs::read(path).await {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "application/octet-stream")
                        .header(header::CONTENT_LENGTH, bytes.len())
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(header::CONNECTION, "keep-alive")
                        .body(Body::from(bytes))
                        .unwrap()
                } else {
                    let stream = ReaderStream::new(file);
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "application/octet-stream")
                        .header(header::CONTENT_LENGTH, len)
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(header::CONNECTION, "keep-alive")
                        .body(Body::from_stream(stream))
                        .unwrap()
                }
            } else {
                let stream = ReaderStream::new(file);
                let body = Body::from_stream(stream);
                Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, len)
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(header::CONNECTION, "keep-alive")
                    .body(body)
                    .unwrap()
            }
        }
        Err(e) => {
            eprintln!("[ERR] Failed to open static file {}: {}", path, e);
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from(format!("File not found: {}", e)))
                .unwrap()
        }
    }
}

async fn render_top_level_ipxe_menu(storage: &StorageManager, client_mac: Option<&str>) -> Response<Body> {
    storage.scan_and_export().await;
    let script = storage.build_top_level_ipxe_menu(client_mac).await;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_LENGTH, script.len())
        .body(Body::from(script))
        .unwrap()
}

async fn render_standalone_ipxe_menu(storage: &StorageManager, client_mac: Option<&str>) -> Response<Body> {
    storage.scan_and_export().await;
    let script = storage.build_standalone_ipxe_menu(client_mac).await;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_LENGTH, script.len())
        .body(Body::from(script))
        .unwrap()
}

// Handler for root path "/"
async fn root_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Query(params): Query<HashMap<String, String>>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /", method));

    let ua = req
        .headers()
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let is_ipxe = ua.contains("iPXE") || params.get("format").map(|v| v.as_str()) == Some("ipxe");
    let mac = params.get("mac").map(|s| s.as_str());

    if is_ipxe {
        if method == Method::GET {
            return render_top_level_ipxe_menu(&state.storage, mac).await;
        }
        if method == Method::HEAD {
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/plain")
                .body(Body::empty())
                .unwrap();
        }
    }

    if method == Method::GET || method == Method::HEAD {
        let body = if method == Method::HEAD {
            Body::empty()
        } else {
            Body::from(DASHBOARD_HTML)
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CONTENT_LENGTH, DASHBOARD_HTML.len())
            .body(body)
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::empty())
        .unwrap()
}

// Top-level dual-engine selector handler
async fn top_level_ipxe_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Query(params): Query<HashMap<String, String>>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /boot.ipxe (Top-Level Dual Engine)", method));

    let mac = params.get("mac").map(|s| s.as_str());
    if method == Method::GET {
        return render_top_level_ipxe_menu(&state.storage, mac).await;
    }
    if method == Method::HEAD {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::empty())
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::empty())
        .unwrap()
}

// Engine A subsystem handler
async fn track_a_ipxe_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Query(params): Query<HashMap<String, String>>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /ipxe/menu-a (Engine A Standalone)", method));

    let mac = params.get("mac").map(|s| s.as_str());
    if method == Method::GET {
        return render_standalone_ipxe_menu(&state.storage, mac).await;
    }
    if method == Method::HEAD {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::empty())
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::empty())
        .unwrap()
}

// Engine B subsystem handler (dynamic GRUB cfg)
async fn track_b_grub_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /grub/grub.cfg (Engine B NetLoop)", method));

    state.storage.scan_and_export().await;
    let cfg_text = state.storage.build_netloop_grub_cfg().await;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_LENGTH, cfg_text.len())
        .header(header::CONNECTION, "keep-alive")
        .body(Body::from(cfg_text))
        .unwrap()
}

// Handlers for GRUB binaries
async fn grub_core_handler() -> Response<Body> {
    let candidates = [
        "assets/grub/core.0",
        "/home/namobuddha/lammacpp/iventoymersed/assets/grub/core.0",
        "/mnt/2tbnvS/tmp/tftp/boot/grub/i386-pc/core.0",
    ];
    for p in &candidates {
        if std::path::Path::new(p).exists() {
            if let Ok(bytes) = tokio::fs::read(p).await {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, bytes.len())
                    .body(Body::from(bytes))
                    .unwrap();
            }
        }
    }
    Response::builder().status(StatusCode::NOT_FOUND).body(Body::from("core.0 not found")).unwrap()
}

async fn grub_efi_handler() -> Response<Body> {
    let candidates = [
        "assets/grub/grubnet.efi",
        "/home/namobuddha/lammacpp/iventoymersed/assets/grub/grubnet.efi",
        "/mnt/2tbnvS/tmp/grub_build/grubnet.efi",
    ];
    for p in &candidates {
        if std::path::Path::new(p).exists() {
            if let Ok(bytes) = tokio::fs::read(p).await {
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/octet-stream")
                    .header(header::CONTENT_LENGTH, bytes.len())
                    .body(Body::from(bytes))
                    .unwrap();
            }
        }
    }
    Response::builder().status(StatusCode::NOT_FOUND).body(Body::from("grubnet.efi not found")).unwrap()
}

// Handlers for static boot files
async fn vmlinuz_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /vtoy/vmlinuz", method));
    let cfg = state.storage.get_config().await;
    serve_static(&cfg.kernel_path, method == Method::HEAD, req.headers().get(header::RANGE)).await
}

async fn initramfs_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /vtoy/initramfs.img", method));
    let cfg = state.storage.get_config().await;
    serve_static(&cfg.initrd_path, method == Method::HEAD, req.headers().get(header::RANGE)).await
}

async fn vtoy_overlay_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /vtoy/vtoy_overlay.cpio", method));
    let cfg = state.storage.get_config().await;
    let overlay_path = format!("{}/vtoy_overlay.cpio", cfg.vtoy_dir);
    serve_static(&overlay_path, method == Method::HEAD, req.headers().get(header::RANGE)).await
}

async fn vtoy_bcd_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Query(params): Query<HashMap<String, String>>,
) -> Response<Body> {
    req_log(&client, &format!("GET /vtoy/bcd (params={:?})", params));
    if let Some(image_name) = params.get("image").or_else(|| params.get("target")) {
        let snapshot = state.storage.health_targets().await;
        if let Some(target) = snapshot.iter().find(|t| &t.file == image_name || &t.id == image_name) {
            match crate::bcd::generate_patched_bcd_for_image(&target.path) {
                Ok((bcd_bytes, _, _)) => {
                    return Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "application/octet-stream")
                        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"BCD\"")
                        .header(header::CONTENT_LENGTH, bcd_bytes.len())
                        .body(Body::from(bcd_bytes))
                        .unwrap();
                }
                Err(e) => {
                    eprintln!("[server:bcd] Error generating BCD for {}: {}", target.path, e);
                }
            }
        }
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"BCD\"")
        .header(header::CONTENT_LENGTH, crate::bcd::BASELINE_BCD.len())
        .body(Body::from(crate::bcd::BASELINE_BCD))
        .unwrap()
}

async fn iso_handler(
    State(state): State<Arc<AppState>>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    AxumPath(file): AxumPath<String>,
    req: Request,
) -> Response<Body> {
    let method = req.method().clone();
    req_log(&client, &format!("{} /iso/{}", method, file));
    let cfg = state.storage.get_config().await;
    let fullpath = format!("{}/{}", cfg.vtoy_dir, file);
    serve_static(&fullpath, method == Method::HEAD, req.headers().get(header::RANGE)).await
}

async fn api_preview(State(state): State<Arc<AppState>>) -> Response<Body> {
    let script = state.storage.build_standalone_ipxe_menu(None).await;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(script))
        .unwrap()
}

async fn api_rescan(State(state): State<Arc<AppState>>) -> Response<Body> {
    state.storage.scan_and_export().await;
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"status":"rescanned"}"#))
        .unwrap()
}

#[derive(Deserialize)]
struct SetWinModeIn {
    mode: String,
}

async fn api_set_win_mode(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SetWinModeIn>,
) -> Response<Body> {
    if let Err(e) = state.storage.set_default_win_mode(&payload.mode).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(e))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"ok":true}"#))
        .unwrap()
}

#[derive(Deserialize)]
struct SetDefaultImageIn {
    image: String,
}

async fn api_set_default_image(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<SetDefaultImageIn>,
) -> Response<Body> {
    let img = if payload.image.is_empty() {
        None
    } else {
        Some(payload.image)
    };
    if let Err(e) = state.storage.set_default_image(img).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(e))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"ok":true}"#))
        .unwrap()
}

#[derive(Deserialize)]
struct MacBindingIn {
    mac: String,
    #[serde(default)]
    image: Option<String>,
}

async fn api_add_mac_binding(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<MacBindingIn>,
) -> Response<Body> {
    let img = payload.image.unwrap_or_default();
    if let Err(e) = state.storage.set_mac_binding(payload.mac, img).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(e))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"ok":true}"#))
        .unwrap()
}

async fn api_remove_mac_binding(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<MacBindingIn>,
) -> Response<Body> {
    if let Err(e) = state.storage.remove_mac_binding(&payload.mac).await {
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(e))
            .unwrap();
    }
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"ok":true}"#))
        .unwrap()
}

pub fn make_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", any(root_handler))
        .route("/boot.ipxe", any(top_level_ipxe_handler))
        .route("/ipxe/menu", any(top_level_ipxe_handler))
        .route("/ipxe", any(top_level_ipxe_handler))
        .route("/ipxe/menu-a", any(track_a_ipxe_handler))
        .route("/ipxe/track-a", any(track_a_ipxe_handler))
        .route("/grub/core.0", any(grub_core_handler))
        .route("/grub/grubnet.efi", any(grub_efi_handler))
        .route("/boot/grub/grub.cfg", any(track_b_grub_handler))
        .route("/grub/grub.cfg", any(track_b_grub_handler))
        .route("/vtoy/vmlinuz", any(vmlinuz_handler))
        .route("/vtoy/initramfs.img", any(initramfs_handler))
        .route("/vtoy/vtoy_overlay.cpio", any(vtoy_overlay_handler))
        .route("/vtoy/bcd", any(vtoy_bcd_handler))
        .route("/iso/{*file}", any(iso_handler))
        .route("/api/status", get(api_status))
        .route("/api/health", get(api_status))
        .route("/api/ipxe/preview", get(api_preview))
        .route("/api/rescan", post(api_rescan))
        .route("/api/config/default-win-mode", post(api_set_win_mode))
        .route("/api/config/default-image", post(api_set_default_image))
        .route(
            "/api/config/mac-binding",
            post(api_add_mac_binding).delete(api_remove_mac_binding),
        )
        .with_state(state)
}

pub fn build_router(state: Arc<AppState>) -> Router {
    make_app(state)
}
