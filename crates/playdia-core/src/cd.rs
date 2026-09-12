//! CDS-XA / ISO9660 helpers and stream demux.

use crate::content::{DiscImage, Sector, COOKED_SECTOR, RAW_SECTOR};

/// Streaming demultiplexer for PID 0x61 video / 0x62 audio.
#[derive(Debug, Default, Clone)]
pub struct XaDemux {
    pub video_sectors: u64,
    pub audio_sectors: u64,
    pub other_sectors: u64,
    pub last_video_lba: Option<u32>,
    pub last_audio_lba: Option<u32>,
    pub end_flag: bool,
}

impl XaDemux {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn push(&mut self, sector: &Sector) -> XaPacket {
        if sector.is_end() {
            self.end_flag = true;
        }
        if sector.is_video() {
            self.video_sectors += 1;
            self.last_video_lba = Some(sector.lba);
            XaPacket::Video {
                lba: sector.lba,
                channel: sector.channel_id,
                submode: sector.submode,
                coding: sector.coding,
                data: sector.data.clone(),
            }
        } else if sector.is_audio() {
            self.audio_sectors += 1;
            self.last_audio_lba = Some(sector.lba);
            XaPacket::Audio {
                lba: sector.lba,
                channel: sector.channel_id,
                submode: sector.submode,
                coding: sector.coding,
                data: sector.data.clone(),
            }
        } else {
            self.other_sectors += 1;
            XaPacket::Other {
                lba: sector.lba,
                file_id: sector.file_id,
                data_len: sector.data.len(),
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum XaPacket {
    Video {
        lba: u32,
        channel: u8,
        submode: u8,
        coding: u8,
        data: Vec<u8>,
    },
    Audio {
        lba: u32,
        channel: u8,
        submode: u8,
        coding: u8,
        data: Vec<u8>,
    },
    Other {
        lba: u32,
        file_id: u8,
        data_len: usize,
    },
}

/// Scan a disc for XA file content statistics.
pub fn scan_xa(disc: &DiscImage) -> XaDemux {
    let mut demux = XaDemux::new();
    if !disc.raw {
        return demux;
    }
    for lba in 0..disc.total_sectors {
        if let Some(sec) = disc.read_sector(lba) {
            let mut sec = sec;
            sec.lba = lba;
            demux.push(&sec);
        }
    }
    demux
}

pub fn sector_bytes(disc: &DiscImage) -> usize {
    if disc.raw {
        RAW_SECTOR
    } else {
        COOKED_SECTOR
    }
}
