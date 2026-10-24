use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskFormat {
    Raw,
    Qcow2,
    Vpc, // VHD
    Vhdx,
    Vmdk,
    Iso,
}

impl DiskFormat {
    pub fn as_qemu_arg(&self) -> &'static str {
        match self {
            DiskFormat::Raw => "raw",
            DiskFormat::Qcow2 => "qcow2",
            DiskFormat::Vpc => "vpc",
            DiskFormat::Vhdx => "vhdx",
            DiskFormat::Vmdk => "vmdk",
            DiskFormat::Iso => "raw",
        }
    }
}

/// Detects virtual disk format from binary Magic Header
pub async fn detect_disk_format(image_path: &str) -> DiskFormat {
    use tokio::io::AsyncReadExt;
    let path = Path::new(image_path);
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(_) => return fallback_by_extension(image_path),
    };

    let mut header = [0u8; 512];
    let n = file.read(&mut header).await.unwrap_or(0);
    if n >= 4 && &header[0..4] == b"QFI\xFB" {
        return DiskFormat::Qcow2;
    }
    if n >= 8 && &header[0..8] == b"conectix" {
        return DiskFormat::Vpc;
    }
    if n >= 8 && &header[0..8] == b"vhdxfile" {
        return DiskFormat::Vhdx;
    }
    if n >= 4 && &header[0..4] == b"KDMV" {
        return DiskFormat::Vmdk;
    }

    // Check VHD footer if header is raw (legacy VHD has 512-byte footer at end of file)
    if let Ok(meta) = file.metadata().await {
        let len = meta.len();
        if len > 512 {
            use tokio::io::AsyncSeekExt;
            if file.seek(std::io::SeekFrom::Start(len - 512)).await.is_ok() {
                let mut footer = [0u8; 512];
                if let Ok(fn_) = file.read(&mut footer).await {
                    if fn_ >= 8 && &footer[0..8] == b"conectix" {
                        let disk_type = u32::from_be_bytes([footer[60], footer[61], footer[62], footer[63]]);
                        if disk_type == 2 {
                            // Fixed VHD: Zero-offset raw payload with trailing footer -> Native Raw
                            return DiskFormat::Raw;
                        }
                        return DiskFormat::Vpc;
                    }
                }
            }
        }
    }

    fallback_by_extension(image_path)
}

fn fallback_by_extension(image_path: &str) -> DiskFormat {
    let lower = image_path.to_ascii_lowercase();
    if lower.ends_with(".qcow2") {
        DiskFormat::Qcow2
    } else if lower.ends_with(".vhd") || lower.ends_with(".vhd.vtoy") {
        DiskFormat::Vpc
    } else if lower.ends_with(".vhdx") {
        DiskFormat::Vhdx
    } else if lower.ends_with(".vmdk") {
        DiskFormat::Vmdk
    } else if lower.ends_with(".iso") {
        DiskFormat::Iso
    } else {
        DiskFormat::Raw
    }
}

pub fn is_windows_image(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    if l.contains("linux") || l.contains("cos") || l.contains("artix") || l.contains("antix") || l.contains("ubuntu") || l.contains("debian") || l.contains("arch") {
        return false;
    }
    l.contains("win") || l.contains("windows") || l.ends_with(".vhd") || l.ends_with(".vhdx")
}

/// Checks if the disk image has a partition table (MBR or GPT).
pub async fn has_partition_table(image_path: &str) -> bool {
    use tokio::io::AsyncReadExt;
    let mut buf = [0u8; 520];
    let f = tokio::fs::File::open(Path::new(image_path)).await;
    let mut f = match f {
        Ok(f) => f,
        Err(_) => return false,
    };
    let n = match f.read(&mut buf).await {
        Ok(n) => n,
        Err(_) => return false,
    };
    if n >= 8 && &buf[0..8] == b"vhdxfile" {
        return true;
    }
    if is_windows_image(image_path) {
        return true;
    }
    if n >= 512 && buf[510] == 0x55 && buf[511] == 0xAA {
        for i in 0..4 {
            let base = 446 + i * 16;
            let boot_flag = buf[base];
            let part_type = buf[base + 4];
            if (boot_flag == 0x00 || boot_flag == 0x80) && part_type != 0x00 {
                return true;
            }
        }
    }
    if n >= 520 && &buf[512..520] == b"EFI PART" {
        return true;
    }
    false
}
