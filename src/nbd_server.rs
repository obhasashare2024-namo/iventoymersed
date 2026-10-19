use std::fs::OpenOptions;
use std::net::SocketAddr;
use std::os::unix::fs::FileExt;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

// NBD Protocol Magic Numbers
const NBD_MAGIC: u64 = 0x4e42444d41474943; // "NBDMAGIC"
const NBD_OPTS_MAGIC: u64 = 0x49484156454F5054; // "IHAVEOPT"
const NBD_REP_MAGIC: u64 = 0x0003e889045565a9;
const NBD_REQUEST_MAGIC: u32 = 0x25609513;
const NBD_SIMPLE_REPLY_MAGIC: u32 = 0x67446698;

// Handshake Flags
const NBD_FLAG_FIXED_NEWSTYLE: u16 = 0x0001;
const NBD_FLAG_NO_ZEROES: u16 = 0x0002;

// Transmission Flags
const NBD_FLAG_HAS_FLAGS: u16 = 0x0001;
const NBD_FLAG_SEND_FLUSH: u16 = 0x0004;
#[allow(dead_code)]
const NBD_FLAG_SEND_FUA: u16 = 0x0008;
const NBD_FLAG_SEND_TRIM: u16 = 0x0020;
const NBD_FLAG_CAN_MULTI_CONN: u16 = 0x0100;

// Option Types
const NBD_OPT_EXPORT_NAME: u32 = 1;
const NBD_OPT_ABORT: u32 = 2;
const NBD_OPT_LIST: u32 = 3;
const NBD_OPT_INFO: u32 = 6;
const NBD_OPT_GO: u32 = 7;
const NBD_OPT_STRUCTURED_REPLY: u32 = 8;

// Option Reply Types
const NBD_REP_ACK: u32 = 1;
const NBD_REP_SERVER: u32 = 2;
const NBD_REP_INFO: u32 = 3;
const NBD_REP_ERR_UNSUP: u32 = 0x80000001;

// Info Types
const NBD_INFO_EXPORT: u16 = 0;

// Command Types
const NBD_CMD_READ: u16 = 0;
const NBD_CMD_WRITE: u16 = 1;
const NBD_CMD_DISC: u16 = 2;
const NBD_CMD_FLUSH: u16 = 3;
const NBD_CMD_TRIM: u16 = 4;

#[allow(dead_code)]
pub struct EmbeddedNbdServer {
    pub port: u16,
    pub path: String,
    pub export_name: String,
    shutdown_tx: Option<watch::Sender<bool>>,
    join_handle: Option<tokio::task::JoinHandle<()>>,
}

impl EmbeddedNbdServer {
    pub async fn start(port: u16, path: &str, export_name: &str) -> std::io::Result<Self> {
        let (file, is_read_only) = match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => (f, false),
            Err(e) => {
                eprintln!("[nbd] Read-write open failed for '{}': {}. Falling back to read-only.", path, e);
                let ro_file = OpenOptions::new().read(true).open(path)?;
                (ro_file, true)
            }
        };

        let export_size = file.metadata()?.len();
        let file_arc = Arc::new(file);

        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        let srv_port = port;
        let srv_path = path.to_string();
        let srv_name = export_name.to_string();
        let srv_name_srv = srv_name.clone();

        let join_handle = tokio::spawn(async move {
            println!("[nbd] Pure Rust In-Tree NBD Server listening on port {} (export='{}', size={} MB, ro={})",
                     srv_port, srv_name_srv, export_size / (1024 * 1024), is_read_only);

            loop {
                tokio::select! {
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((stream, peer_addr)) => {
                                let f = file_arc.clone();
                                let exp_name = srv_name_srv.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(stream, peer_addr, f, export_size, exp_name, is_read_only).await {
                                        // Ignore normal disconnect EOFs
                                        if e.kind() != std::io::ErrorKind::UnexpectedEof && e.kind() != std::io::ErrorKind::ConnectionReset {
                                            eprintln!("[nbd:client {}] Session error: {}", peer_addr, e);
                                        }
                                    }
                                });
                            }
                            Err(e) => {
                                eprintln!("[nbd] Listener accept error on port {}: {}", srv_port, e);
                                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            println!("[nbd] Pure Rust NBD Server on port {} shutting down listener.", srv_port);
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            port,
            path: srv_path,
            export_name: srv_name,
            shutdown_tx: Some(shutdown_tx),
            join_handle: Some(join_handle),
        })
    }

    pub fn shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        if let Some(h) = self.join_handle.take() {
            h.abort();
        }
        println!("[nbd] In-Tree NBD Server on port {} stopped.", self.port);
    }
}

impl Drop for EmbeddedNbdServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    file: Arc<std::fs::File>,
    export_size: u64,
    export_name: String,
    is_read_only: bool,
) -> std::io::Result<()> {
    let _ = stream.set_nodelay(true);

    // 1. Initial Handshake (Fixed Newstyle)
    let mut handshake = Vec::with_capacity(18);
    handshake.extend_from_slice(&NBD_MAGIC.to_be_bytes());
    handshake.extend_from_slice(&NBD_OPTS_MAGIC.to_be_bytes());
    let server_flags = NBD_FLAG_FIXED_NEWSTYLE | NBD_FLAG_NO_ZEROES;
    handshake.extend_from_slice(&server_flags.to_be_bytes());
    stream.write_all(&handshake).await?;
    stream.flush().await?;

    // 2. Read Client Flags
    let client_flags = stream.read_u32().await?;
    let client_no_zeroes = (client_flags & (NBD_FLAG_NO_ZEROES as u32)) != 0;

    // 3. Option Negotiation Loop
    loop {
        let opt_magic = stream.read_u64().await?;
        if opt_magic != NBD_OPTS_MAGIC {
            eprintln!("[nbd:client {}] Bad option magic: 0x{:016x}", peer_addr, opt_magic);
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Bad NBD option magic"));
        }

        let opt_id = stream.read_u32().await?;
        let opt_len = stream.read_u32().await? as usize;
        let mut opt_data = vec![0u8; opt_len];
        if opt_len > 0 {
            stream.read_exact(&mut opt_data).await?;
        }

        match opt_id {
            NBD_OPT_EXPORT_NAME => {
                let requested_name = String::from_utf8_lossy(&opt_data);
                println!("[nbd:client {}] Handshake completed via OPT_EXPORT_NAME (export='{}')", peer_addr, requested_name);

                let mut trans_flags = NBD_FLAG_HAS_FLAGS | NBD_FLAG_SEND_FLUSH | NBD_FLAG_CAN_MULTI_CONN;
                if is_read_only {
                    trans_flags |= 0x0002; // NBD_FLAG_READ_ONLY
                } else {
                    trans_flags |= NBD_FLAG_SEND_TRIM;
                }

                stream.write_u64(export_size).await?;
                stream.write_u16(trans_flags).await?;
                if !client_no_zeroes {
                    let zeroes = [0u8; 124];
                    stream.write_all(&zeroes).await?;
                }
                stream.flush().await?;

                return run_transmission_loop(stream, peer_addr, file, export_size, is_read_only).await;
            }
            NBD_OPT_ABORT => {
                println!("[nbd:client {}] Client requested OPT_ABORT", peer_addr);
                send_opt_reply(&mut stream, opt_id, NBD_REP_ACK, &[]).await?;
                return Ok(());
            }
            NBD_OPT_LIST => {
                let name_bytes = export_name.as_bytes();
                let mut reply_payload = Vec::with_capacity(4 + name_bytes.len());
                reply_payload.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
                reply_payload.extend_from_slice(name_bytes);
                send_opt_reply(&mut stream, opt_id, NBD_REP_SERVER, &reply_payload).await?;
                send_opt_reply(&mut stream, opt_id, NBD_REP_ACK, &[]).await?;
            }
            NBD_OPT_INFO | NBD_OPT_GO => {
                let mut trans_flags = NBD_FLAG_HAS_FLAGS | NBD_FLAG_SEND_FLUSH | NBD_FLAG_CAN_MULTI_CONN;
                if is_read_only {
                    trans_flags |= 0x0002;
                } else {
                    trans_flags |= NBD_FLAG_SEND_TRIM;
                }

                let mut info_payload = Vec::with_capacity(12);
                info_payload.extend_from_slice(&NBD_INFO_EXPORT.to_be_bytes());
                info_payload.extend_from_slice(&export_size.to_be_bytes());
                info_payload.extend_from_slice(&trans_flags.to_be_bytes());

                send_opt_reply(&mut stream, opt_id, NBD_REP_INFO, &info_payload).await?;
                send_opt_reply(&mut stream, opt_id, NBD_REP_ACK, &[]).await?;

                if opt_id == NBD_OPT_GO {
                    println!("[nbd:client {}] Handshake completed via OPT_GO", peer_addr);
                    return run_transmission_loop(stream, peer_addr, file, export_size, is_read_only).await;
                }
            }
            NBD_OPT_STRUCTURED_REPLY => {
                send_opt_reply(&mut stream, opt_id, NBD_REP_ERR_UNSUP, &[]).await?;
            }
            _ => {
                send_opt_reply(&mut stream, opt_id, NBD_REP_ERR_UNSUP, &[]).await?;
            }
        }
    }
}

async fn send_opt_reply<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    opt_id: u32,
    reply_type: u32,
    data: &[u8],
) -> std::io::Result<()> {
    writer.write_u64(NBD_REP_MAGIC).await?;
    writer.write_u32(opt_id).await?;
    writer.write_u32(reply_type).await?;
    writer.write_u32(data.len() as u32).await?;
    if !data.is_empty() {
        writer.write_all(data).await?;
    }
    writer.flush().await
}

async fn run_transmission_loop(
    stream: TcpStream,
    peer_addr: SocketAddr,
    file: Arc<std::fs::File>,
    export_size: u64,
    is_read_only: bool,
) -> std::io::Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    loop {
        let magic = match reader.read_u32().await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("[nbd:client {}] Client disconnected (EOF)", peer_addr);
                break;
            }
            Err(e) => return Err(e),
        };

        if magic != NBD_REQUEST_MAGIC {
            eprintln!("[nbd:client {}] Invalid request magic: 0x{:08x}", peer_addr, magic);
            break;
        }

        let flags = reader.read_u16().await?;
        let cmd_type = reader.read_u16().await?;
        let handle = reader.read_u64().await?;
        let offset = reader.read_u64().await?;
        let length = reader.read_u32().await?;

        match cmd_type {
            NBD_CMD_READ => {
                if offset.saturating_add(length as u64) > export_size {
                    send_simple_reply(&mut writer, 22 /* EINVAL */, handle).await?;
                    continue;
                }

                let f = file.clone();
                let len = length as usize;
                let read_res = tokio::task::spawn_blocking(move || {
                    let mut buf = vec![0u8; len];
                    match f.read_exact_at(&mut buf, offset) {
                        Ok(()) => Ok(buf),
                        Err(e) => Err(e),
                    }
                })
                .await;

                match read_res {
                    Ok(Ok(buf)) => {
                        send_simple_reply(&mut writer, 0, handle).await?;
                        writer.write_all(&buf).await?;
                        writer.flush().await?;
                    }
                    _ => {
                        send_simple_reply(&mut writer, 5 /* EIO */, handle).await?;
                    }
                }
            }
            NBD_CMD_WRITE => {
                let mut data = vec![0u8; length as usize];
                reader.read_exact(&mut data).await?;

                if is_read_only {
                    send_simple_reply(&mut writer, 30 /* EROFS */, handle).await?;
                    continue;
                }

                if offset.saturating_add(length as u64) > export_size {
                    send_simple_reply(&mut writer, 28 /* ENOSPC */, handle).await?;
                    continue;
                }

                let f = file.clone();
                let fua = (flags & 1) != 0;
                let write_res = tokio::task::spawn_blocking(move || {
                    let r = f.write_all_at(&data, offset);
                    if r.is_ok() && fua {
                        let _ = f.sync_data();
                    }
                    r
                })
                .await;

                match write_res {
                    Ok(Ok(())) => {
                        send_simple_reply(&mut writer, 0, handle).await?;
                    }
                    _ => {
                        send_simple_reply(&mut writer, 5 /* EIO */, handle).await?;
                    }
                }
            }
            NBD_CMD_DISC => {
                println!("[nbd:client {}] Received NBD_CMD_DISC, cleanly terminating session.", peer_addr);
                break;
            }
            NBD_CMD_FLUSH => {
                let f = file.clone();
                let sync_res = tokio::task::spawn_blocking(move || f.sync_data()).await;
                match sync_res {
                    Ok(Ok(())) => {
                        send_simple_reply(&mut writer, 0, handle).await?;
                    }
                    _ => {
                        send_simple_reply(&mut writer, 5 /* EIO */, handle).await?;
                    }
                }
            }
            NBD_CMD_TRIM => {
                if is_read_only {
                    send_simple_reply(&mut writer, 0, handle).await?;
                    continue;
                }

                let f = file.clone();
                let trim_res = tokio::task::spawn_blocking(move || {
                    #[cfg(target_os = "linux")]
                    {
                        use std::os::unix::io::AsRawFd;
                        let fd = f.as_raw_fd();
                        // FALLOC_FL_PUNCH_HOLE (0x02) | FALLOC_FL_KEEP_SIZE (0x01)
                        let ret = unsafe {
                            libc::fallocate(fd, 0x03, offset as libc::off_t, length as libc::off_t)
                        };
                        if ret == 0 {
                            Ok(())
                        } else {
                            Err(std::io::Error::last_os_error())
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    {
                        Ok(())
                    }
                })
                .await;

                match trim_res {
                    Ok(Ok(())) => {
                        send_simple_reply(&mut writer, 0, handle).await?;
                    }
                    _ => {
                        // Return OK on unsupported punch hole to maintain client transparency
                        send_simple_reply(&mut writer, 0, handle).await?;
                    }
                }
            }
            _ => {
                send_simple_reply(&mut writer, 22 /* EINVAL */, handle).await?;
            }
        }
    }

    Ok(())
}

async fn send_simple_reply<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    error: u32,
    handle: u64,
) -> std::io::Result<()> {
    writer.write_u32(NBD_SIMPLE_REPLY_MAGIC).await?;
    writer.write_u32(error).await?;
    writer.write_u64(handle).await?;
    writer.flush().await
}
