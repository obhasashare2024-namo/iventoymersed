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

const VHDX_FILE_SIG: &[u8; 8] = b"vhdxfile";
const VHDX_REGI_SIG: &[u8; 4] = b"regi";
const VHDX_META_SIG: &[u8; 8] = b"metadata";

const BAT_GUID: [u8; 16] = [0x66, 0x77, 0xc2, 0x2d, 0x23, 0xf6, 0x00, 0x42, 0x9d, 0x64, 0x11, 0x5e, 0x9b, 0xfd, 0x4a, 0x08];
const META_GUID: [u8; 16] = [0x06, 0xa2, 0x7c, 0x8b, 0x90, 0x47, 0x9a, 0x4b, 0xb8, 0xfe, 0x57, 0x5f, 0x05, 0x0f, 0x88, 0x6e];

const FILE_PARAMS_GUID: [u8; 16] = [0x37, 0x67, 0xa1, 0xca, 0x36, 0xfa, 0x43, 0x4d, 0xb3, 0xb6, 0x33, 0xf0, 0xaa, 0x44, 0xe7, 0x6b];
const VDISK_SIZE_GUID: [u8; 16] = [0x24, 0x42, 0xa5, 0x2f, 0x1b, 0xcd, 0x76, 0x48, 0xb2, 0x11, 0x5d, 0xbe, 0xd8, 0x3b, 0xf4, 0xb8];
const SECTOR_SIZE_GUID: [u8; 16] = [0x1d, 0xbf, 0x41, 0x81, 0x6f, 0xa9, 0x09, 0x47, 0xba, 0x47, 0xf2, 0x33, 0xa8, 0xfa, 0xab, 0x5f];

#[derive(Clone, Debug)]
pub struct VhdxMeta {
    pub virtual_disk_size: u64,
    pub block_size: u64,
    pub chunk_ratio: u64,
    pub bat: Arc<Vec<u64>>,
}

pub fn parse_vhdx_metadata(file: &std::fs::File) -> std::io::Result<Option<VhdxMeta>> {
    use std::os::unix::fs::FileExt;
    let mut sig = [0u8; 8];
    if file.read_exact_at(&mut sig, 0).is_err() || &sig != VHDX_FILE_SIG {
        return Ok(None);
    }

    let mut regi_hdr = [0u8; 16];
    let mut regi_found = false;
    let mut regi_offset = 196608u64;
    for &candidate in &[196608u64, 262144u64] {
        if file.read_exact_at(&mut regi_hdr, candidate).is_ok() && &regi_hdr[0..4] == VHDX_REGI_SIG {
            regi_found = true;
            regi_offset = candidate;
            break;
        }
    }
    if !regi_found {
        return Ok(None);
    }

    let entry_cnt = u32::from_le_bytes(regi_hdr[8..12].try_into().unwrap()) as usize;
    let mut bat_offset = None;
    let mut bat_length = None;
    let mut meta_offset = None;
    let mut meta_length = None;

    let mut entry_buf = [0u8; 32];
    for i in 0..entry_cnt {
        let off = regi_offset + 16 + (i as u64) * 32;
        if file.read_exact_at(&mut entry_buf, off).is_err() {
            break;
        }
        let guid = &entry_buf[0..16];
        let file_off = u64::from_le_bytes(entry_buf[16..24].try_into().unwrap());
        let length = u32::from_le_bytes(entry_buf[24..28].try_into().unwrap());
        if guid == &BAT_GUID {
            bat_offset = Some(file_off);
            bat_length = Some(length);
        } else if guid == &META_GUID {
            meta_offset = Some(file_off);
            meta_length = Some(length);
        }
    }

    let (bat_off, bat_len) = match (bat_offset, bat_length) {
        (Some(o), Some(l)) => (o, l),
        _ => return Ok(None),
    };
    let (meta_off, _meta_len) = match (meta_offset, meta_length) {
        (Some(o), Some(l)) => (o, l),
        _ => return Ok(None),
    };

    let mut meta_hdr = [0u8; 32];
    file.read_exact_at(&mut meta_hdr, meta_off)?;
    if &meta_hdr[0..8] != VHDX_META_SIG {
        return Ok(None);
    }
    let meta_entry_cnt = u16::from_le_bytes(meta_hdr[10..12].try_into().unwrap()) as usize;

    let mut block_size = None;
    let mut virtual_disk_size = None;
    let mut logical_sector_size = 512u64;

    for i in 0..meta_entry_cnt {
        let off = meta_off + 32 + (i as u64) * 32;
        if file.read_exact_at(&mut entry_buf, off).is_err() {
            break;
        }
        let guid = &entry_buf[0..16];
        let item_off = u32::from_le_bytes(entry_buf[16..20].try_into().unwrap()) as u64;
        let item_len = u32::from_le_bytes(entry_buf[20..24].try_into().unwrap());

        if guid == &FILE_PARAMS_GUID && item_len >= 4 {
            let mut buf = [0u8; 4];
            file.read_exact_at(&mut buf, meta_off + item_off)?;
            block_size = Some(u32::from_le_bytes(buf) as u64);
        } else if guid == &VDISK_SIZE_GUID && item_len >= 8 {
            let mut buf = [0u8; 8];
            file.read_exact_at(&mut buf, meta_off + item_off)?;
            virtual_disk_size = Some(u64::from_le_bytes(buf));
        } else if guid == &SECTOR_SIZE_GUID && item_len >= 4 {
            let mut buf = [0u8; 4];
            file.read_exact_at(&mut buf, meta_off + item_off)?;
            logical_sector_size = u32::from_le_bytes(buf) as u64;
        }
    }

    let b_size = match block_size {
        Some(s) if s > 0 => s,
        _ => return Ok(None),
    };
    let v_size = match virtual_disk_size {
        Some(s) if s > 0 => s,
        _ => return Ok(None),
    };

    let chunk_ratio = ((1u64 << 23) * logical_sector_size) / b_size;
    let bat_entries = (bat_len as usize) / 8;
    let mut bat_bytes = vec![0u8; bat_len as usize];
    file.read_exact_at(&mut bat_bytes, bat_off)?;

    let mut bat = Vec::with_capacity(bat_entries);
    for chunk in bat_bytes.chunks_exact(8) {
        bat.push(u64::from_le_bytes(chunk.try_into().unwrap()));
    }

    println!("[nbd:vhdx] In-Tree Pure Rust loaded Dynamic VHDX: VirtualDiskSize={} MB, BlockSize={} MB, ChunkRatio={}, BAT entries={}",
             v_size / (1024 * 1024), b_size / (1024 * 1024), chunk_ratio, bat.len());

    Ok(Some(VhdxMeta {
        virtual_disk_size: v_size,
        block_size: b_size,
        chunk_ratio,
        bat: Arc::new(bat),
    }))
}

#[derive(Clone, Debug)]
pub struct VmdkMeta {
    pub capacity: u64,
    pub grain_size: u64,
    pub num_gtes: u32,
    pub gd_offset: u64,
    pub gd: Arc<Vec<u32>>,
}

pub fn parse_vmdk_metadata(file: &std::fs::File) -> std::io::Result<Option<VmdkMeta>> {
    use std::os::unix::fs::FileExt;
    let mut hdr = [0u8; 512];
    if file.read_exact_at(&mut hdr, 0).is_err() {
        return Ok(None);
    }
    if &hdr[0..4] != b"KDMV" {
        return Ok(None);
    }
    let cap_sectors = u64::from_le_bytes(hdr[12..20].try_into().unwrap());
    let grain_size_sectors = u64::from_le_bytes(hdr[20..28].try_into().unwrap());
    let num_gtes = u32::from_le_bytes(hdr[44..48].try_into().unwrap());
    let gd_offset_sectors = u64::from_le_bytes(hdr[56..64].try_into().unwrap());

    if grain_size_sectors == 0 || num_gtes == 0 {
        return Ok(None);
    }

    let grain_bytes = grain_size_sectors * 512;
    let gt_coverage = grain_bytes * (num_gtes as u64);
    let num_gt = (cap_sectors * 512 + gt_coverage - 1) / gt_coverage;

    let mut gd_bytes = vec![0u8; (num_gt * 4) as usize];
    file.read_exact_at(&mut gd_bytes, gd_offset_sectors * 512)?;
    let mut gd = Vec::with_capacity(num_gt as usize);
    for chunk in gd_bytes.chunks_exact(4) {
        gd.push(u32::from_le_bytes(chunk.try_into().unwrap()));
    }

    println!("[nbd:vmdk] In-Tree Pure Rust loaded VMDK (monolithicSparse): Capacity={} MB, GrainSize={} KB, NumGTEs={}, GD entries={}",
             (cap_sectors * 512) / (1024 * 1024), grain_bytes / 1024, num_gtes, gd.len());

    Ok(Some(VmdkMeta {
        capacity: cap_sectors * 512,
        grain_size: grain_bytes,
        num_gtes,
        gd_offset: gd_offset_sectors * 512,
        gd: Arc::new(gd),
    }))
}

#[derive(Clone, Debug)]
pub struct VdiMeta {
    pub disk_size: u64,
    pub block_size: u64,
    pub off_data: u64,
    pub bat: Arc<Vec<u32>>,
}

pub fn parse_vdi_metadata(file: &std::fs::File) -> std::io::Result<Option<VdiMeta>> {
    use std::os::unix::fs::FileExt;
    let mut hdr = [0u8; 512];
    if file.read_exact_at(&mut hdr, 0).is_err() {
        return Ok(None);
    }
    // Signature 0xBEDA107F at byte offset 64
    if &hdr[64..68] != &[0x7f, 0x10, 0xda, 0xbe] {
        return Ok(None);
    }
    let off_blocks = u32::from_le_bytes(hdr[340..344].try_into().unwrap()) as u64;
    let off_data = u32::from_le_bytes(hdr[344..348].try_into().unwrap()) as u64;
    let disk_size = u64::from_le_bytes(hdr[368..376].try_into().unwrap());
    let block_size = u32::from_le_bytes(hdr[376..380].try_into().unwrap()) as u64;
    let blocks_total = u32::from_le_bytes(hdr[384..388].try_into().unwrap()) as usize;

    if block_size == 0 || blocks_total == 0 {
        return Ok(None);
    }

    let mut bat_bytes = vec![0u8; blocks_total * 4];
    file.read_exact_at(&mut bat_bytes, off_blocks)?;
    let mut bat = Vec::with_capacity(blocks_total);
    for chunk in bat_bytes.chunks_exact(4) {
        bat.push(u32::from_le_bytes(chunk.try_into().unwrap()));
    }

    println!("[nbd:vdi] In-Tree Pure Rust loaded VDI: DiskSize={} MB, BlockSize={} KB, BlocksTotal={}, OffData={}",
             disk_size / (1024 * 1024), block_size / 1024, blocks_total, off_data);

    Ok(Some(VdiMeta {
        disk_size,
        block_size,
        off_data,
        bat: Arc::new(bat),
    }))
}

#[derive(Clone, Debug)]
pub struct Qcow2Meta {
    pub virtual_disk_size: u64,
    pub cluster_bits: u32,
    pub cluster_size: u64,
    pub l2_bits: u32,
    pub l2_entries: usize,
    pub l1_table: Arc<Vec<u64>>,
}

pub fn parse_qcow2_metadata(file: &std::fs::File) -> std::io::Result<Option<Qcow2Meta>> {
    use std::os::unix::fs::FileExt;
    let mut header = [0u8; 104];
    if file.read_exact_at(&mut header, 0).is_err() {
        return Ok(None);
    }
    if &header[0..4] != b"QFI\xfb" {
        return Ok(None);
    }
    let version = u32::from_be_bytes(header[4..8].try_into().unwrap());
    if version != 2 && version != 3 {
        return Ok(None);
    }
    let cluster_bits = u32::from_be_bytes(header[20..24].try_into().unwrap());
    if cluster_bits < 9 || cluster_bits > 28 {
        return Ok(None);
    }
    let cluster_size = 1u64 << cluster_bits;
    let virtual_disk_size = u64::from_be_bytes(header[24..32].try_into().unwrap());
    let crypt_method = u32::from_be_bytes(header[32..36].try_into().unwrap());
    if crypt_method != 0 {
        eprintln!("[nbd:qcow2] Encrypted QCOW2 is not supported");
        return Ok(None);
    }
    let l1_size = u32::from_be_bytes(header[36..40].try_into().unwrap()) as usize;
    let l1_table_offset = u64::from_be_bytes(header[40..48].try_into().unwrap());

    let mut l1_bytes = vec![0u8; l1_size * 8];
    file.read_exact_at(&mut l1_bytes, l1_table_offset)?;
    let mut l1_table = Vec::with_capacity(l1_size);
    for chunk in l1_bytes.chunks_exact(8) {
        l1_table.push(u64::from_be_bytes(chunk.try_into().unwrap()));
    }

    let l2_bits = cluster_bits - 3;
    let l2_entries = 1usize << l2_bits;

    println!("[nbd:qcow2] In-Tree Pure Rust loaded QCOW2 (v{}): VirtualSize={} MB, ClusterSize={} KB, L1 Entries={}",
             version, virtual_disk_size / (1024 * 1024), cluster_size / 1024, l1_size);

    Ok(Some(Qcow2Meta {
        virtual_disk_size,
        cluster_bits,
        cluster_size,
        l2_bits,
        l2_entries,
        l1_table: Arc::new(l1_table),
    }))
}

#[derive(Clone, Debug)]
pub struct VpcMeta {
    pub virtual_disk_size: u64,
    pub block_size: u64,
    pub bitmap_bytes: u64,
    pub bat: Arc<Vec<u32>>,
}

pub fn parse_vpc_metadata(file: &std::fs::File) -> std::io::Result<Option<VpcMeta>> {
    use std::os::unix::fs::FileExt;
    let file_len = file.metadata()?.len();
    if file_len < 1024 {
        return Ok(None);
    }
    let mut footer = [0u8; 512];
    if file.read_exact_at(&mut footer, 0).is_err() {
        return Ok(None);
    }
    if &footer[0..8] != b"conectix" {
        if file.read_exact_at(&mut footer, file_len - 512).is_err() || &footer[0..8] != b"conectix" {
            return Ok(None);
        }
    }
    let disk_type = u32::from_be_bytes(footer[60..64].try_into().unwrap());
    if disk_type != 3 && disk_type != 4 {
        return Ok(None);
    }
    let virtual_disk_size = u64::from_be_bytes(footer[48..56].try_into().unwrap());

    let mut dyn_hdr = [0u8; 512];
    if file.read_exact_at(&mut dyn_hdr, 512).is_err() {
        return Ok(None);
    }
    if &dyn_hdr[0..8] != b"cxsparse" {
        return Ok(None);
    }
    let table_offset = u64::from_be_bytes(dyn_hdr[16..24].try_into().unwrap());
    let max_entries = u32::from_be_bytes(dyn_hdr[28..32].try_into().unwrap()) as usize;
    let block_size = u32::from_be_bytes(dyn_hdr[32..36].try_into().unwrap()) as u64;
    if block_size == 0 || (block_size % 512) != 0 {
        return Ok(None);
    }

    let bitmap_sectors = ((block_size / 512 / 8) + 511) / 512;
    let bitmap_bytes = bitmap_sectors * 512;

    let mut bat_bytes = vec![0u8; max_entries * 4];
    file.read_exact_at(&mut bat_bytes, table_offset)?;
    let mut bat = Vec::with_capacity(max_entries);
    for chunk in bat_bytes.chunks_exact(4) {
        bat.push(u32::from_be_bytes(chunk.try_into().unwrap()));
    }

    println!("[nbd:vpc] In-Tree Pure Rust loaded Dynamic VHD (VPC): DiskSize={} MB, BlockSize={} KB, BAT entries={}",
             virtual_disk_size / (1024 * 1024), block_size / 1024, max_entries);

    Ok(Some(VpcMeta {
        virtual_disk_size,
        block_size,
        bitmap_bytes,
        bat: Arc::new(bat),
    }))
}

/// In-Tree Pure Rust Copy-On-Write (COW) Storage Driver
/// Supports Direct Raw storage, Fixed VHD, Dynamic VHDX, VMDK, VDI, QCOW2, and Dynamic VHD (VPC).
pub struct CowStorage {
    pub base_file: Arc<std::fs::File>,
    pub diff_file: Option<Arc<std::fs::File>>,
    pub diff_path: Option<String>,
    pub idx_path: Option<String>,
    pub export_size: u64,
    pub block_size: u64,
    pub bitmap: Option<Arc<std::sync::RwLock<Vec<u8>>>>,
    pub is_read_only: bool,
    pub vhdx_meta: Option<VhdxMeta>,
    pub vmdk_meta: Option<VmdkMeta>,
    pub vdi_meta: Option<VdiMeta>,
    pub qcow2_meta: Option<Qcow2Meta>,
    pub vpc_meta: Option<VpcMeta>,
}

impl CowStorage {
    pub fn open_direct(path: &str) -> std::io::Result<(Self, bool)> {
        let (file, is_read_only) = match OpenOptions::new().read(true).write(true).open(path) {
            Ok(f) => (f, false),
            Err(e) => {
                eprintln!("[nbd] Read-write open failed for '{}': {}. Falling back to read-only.", path, e);
                let ro_file = OpenOptions::new().read(true).open(path)?;
                (ro_file, true)
            }
        };

        let vhdx_meta = parse_vhdx_metadata(&file)?;
        let vmdk_meta = if vhdx_meta.is_none() { parse_vmdk_metadata(&file)? } else { None };
        let vdi_meta = if vhdx_meta.is_none() && vmdk_meta.is_none() { parse_vdi_metadata(&file)? } else { None };
        let qcow2_meta = if vhdx_meta.is_none() && vmdk_meta.is_none() && vdi_meta.is_none() { parse_qcow2_metadata(&file)? } else { None };
        let vpc_meta = if vhdx_meta.is_none() && vmdk_meta.is_none() && vdi_meta.is_none() && qcow2_meta.is_none() { parse_vpc_metadata(&file)? } else { None };

        let mut export_size = if let Some(ref meta) = vhdx_meta {
            meta.virtual_disk_size
        } else if let Some(ref meta) = vmdk_meta {
            meta.capacity
        } else if let Some(ref meta) = vdi_meta {
            meta.disk_size
        } else if let Some(ref meta) = qcow2_meta {
            meta.virtual_disk_size
        } else if let Some(ref meta) = vpc_meta {
            meta.virtual_disk_size
        } else {
            file.metadata()?.len()
        };

        if vhdx_meta.is_none() && vmdk_meta.is_none() && vdi_meta.is_none() && qcow2_meta.is_none() && vpc_meta.is_none() && export_size > 512 {
            use std::os::unix::fs::FileExt;
            let mut footer = [0u8; 512];
            if file.read_exact_at(&mut footer, export_size - 512).is_ok() {
                if &footer[0..8] == b"conectix" {
                    let disk_type = u32::from_be_bytes([footer[60], footer[61], footer[62], footer[63]]);
                    if disk_type == 2 {
                        println!("[nbd] Auto-detected Fixed VHD: stripping 512B trailer (file_size={} MB, raw_size={} MB)",
                                 export_size / (1024 * 1024), (export_size - 512) / (1024 * 1024));
                        export_size -= 512;
                    }
                }
            }
        }

        let is_ro = is_read_only || vhdx_meta.is_some() || vmdk_meta.is_some() || vdi_meta.is_some() || qcow2_meta.is_some() || vpc_meta.is_some();
        Ok((Self {
            base_file: Arc::new(file),
            diff_file: None,
            diff_path: None,
            idx_path: None,
            export_size,
            block_size: 4096,
            bitmap: None,
            is_read_only: is_ro,
            vhdx_meta,
            vmdk_meta,
            vdi_meta,
            qcow2_meta,
            vpc_meta,
        }, is_ro))
    }

    pub fn open_cow(base_path: &str, diff_path: &str) -> std::io::Result<Self> {
        let base_file = OpenOptions::new().read(true).open(base_path)?;
        let vhdx_meta = parse_vhdx_metadata(&base_file)?;
        let vmdk_meta = if vhdx_meta.is_none() { parse_vmdk_metadata(&base_file)? } else { None };
        let vdi_meta = if vhdx_meta.is_none() && vmdk_meta.is_none() { parse_vdi_metadata(&base_file)? } else { None };
        let qcow2_meta = if vhdx_meta.is_none() && vmdk_meta.is_none() && vdi_meta.is_none() { parse_qcow2_metadata(&base_file)? } else { None };
        let vpc_meta = if vhdx_meta.is_none() && vmdk_meta.is_none() && vdi_meta.is_none() && qcow2_meta.is_none() { parse_vpc_metadata(&base_file)? } else { None };

        let mut export_size = if let Some(ref meta) = vhdx_meta {
            meta.virtual_disk_size
        } else if let Some(ref meta) = vmdk_meta {
            meta.capacity
        } else if let Some(ref meta) = vdi_meta {
            meta.disk_size
        } else if let Some(ref meta) = qcow2_meta {
            meta.virtual_disk_size
        } else if let Some(ref meta) = vpc_meta {
            meta.virtual_disk_size
        } else {
            base_file.metadata()?.len()
        };

        if vhdx_meta.is_none() && vmdk_meta.is_none() && vdi_meta.is_none() && qcow2_meta.is_none() && vpc_meta.is_none() && export_size > 512 {
            use std::os::unix::fs::FileExt;
            let mut footer = [0u8; 512];
            if base_file.read_exact_at(&mut footer, export_size - 512).is_ok() {
                if &footer[0..8] == b"conectix" {
                    let disk_type = u32::from_be_bytes([footer[60], footer[61], footer[62], footer[63]]);
                    if disk_type == 2 {
                        println!("[nbd:cow] Auto-detected Fixed VHD: stripping 512B trailer (file_size={} MB, raw_size={} MB)",
                                 export_size / (1024 * 1024), (export_size - 512) / (1024 * 1024));
                        export_size -= 512;
                    }
                }
            }
        }

        let diff_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(diff_path)?;

        let cur_diff_len = diff_file.metadata()?.len();
        if cur_diff_len < export_size {
            diff_file.set_len(export_size)?;
        }

        let block_size = 4096u64; // 4 KB block size (standard page/FS block size)
        let num_blocks = (export_size + block_size - 1) / block_size;
        let bitmap_len = ((num_blocks + 7) / 8) as usize;

        let idx_path = format!("{}.idx", diff_path);
        let bitmap_vec = if let Ok(data) = std::fs::read(&idx_path) {
            if data.len() == bitmap_len {
                data
            } else {
                vec![0u8; bitmap_len]
            }
        } else {
            vec![0u8; bitmap_len]
        };

        println!("[nbd:cow] Initialized CowStorage: Base='{}' ({} MB, RO, is_vhdx={}, is_vmdk={}, is_vdi={}, is_qcow2={}, is_vpc={}), Diff='{}' (Raw Sparse, blocks={}, idx='{}')",
                 base_path, export_size / (1024 * 1024), vhdx_meta.is_some(), vmdk_meta.is_some(), vdi_meta.is_some(), qcow2_meta.is_some(), vpc_meta.is_some(), diff_path, num_blocks, idx_path);

        Ok(Self {
            base_file: Arc::new(base_file),
            diff_file: Some(Arc::new(diff_file)),
            diff_path: Some(diff_path.to_string()),
            idx_path: Some(idx_path),
            export_size,
            block_size,
            bitmap: Some(Arc::new(std::sync::RwLock::new(bitmap_vec))),
            is_read_only: false,
            vhdx_meta,
            vmdk_meta,
            vdi_meta,
            qcow2_meta,
            vpc_meta,
        })
    }

    fn read_base_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        use std::os::unix::fs::FileExt;
        if let Some(ref vmdk) = self.vmdk_meta {
            let grain_bytes = vmdk.grain_size;
            let gt_coverage = grain_bytes * (vmdk.num_gtes as u64);
            let mut cur_offset = offset;
            let mut buf_idx = 0;
            let total_len = buf.len();

            while buf_idx < total_len {
                let gt_idx = (cur_offset / gt_coverage) as usize;
                let in_gt = cur_offset % gt_coverage;
                let gte_idx = (in_gt / grain_bytes) as usize;
                let in_grain = in_gt % grain_bytes;
                let chunk_len = ((grain_bytes - in_grain) as usize).min(total_len - buf_idx);

                if gt_idx < vmdk.gd.len() {
                    let gt_sector = vmdk.gd[gt_idx];
                    if gt_sector != 0 {
                        let mut gte_buf = [0u8; 4];
                        if self.base_file.read_exact_at(&mut gte_buf, (gt_sector as u64) * 512 + (gte_idx as u64) * 4).is_ok() {
                            let data_sector = u32::from_le_bytes(gte_buf);
                            if data_sector != 0 {
                                let file_off = (data_sector as u64) * 512 + in_grain;
                                self.base_file.read_exact_at(&mut buf[buf_idx..buf_idx + chunk_len], file_off)?;
                            } else {
                                buf[buf_idx..buf_idx + chunk_len].fill(0);
                            }
                        } else {
                            buf[buf_idx..buf_idx + chunk_len].fill(0);
                        }
                    } else {
                        buf[buf_idx..buf_idx + chunk_len].fill(0);
                    }
                } else {
                    buf[buf_idx..buf_idx + chunk_len].fill(0);
                }

                cur_offset += chunk_len as u64;
                buf_idx += chunk_len;
            }
            Ok(())
        } else if let Some(ref vdi) = self.vdi_meta {
            let block_size = vdi.block_size;
            let mut cur_offset = offset;
            let mut buf_idx = 0;
            let total_len = buf.len();

            while buf_idx < total_len {
                let blk_idx = (cur_offset / block_size) as usize;
                let in_block = cur_offset % block_size;
                let chunk_len = ((block_size - in_block) as usize).min(total_len - buf_idx);

                if blk_idx < vdi.bat.len() {
                    let p_block = vdi.bat[blk_idx];
                    if p_block < 0xFFFFFFFE {
                        let file_off = vdi.off_data + (p_block as u64) * block_size + in_block;
                        self.base_file.read_exact_at(&mut buf[buf_idx..buf_idx + chunk_len], file_off)?;
                    } else {
                        buf[buf_idx..buf_idx + chunk_len].fill(0);
                    }
                } else {
                    buf[buf_idx..buf_idx + chunk_len].fill(0);
                }

                cur_offset += chunk_len as u64;
                buf_idx += chunk_len;
            }
            Ok(())
        } else if let Some(ref vhdx) = self.vhdx_meta {
            let block_size = vhdx.block_size;
            let chunk_ratio = vhdx.chunk_ratio;
            let mut cur_offset = offset;
            let mut buf_idx = 0;
            let total_len = buf.len();

            while buf_idx < total_len {
                let p_idx = cur_offset / block_size;
                let in_block_offset = cur_offset % block_size;
                let block_rem = block_size - in_block_offset;
                let chunk_len = (block_rem as usize).min(total_len - buf_idx);

                let bat_idx = (p_idx + p_idx / chunk_ratio) as usize;
                if bat_idx < vhdx.bat.len() {
                    let entry = vhdx.bat[bat_idx];
                    let state = entry & 0x7;
                    if state == 6 {
                        let file_off = (entry & 0xFFFFFFFFFFF00000) + in_block_offset;
                        self.base_file.read_exact_at(&mut buf[buf_idx..buf_idx + chunk_len], file_off)?;
                    } else {
                        buf[buf_idx..buf_idx + chunk_len].fill(0);
                    }
                } else {
                    buf[buf_idx..buf_idx + chunk_len].fill(0);
                }

                cur_offset += chunk_len as u64;
                buf_idx += chunk_len;
            }
            Ok(())
        } else if let Some(ref qcow2) = self.qcow2_meta {
            let cluster_size = qcow2.cluster_size;
            let cluster_bits = qcow2.cluster_bits;
            let l2_bits = qcow2.l2_bits;
            let l2_entries = qcow2.l2_entries;
            let mut cur_offset = offset;
            let mut buf_idx = 0;
            let total_len = buf.len();

            while buf_idx < total_len {
                let in_cluster = (cur_offset & (cluster_size - 1)) as usize;
                let chunk_len = ((cluster_size as usize) - in_cluster).min(total_len - buf_idx);

                let l1_idx = (cur_offset >> (cluster_bits + l2_bits)) as usize;
                let l2_idx = ((cur_offset >> cluster_bits) as usize) & (l2_entries - 1);

                if l1_idx < qcow2.l1_table.len() {
                    let l1_entry = qcow2.l1_table[l1_idx];
                    let l2_offset = l1_entry & 0x00fffffffffffe00;
                    if l2_offset != 0 {
                        let mut l2_buf = [0u8; 8];
                        if self.base_file.read_exact_at(&mut l2_buf, l2_offset + (l2_idx as u64) * 8).is_ok() {
                            let l2_entry = u64::from_be_bytes(l2_buf);
                            let is_zero = (l2_entry & 1) == 1 && ((l2_entry >> 62) & 1) == 1;
                            let cluster_offset = l2_entry & 0x00fffffffffffe00;
                            if !is_zero && cluster_offset != 0 {
                                self.base_file.read_exact_at(
                                    &mut buf[buf_idx..buf_idx + chunk_len],
                                    cluster_offset + in_cluster as u64,
                                )?;
                            } else {
                                buf[buf_idx..buf_idx + chunk_len].fill(0);
                            }
                        } else {
                            buf[buf_idx..buf_idx + chunk_len].fill(0);
                        }
                    } else {
                        buf[buf_idx..buf_idx + chunk_len].fill(0);
                    }
                } else {
                    buf[buf_idx..buf_idx + chunk_len].fill(0);
                }

                cur_offset += chunk_len as u64;
                buf_idx += chunk_len;
            }
            Ok(())
        } else if let Some(ref vpc) = self.vpc_meta {
            let block_size = vpc.block_size;
            let mut cur_offset = offset;
            let mut buf_idx = 0;
            let total_len = buf.len();

            while buf_idx < total_len {
                let blk_idx = (cur_offset / block_size) as usize;
                let in_block = cur_offset % block_size;
                let chunk_len = ((block_size - in_block) as usize).min(total_len - buf_idx);

                if blk_idx < vpc.bat.len() {
                    let sector_off = vpc.bat[blk_idx];
                    if sector_off != 0xFFFFFFFF {
                        let file_off = (sector_off as u64) * 512 + vpc.bitmap_bytes + in_block;
                        self.base_file.read_exact_at(&mut buf[buf_idx..buf_idx + chunk_len], file_off)?;
                    } else {
                        buf[buf_idx..buf_idx + chunk_len].fill(0);
                    }
                } else {
                    buf[buf_idx..buf_idx + chunk_len].fill(0);
                }

                cur_offset += chunk_len as u64;
                buf_idx += chunk_len;
            }
            Ok(())
        } else {
            self.base_file.read_exact_at(buf, offset)
        }
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        if self.diff_file.is_none() {
            return self.read_base_at(buf, offset);
        }

        let diff_file = self.diff_file.as_ref().unwrap();
        let bitmap_lock = self.bitmap.as_ref().unwrap().read().unwrap();
        let block_size = self.block_size;
        let start_block = offset / block_size;
        let end_block = (offset + buf.len() as u64 + block_size - 1) / block_size;

        let mut all_base = true;
        let mut all_diff = true;
        for b in start_block..end_block {
            let byte_idx = (b / 8) as usize;
            let bit_idx = (b % 8) as u8;
            let written = if byte_idx < bitmap_lock.len() {
                (bitmap_lock[byte_idx] & (1 << bit_idx)) != 0
            } else {
                false
            };
            if written {
                all_base = false;
            } else {
                all_diff = false;
            }
        }

        if all_base {
            drop(bitmap_lock);
            return self.read_base_at(buf, offset);
        }
        if all_diff {
            drop(bitmap_lock);
            return diff_file.read_exact_at(buf, offset);
        }

        let mut cur_offset = offset;
        let mut buf_idx = 0;
        let total_len = buf.len();

        while buf_idx < total_len {
            let b = cur_offset / block_size;
            let block_end = (b + 1) * block_size;
            let chunk_len = ((block_end - cur_offset) as usize).min(total_len - buf_idx);

            let byte_idx = (b / 8) as usize;
            let bit_idx = (b % 8) as u8;
            let written = if byte_idx < bitmap_lock.len() {
                (bitmap_lock[byte_idx] & (1 << bit_idx)) != 0
            } else {
                false
            };

            let slice = &mut buf[buf_idx..buf_idx + chunk_len];
            if written {
                diff_file.read_exact_at(slice, cur_offset)?;
            } else {
                self.read_base_at(slice, cur_offset)?;
            }

            cur_offset += chunk_len as u64;
            buf_idx += chunk_len;
        }

        Ok(())
    }

    pub fn write_at(&self, data: &[u8], offset: u64) -> std::io::Result<()> {
        if self.diff_file.is_none() {
            if self.is_read_only {
                return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Read-only export"));
            }
            if self.vhdx_meta.is_some() {
                return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Direct write to Dynamic VHDX without COW layer is prohibited"));
            }
            return self.base_file.write_all_at(data, offset);
        }

        let diff_file = self.diff_file.as_ref().unwrap();
        let block_size = self.block_size;
        let mut cur_offset = offset;
        let mut buf_idx = 0;
        let total_len = data.len();

        let mut bitmap_lock = self.bitmap.as_ref().unwrap().write().unwrap();

        while buf_idx < total_len {
            let b = cur_offset / block_size;
            let block_start = b * block_size;
            let block_end = block_start + block_size;
            let chunk_len = ((block_end - cur_offset) as usize).min(total_len - buf_idx);
            let chunk = &data[buf_idx..buf_idx + chunk_len];

            let byte_idx = (b / 8) as usize;
            let bit_idx = (b % 8) as u8;
            let already_in_diff = if byte_idx < bitmap_lock.len() {
                (bitmap_lock[byte_idx] & (1 << bit_idx)) != 0
            } else {
                false
            };

            if already_in_diff {
                // Block already modified in diff layer: write chunk directly
                diff_file.write_all_at(chunk, cur_offset)?;
            } else if cur_offset == block_start && chunk_len == block_size as usize {
                // Full block overwrite: write directly without reading base
                diff_file.write_all_at(chunk, block_start)?;
                if byte_idx < bitmap_lock.len() {
                    bitmap_lock[byte_idx] |= 1 << bit_idx;
                }
            } else {
                // Partial block on first modification: True Copy-On-Write
                // Read base block, overlay chunk, and commit full block to diff file
                let read_len = (self.export_size.saturating_sub(block_start) as usize).min(block_size as usize);
                let mut full_block = vec![0u8; read_len];
                let _ = self.read_base_at(&mut full_block, block_start);
                let rel_offset = (cur_offset - block_start) as usize;
                full_block[rel_offset..rel_offset + chunk_len].copy_from_slice(chunk);
                diff_file.write_all_at(&full_block, block_start)?;
                if byte_idx < bitmap_lock.len() {
                    bitmap_lock[byte_idx] |= 1 << bit_idx;
                }
            }

            cur_offset += chunk_len as u64;
            buf_idx += chunk_len;
        }

        Ok(())
    }

    pub fn trim(&self, offset: u64, length: u32) -> std::io::Result<()> {
        if let Some(diff_file) = &self.diff_file {
            let block_size = self.block_size;
            let start_block = (offset + block_size - 1) / block_size;
            let end_block = (offset + length as u64) / block_size;
            if start_block < end_block {
                let mut bitmap_lock = self.bitmap.as_ref().unwrap().write().unwrap();
                for b in start_block..end_block {
                    let byte_idx = (b / 8) as usize;
                    let bit_idx = (b % 8) as u8;
                    if byte_idx < bitmap_lock.len() {
                        bitmap_lock[byte_idx] &= !(1 << bit_idx);
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                use std::os::unix::io::AsRawFd;
                let fd = diff_file.as_raw_fd();
                let _ = unsafe {
                    libc::fallocate(fd, 0x03, offset as libc::off_t, length as libc::off_t)
                };
            }
        }
        Ok(())
    }

    pub fn sync_data(&self) -> std::io::Result<()> {
        if let Some(diff_file) = &self.diff_file {
            diff_file.sync_data()?;
            if let Some(idx_p) = &self.idx_path {
                if let Some(bm) = &self.bitmap {
                    let data = bm.read().unwrap().clone();
                    let _ = std::fs::write(idx_p, &data);
                }
            }
        } else {
            self.base_file.sync_data()?;
        }
        Ok(())
    }
}

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
        let (storage, is_read_only) = CowStorage::open_direct(path)?;
        Self::start_with_storage(port, Arc::new(storage), path, export_name, is_read_only).await
    }

    pub async fn start_cow(port: u16, base_path: &str, diff_path: &str, export_name: &str) -> std::io::Result<Self> {
        let storage = CowStorage::open_cow(base_path, diff_path)?;
        Self::start_with_storage(port, Arc::new(storage), base_path, export_name, false).await
    }

    async fn start_with_storage(
        port: u16,
        storage: Arc<CowStorage>,
        path_str: &str,
        export_name: &str,
        is_read_only: bool,
    ) -> std::io::Result<Self> {
        let export_size = storage.export_size;
        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        let srv_port = port;
        let srv_path = path_str.to_string();
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
                                let st = storage.clone();
                                let exp_name = srv_name_srv.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = handle_connection(stream, peer_addr, st, export_size, exp_name, is_read_only).await {
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
    storage: Arc<CowStorage>,
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

                return run_transmission_loop(stream, peer_addr, storage, export_size, is_read_only).await;
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
                    return run_transmission_loop(stream, peer_addr, storage, export_size, is_read_only).await;
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
    storage: Arc<CowStorage>,
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

                let st = storage.clone();
                let len = length as usize;
                let read_res = tokio::task::spawn_blocking(move || {
                    let mut buf = vec![0u8; len];
                    match st.read_at(&mut buf, offset) {
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

                let st = storage.clone();
                let fua = (flags & 1) != 0;
                let write_res = tokio::task::spawn_blocking(move || {
                    let r = st.write_at(&data, offset);
                    if r.is_ok() && fua {
                        let _ = st.sync_data();
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
                let st = storage.clone();
                let sync_res = tokio::task::spawn_blocking(move || st.sync_data()).await;
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

                let st = storage.clone();
                let trim_res = tokio::task::spawn_blocking(move || st.trim(offset, length)).await;

                match trim_res {
                    Ok(Ok(())) => {
                        send_simple_reply(&mut writer, 0, handle).await?;
                    }
                    _ => {
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
