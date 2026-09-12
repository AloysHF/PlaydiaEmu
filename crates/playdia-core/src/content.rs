//! Content loading: raw/cooked CDS-XA disc images + BIOS EPROM.

use crate::state::crc32;
use thiserror::Error;

pub const RAW_SECTOR: usize = 2352;
pub const COOKED_SECTOR: usize = 2048;
pub const BIOS_SIZE: usize = 0x8_0000;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("empty content")]
    Empty,
    #[error("unsupported disc size {0}")]
    BadDiscSize(usize),
    #[error("bios must be {BIOS_SIZE} bytes, got {0}")]
    BadBiosSize(usize),
}

/// Parsed disc image (raw 2352 or cooked 2048).
#[derive(Debug, Clone)]
pub struct DiscImage {
    pub data: Vec<u8>,
    pub raw: bool,
    pub total_sectors: u32,
    pub crc: u32,
}

impl DiscImage {
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, LoadError> {
        if bytes.is_empty() {
            return Err(LoadError::Empty);
        }
        let (raw, total) = if bytes.len().is_multiple_of(RAW_SECTOR) && bytes.len() >= RAW_SECTOR {
            (true, (bytes.len() / RAW_SECTOR) as u32)
        } else if bytes.len().is_multiple_of(COOKED_SECTOR) {
            (false, (bytes.len() / COOKED_SECTOR) as u32)
        } else {
            return Err(LoadError::BadDiscSize(bytes.len()));
        };
        let crc = crc32(&bytes);
        Ok(Self {
            data: bytes,
            raw,
            total_sectors: total,
            crc,
        })
    }

    pub fn from_path(path: &std::path::Path) -> Result<Self, LoadError> {
        let data = std::fs::read(path)?;
        Self::from_bytes(data)
    }

    /// Byte offset of LBA (ISO LBA 0 = MSF 00:02:00 for raw).
    pub fn sector_offset(&self, lba: u32) -> Option<usize> {
        let idx = lba as usize;
        let bps = if self.raw { RAW_SECTOR } else { COOKED_SECTOR };
        let off = idx.checked_mul(bps)?;
        if off + bps <= self.data.len() {
            Some(off)
        } else {
            None
        }
    }

    pub fn read_sector(&self, lba: u32) -> Option<Sector> {
        let off = self.sector_offset(lba)?;
        let bps = if self.raw { RAW_SECTOR } else { COOKED_SECTOR };
        let raw = &self.data[off..off + bps];
        if self.raw {
            parse_raw_sector(raw)
        } else {
            Some(Sector {
                mode: 2,
                form: 2,
                subheader: [0; 8],
                file_id: 0,
                channel_id: 0,
                submode: 0,
                coding: 0,
                data: raw.to_vec(),
                lba,
            })
        }
    }
}

#[derive(Debug, Clone)]
pub struct Sector {
    pub mode: u8,
    pub form: u8,
    pub subheader: [u8; 8],
    pub file_id: u8,
    pub channel_id: u8,
    pub submode: u8,
    pub coding: u8,
    pub data: Vec<u8>,
    pub lba: u32,
}

impl Sector {
    pub fn is_video(&self) -> bool {
        self.file_id == 0x61
    }

    pub fn is_audio(&self) -> bool {
        self.file_id == 0x62
    }

    pub fn is_realtime(&self) -> bool {
        self.submode & 0x04 != 0
    }

    pub fn is_end(&self) -> bool {
        self.submode & 0x80 != 0
    }
}

pub fn parse_raw_sector(raw: &[u8]) -> Option<Sector> {
    if raw.len() < RAW_SECTOR {
        return None;
    }
    let mode = raw[15];
    // Subheader at 16..24 (file, channel, submode, coding + pad).
    // Mode 2 Form 1/2 distinguished by submode bit 5 (0x20).
    let sub = &raw[16..24];
    let file_id = sub[0];
    let channel_id = sub[1];
    let submode = sub[2];
    let coding = sub[3];
    // 2352-byte raw sector:
    //  0..11 sync, 12..15 header, 16..23 subheader
    //  Form2 user payload: 24 .. 24+2324 (EDC in last 4 of payload zone)
    //  Form1 user payload: 24 .. 24+2048
    let (form, data_off, data_len) = if mode == 1 {
        (1u8, 16usize, 2048usize)
    } else if submode & 0x20 != 0 {
        (2u8, 24usize, 2324usize)
    } else {
        (1u8, 24usize, 2048usize)
    };
    let end = data_off.checked_add(data_len)?;
    if end > raw.len() {
        return None;
    }
    let data = raw[data_off..end].to_vec();
    Some(Sector {
        mode,
        form,
        subheader: sub.try_into().ok()?,
        file_id,
        channel_id,
        submode,
        coding,
        data,
        lba: 0,
    })
}

/// Load BIOS EPROM. `allow_placeholder` permits zero-filled 512 KiB for tests only.
pub fn load_bios(bytes: &[u8], allow_placeholder: bool) -> Result<Vec<u8>, LoadError> {
    if bytes.len() == BIOS_SIZE {
        return Ok(bytes.to_vec());
    }
    if allow_placeholder && bytes.is_empty() {
        return Ok(vec![0; BIOS_SIZE]);
    }
    Err(LoadError::BadBiosSize(bytes.len()))
}
