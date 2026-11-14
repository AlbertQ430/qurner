//! 无外部依赖的 Stored ZIP 写入器。
//!
//! 手写 ZIP 结构以便完全控制条目顺序、重复名称和不安全文件名字节；
//! 仅用于测试夹具，不进入产品代码。

pub struct ZipEntry {
    name: String,
    data: Vec<u8>,
}

impl ZipEntry {
    pub fn new(name: impl Into<String>, data: impl Into<Vec<u8>>) -> Self {
        Self {
            name: name.into(),
            data: data.into(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

const LOCAL_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const CENTRAL_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const END_OF_CENTRAL_DIRECTORY_SIGNATURE: u32 = 0x0605_4b50;
const UTF8_FLAG: u16 = 0x0800;
const STORED_METHOD: u16 = 0;
const DOS_DATE_1980_01_01: u16 = 0x0021;

pub fn build_zip(entries: &[ZipEntry]) -> Vec<u8> {
    assert!(
        entries.len() <= usize::from(u16::MAX),
        "ZIP 条目数超过 65535"
    );

    let mut output = Vec::new();
    let mut central = Vec::new();
    let mut offset = 0_u32;

    for entry in entries {
        let name = entry.name.as_bytes();
        assert!(
            name.len() <= usize::from(u16::MAX),
            "ZIP 条目名超过 65535 字节"
        );
        assert!(
            entry.data.len() <= u32::MAX as usize,
            "ZIP 条目数据超过 4 GiB"
        );

        let crc = crc32(&entry.data);
        let size = entry.data.len() as u32;

        write_u32(&mut output, LOCAL_HEADER_SIGNATURE);
        write_u16(&mut output, 20);
        write_u16(&mut output, UTF8_FLAG);
        write_u16(&mut output, STORED_METHOD);
        write_u16(&mut output, 0);
        write_u16(&mut output, DOS_DATE_1980_01_01);
        write_u32(&mut output, crc);
        write_u32(&mut output, size);
        write_u32(&mut output, size);
        write_u16(&mut output, name.len() as u16);
        write_u16(&mut output, 0);
        output.extend_from_slice(name);
        output.extend_from_slice(&entry.data);

        write_u32(&mut central, CENTRAL_HEADER_SIGNATURE);
        write_u16(&mut central, 20);
        write_u16(&mut central, 20);
        write_u16(&mut central, UTF8_FLAG);
        write_u16(&mut central, STORED_METHOD);
        write_u16(&mut central, 0);
        write_u16(&mut central, DOS_DATE_1980_01_01);
        write_u32(&mut central, crc);
        write_u32(&mut central, size);
        write_u32(&mut central, size);
        write_u16(&mut central, name.len() as u16);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, 0);
        write_u32(&mut central, offset);
        central.extend_from_slice(name);

        offset += 30 + name.len() as u32 + size;
    }

    let central_offset = offset;
    let central_size = central.len() as u32;
    output.extend_from_slice(&central);

    write_u32(&mut output, END_OF_CENTRAL_DIRECTORY_SIGNATURE);
    write_u16(&mut output, 0);
    write_u16(&mut output, 0);
    write_u16(&mut output, entries.len() as u16);
    write_u16(&mut output, entries.len() as u16);
    write_u32(&mut output, central_size);
    write_u32(&mut output, central_offset);
    write_u16(&mut output, 0);

    output
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn write_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic(expected = "ZIP 条目数超过 65535")]
    fn rejects_more_entries_than_zip_limit() {
        let entries: Vec<ZipEntry> = (0..=usize::from(u16::MAX))
            .map(|index| ZipEntry::new(format!("entry-{index}.txt"), Vec::new()))
            .collect();
        let _ = build_zip(&entries);
    }
}
