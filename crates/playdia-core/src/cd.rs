//! CDS-XA / ISO9660 helpers and stream demux.
//!
//! Playdia routing (from hardware/software behavior evidence):
//! - submode bit2 (0x04): XA ADPCM audio sector (Form2 payload)
//! - channel 0 + submode bit3 (0x08) data sectors carry markers:
//!   - 0xF1: video payload bytes
//!   - 0xF2: frame-end or interactive command (submode bit0 distinguishes)
//!   - 0xF3: scene reset

use crate::content::{DiscImage, Sector, COOKED_SECTOR, RAW_SECTOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XaRoute {
    Audio,
    VideoData,
    FrameEnd,
    Interactive,
    SceneReset,
    Other,
}

pub fn route_sector(sector: &Sector) -> XaRoute {
    if sector.submode & 0x04 != 0 {
        return XaRoute::Audio;
    }
    if sector.channel_id == 0 && sector.submode & 0x08 != 0 && !sector.data.is_empty() {
        return match sector.data[0] {
            0xF1 => XaRoute::VideoData,
            0xF2 => {
                if sector.submode & 0x01 != 0 {
                    XaRoute::Interactive
                } else {
                    XaRoute::FrameEnd
                }
            }
            0xF3 => XaRoute::SceneReset,
            _ => XaRoute::Other,
        };
    }
    // Fallback classic XA file IDs used by some titles.
    if sector.file_id == 0x61 {
        return XaRoute::VideoData;
    }
    if sector.file_id == 0x62 {
        return XaRoute::Audio;
    }
    XaRoute::Other
}

#[derive(Debug, Default, Clone)]
pub struct XaDemux {
    pub video_sectors: u64,
    pub audio_sectors: u64,
    pub other_sectors: u64,
    pub frame_ends: u64,
    pub interactive_cmds: u64,
    pub scene_resets: u64,
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
        match route_sector(sector) {
            XaRoute::Audio => {
                self.audio_sectors += 1;
                self.last_audio_lba = Some(sector.lba);
                XaPacket::Audio {
                    lba: sector.lba,
                    channel: sector.channel_id,
                    submode: sector.submode,
                    coding: sector.coding,
                    data: sector.data.clone(),
                }
            }
            XaRoute::VideoData => {
                self.video_sectors += 1;
                self.last_video_lba = Some(sector.lba);
                XaPacket::Video {
                    lba: sector.lba,
                    channel: sector.channel_id,
                    submode: sector.submode,
                    coding: sector.coding,
                    data: sector.data.clone(),
                }
            }
            XaRoute::FrameEnd => {
                self.frame_ends += 1;
                XaPacket::FrameEnd {
                    lba: sector.lba,
                    submode: sector.submode,
                    data: sector.data.clone(),
                }
            }
            XaRoute::Interactive => {
                self.interactive_cmds += 1;
                XaPacket::Interactive {
                    lba: sector.lba,
                    data: sector.data.clone(),
                }
            }
            XaRoute::SceneReset => {
                self.scene_resets += 1;
                XaPacket::SceneReset { lba: sector.lba }
            }
            XaRoute::Other => {
                self.other_sectors += 1;
                XaPacket::Other {
                    lba: sector.lba,
                    file_id: sector.file_id,
                    data_len: sector.data.len(),
                }
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
    FrameEnd {
        lba: u32,
        submode: u8,
        data: Vec<u8>,
    },
    Interactive {
        lba: u32,
        data: Vec<u8>,
    },
    SceneReset {
        lba: u32,
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
