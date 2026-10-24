use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;

pub static BASELINE_BCD: &[u8] = include_bytes!("static/baseline_bcd.bin");

// Golden baseline GUIDs embedded inside static/baseline_bcd.bin
pub const BASELINE_DISK_GUID: &str = "2DDB2FBE-437C-4F76-81E1-0AD8964DD2E6";
pub const BASELINE_PART_GUID: &str = "4F058CE4-322B-4E87-88C9-67E3D6DA20A9";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Guid(pub [u8; 16]);

impl Guid {
    pub const ZERO: Guid = Guid([0u8; 16]);

    // EFI System Partition (ESP): C12A7328-F81F-11D2-BA4B-00A0C93EC93B
    pub const ESP: Guid = Guid([
        0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
    ]);

    // Microsoft Basic Data: EBD0A0A2-B9E5-4433-87C0-68B6B72699C7
    pub const MS_BASIC_DATA: Guid = Guid([
        0xa2, 0xa0, 0xd0, 0xeb, 0xe5, 0xb9, 0x33, 0x44, 0x87, 0xc0, 0x68, 0xb6, 0xb7, 0x26, 0x99, 0xc7,
    ]);

    // Microsoft Reserved (MSR): E3C9E316-0B5C-4DB8-817D-F92DF00215AE
    pub const MS_RESERVED: Guid = Guid([
        0x16, 0xe3, 0xc9, 0xe3, 0x5c, 0x0b, 0xb8, 0x4d, 0x81, 0x7d, 0xf9, 0x2d, 0xf0, 0x02, 0x15, 0xae,
    ]);

    // Linux Filesystem: 0FC63DAF-8483-4772-8E79-3D69D8477DE4
    pub const LINUX_FS: Guid = Guid([
        0xaf, 0x3d, 0xc6, 0x0f, 0x83, 0x84, 0x72, 0x47, 0x8e, 0x79, 0x3d, 0x69, 0xd8, 0x47, 0x7d, 0xe4,
    ]);

    pub fn from_string(s: &str) -> Option<Self> {
        let clean = s.replace('-', "");
        if clean.len() != 32 {
            return None;
        }
        let mut raw = [0u8; 16];
        for i in 0..16 {
            raw[i] = u8::from_str_radix(&clean[i * 2..i * 2 + 2], 16).ok()?;
        }
        // Mixed endian: [0..4] LE, [4..6] LE, [6..8] LE, [8..16] BE
        let mut out = [0u8; 16];
        out[0] = raw[3];
        out[1] = raw[2];
        out[2] = raw[1];
        out[3] = raw[0];

        out[4] = raw[5];
        out[5] = raw[4];

        out[6] = raw[7];
        out[7] = raw[6];

        out[8..16].copy_from_slice(&raw[8..16]);
        Some(Guid(out))
    }

    pub fn to_string(&self) -> String {
        let b = &self.0;
        format!(
            "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            b[3], b[2], b[1], b[0],
            b[5], b[4],
            b[7], b[6],
            b[8], b[9],
            b[10], b[11], b[12], b[13], b[14], b[15]
        )
    }
}

#[derive(Clone, Debug)]
pub struct GptPartition {
    pub index: usize,
    pub type_guid: Guid,
    pub unique_guid: Guid,
    pub start_lba: u64,
    pub end_lba: u64,
    pub flags: u64,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct GptDiskInfo {
    pub disk_guid: Guid,
    pub has_esp: bool,
    pub partitions: Vec<GptPartition>,
    pub windows_partition: Option<GptPartition>,
}

/// Reads the first `num_lbas` (each 512 bytes) from raw, VHD, or dynamic VHDX.
pub fn read_disk_lbas(path: &str, num_lbas: usize) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let total_bytes = num_lbas * 512;
    let mut buf = vec![0u8; total_bytes];

    let vhdx_meta = crate::nbd_server::parse_vhdx_metadata(&file)?;
    if let Some(ref vhdx) = vhdx_meta {
        let block_size = vhdx.block_size;
        let chunk_ratio = vhdx.chunk_ratio;
        let mut cur_offset = 0u64;
        let mut buf_idx = 0;
        while buf_idx < total_bytes {
            let p_idx = cur_offset / block_size;
            let in_block_offset = cur_offset % block_size;
            let block_rem = block_size - in_block_offset;
            let chunk_len = (block_rem as usize).min(total_bytes - buf_idx);
            let bat_idx = (p_idx + p_idx / chunk_ratio) as usize;
            if bat_idx < vhdx.bat.len() {
                let entry = vhdx.bat[bat_idx];
                let state = entry & 0x7;
                if state == 6 {
                    let file_off = (entry & 0xFFFFFFFFFFF00000) + in_block_offset;
                    file.read_exact_at(&mut buf[buf_idx..buf_idx + chunk_len], file_off)?;
                } else {
                    buf[buf_idx..buf_idx + chunk_len].fill(0);
                }
            } else {
                buf[buf_idx..buf_idx + chunk_len].fill(0);
            }
            cur_offset += chunk_len as u64;
            buf_idx += chunk_len;
        }
    } else {
        file.read_exact_at(&mut buf, 0)?;
    }

    Ok(buf)
}

/// Parses GPT header and partition entries from any disk image (Raw, VHD, VHDX).
pub fn inspect_disk_gpt(path: &str) -> io::Result<Option<GptDiskInfo>> {
    let data = match read_disk_lbas(path, 34) {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };
    if data.len() < 17408 {
        return Ok(None);
    }

    // LBA 1 (GPT Header) is at offset 512..1024
    if &data[512..520] != b"EFI PART" {
        return Ok(None);
    }

    let mut disk_guid_bytes = [0u8; 16];
    disk_guid_bytes.copy_from_slice(&data[568..584]);
    let disk_guid = Guid(disk_guid_bytes);

    let start_lba = u64::from_le_bytes(data[584..592].try_into().unwrap());
    let num_entries = u32::from_le_bytes(data[592..596].try_into().unwrap()) as usize;
    let entry_size = u32::from_le_bytes(data[596..600].try_into().unwrap()) as usize;

    if entry_size < 128 || start_lba != 2 {
        return Ok(None);
    }

    let mut partitions = Vec::new();
    let mut has_esp = false;
    let mut win_part = None;

    let part_start_offset = 1024;
    for i in 0..num_entries.min(128) {
        let off = part_start_offset + i * entry_size;
        if off + 128 > data.len() {
            break;
        }
        let entry = &data[off..off + 128];
        let mut type_bytes = [0u8; 16];
        type_bytes.copy_from_slice(&entry[0x00..0x10]);
        let type_guid = Guid(type_bytes);
        if type_guid == Guid::ZERO {
            continue;
        }

        let mut uniq_bytes = [0u8; 16];
        uniq_bytes.copy_from_slice(&entry[0x10..0x20]);
        let uniq_guid = Guid(uniq_bytes);

        let start_lba = u64::from_le_bytes(entry[0x20..0x28].try_into().unwrap());
        let end_lba = u64::from_le_bytes(entry[0x28..0x30].try_into().unwrap());
        let flags = u64::from_le_bytes(entry[0x30..0x38].try_into().unwrap());

        let name_bytes = &entry[0x38..0x80];
        let mut name_u16 = Vec::new();
        for chunk in name_bytes.chunks_exact(2) {
            let ch = u16::from_le_bytes([chunk[0], chunk[1]]);
            if ch == 0 {
                break;
            }
            name_u16.push(ch);
        }
        let name = String::from_utf16_lossy(&name_u16);

        if type_guid == Guid::ESP {
            has_esp = true;
        }

        let p = GptPartition {
            index: i + 1,
            type_guid,
            unique_guid: uniq_guid,
            start_lba,
            end_lba,
            flags,
            name,
        };

        if type_guid == Guid::MS_BASIC_DATA && win_part.is_none() {
            win_part = Some(p.clone());
        }

        partitions.push(p);
    }

    Ok(Some(GptDiskInfo {
        disk_guid,
        has_esp,
        partitions,
        windows_partition: win_part,
    }))
}

/// In-place byte replacement of 16-byte GUID occurrences in BCD registry hive data.
pub fn patch_bcd_bytes(
    template_bcd: &[u8],
    orig_disk: Guid,
    orig_part: Guid,
    target_disk: Guid,
    target_part: Guid,
) -> (Vec<u8>, usize, usize) {
    let mut out = template_bcd.to_vec();
    let mut disk_count = 0;
    let mut part_count = 0;

    let orig_d = orig_disk.0;
    let orig_p = orig_part.0;
    let tgt_d = target_disk.0;
    let tgt_p = target_part.0;

    let len = out.len();
    if len >= 16 {
        let mut i = 0;
        while i + 16 <= len {
            if &out[i..i + 16] == &orig_d {
                out[i..i + 16].copy_from_slice(&tgt_d);
                disk_count += 1;
                i += 16;
            } else if &out[i..i + 16] == &orig_p {
                out[i..i + 16].copy_from_slice(&tgt_p);
                part_count += 1;
                i += 16;
            } else {
                i += 1;
            }
        }
    }

    (out, disk_count, part_count)
}

/// Generates a synthesized BCD hive dynamically tailored for the given disk image.
pub fn generate_patched_bcd_for_image(image_path: &str) -> io::Result<(Vec<u8>, Guid, Guid)> {
    let info = inspect_disk_gpt(image_path)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("Failed to read GPT from '{}'", image_path)))?;

    let win_part = info.windows_partition
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("No Microsoft Basic Data partition found in '{}'", image_path)))?;

    let orig_disk = Guid::from_string(BASELINE_DISK_GUID).unwrap();
    let orig_part = Guid::from_string(BASELINE_PART_GUID).unwrap();

    let target_disk = info.disk_guid;
    let target_part = win_part.unique_guid;

    let (patched_bcd, disk_refs, part_refs) = patch_bcd_bytes(BASELINE_BCD, orig_disk, orig_part, target_disk, target_part);
    println!("[bcd] Patched in-tree BCD for '{}': Disk GUID={} ({} refs), Windows Part GUID={} ({} refs)",
             image_path, target_disk.to_string(), disk_refs, target_part.to_string(), part_refs);

    Ok((patched_bcd, target_disk, target_part))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guid_roundtrip() {
        let s = "2DDB2FBE-437C-4F76-81E1-0AD8964DD2E6";
        let guid = Guid::from_string(s).unwrap();
        assert_eq!(guid.to_string(), s);

        let esp = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
        assert_eq!(Guid::ESP.to_string(), esp);
    }

    #[test]
    fn test_inspect_gpt_win8() {
        let p = "/mnt/ax9/vms/win8_golden.img";
        if std::path::Path::new(p).exists() {
            let info = inspect_disk_gpt(p).unwrap().expect("should parse gpt");
            assert_eq!(info.disk_guid.to_string(), "B0AEDE45-7B48-4BDC-AFE3-C07BBA593BD8");
            assert!(info.has_esp);
            assert!(info.windows_partition.is_some());
            let win = info.windows_partition.unwrap();
            assert_eq!(win.unique_guid.to_string(), "DF351A3D-33FB-47C0-9EB7-29C91B8CD4F4");
        }
    }

    #[test]
    fn test_inspect_gpt_25h2_vhdx() {
        let p = "/run/media/namobuddha/2tbnvV/25h2.vhdx";
        if std::path::Path::new(p).exists() {
            let info = inspect_disk_gpt(p).unwrap().expect("should parse dynamic vhdx gpt");
            assert_eq!(info.disk_guid.to_string(), "F25B6A18-D2D8-4E30-ACC2-0561A973D3AA");
            assert!(info.has_esp);
        }
    }

    #[test]
    fn test_patch_bcd_pipeline() {
        let orig_d = Guid::from_string(BASELINE_DISK_GUID).unwrap();
        let orig_p = Guid::from_string(BASELINE_PART_GUID).unwrap();
        let tgt_d = Guid::from_string("11111111-2222-3333-4444-555555555555").unwrap();
        let tgt_p = Guid::from_string("AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE").unwrap();

        let (patched, c_d, c_p) = patch_bcd_bytes(BASELINE_BCD, orig_d, orig_p, tgt_d, tgt_p);
        assert_eq!(c_d, 5);
        assert_eq!(c_p, 5);
        assert_eq!(patched.len(), BASELINE_BCD.len());
    }
}
