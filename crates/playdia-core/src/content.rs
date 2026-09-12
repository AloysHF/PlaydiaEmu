//! Content loading: raw/cooked discs + multi-track CUE+BIN.

use crate::state::crc32;
use std::path::{Path, PathBuf};
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
    #[error("cue parse: {0}")]
    Cue(String),
    #[error("missing bin for cue track: {0}")]
    MissingBin(String),
}

/// One MODE2/2352 (or MODE1) binary track from a CUE sheet.
#[derive(Debug, Clone)]
pub struct Track {
    pub number: u8,
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub sectors: u32,
    pub mode2: bool,
}

/// Loaded disc: optional multi-track CUE, or a single image.
#[derive(Debug, Clone)]
pub struct DiscImage {
    pub cue_path: Option<PathBuf>,
    pub tracks: Vec<Track>,
    /// Single-image fallback (raw or cooked).
    pub single: Option<SingleImage>,
    pub total_sectors: u32,
    pub crc: u32,
    pub kind: DiscKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscKind {
    SingleRaw,
    SingleCooked,
    CueMultiTrack,
}

#[derive(Debug, Clone)]
pub struct SingleImage {
    pub data: Vec<u8>,
    pub raw: bool,
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
            cue_path: None,
            tracks: vec![],
            single: Some(SingleImage { data: bytes, raw }),
            total_sectors: total,
            crc,
            kind: if raw {
                DiscKind::SingleRaw
            } else {
                DiscKind::SingleCooked
            },
        })
    }

    /// Load `.cue` (+ sibling `.bin`) or a raw/cooked image by extension.
    pub fn from_path(path: &Path) -> Result<Self, LoadError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "cue" => Self::from_cue(path),
            "iso" | "bin" | "img" => {
                let data = std::fs::read(path)?;
                Self::from_bytes(data)
            }
            _ => {
                // Try as raw image anyway.
                let data = std::fs::read(path)?;
                Self::from_bytes(data)
            }
        }
    }

    pub fn from_cue(cue_path: &Path) -> Result<Self, LoadError> {
        let text = std::fs::read_to_string(cue_path)?;
        let base = cue_path.parent().unwrap_or(Path::new("."));
        let mut tracks: Vec<Track> = Vec::new();
        let mut current: Option<(u8, PathBuf, bool)> = None;

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("FILE ") {
                if let Some((num, pth, mode2)) = current.take() {
                    tracks.push(Self::load_track(num, pth, mode2)?);
                }
                // FILE "name" BINARY
                let rest = line[5..].trim();
                let name = if let Some(stripped) = rest.strip_prefix('"') {
                    match stripped.find('"') {
                        Some(end) => &stripped[..end],
                        None => stripped,
                    }
                } else {
                    rest.split_whitespace().next().unwrap_or("")
                };
                if name.is_empty() {
                    return Err(LoadError::Cue("empty FILE name".into()));
                }
                current = Some((0, base.join(name), false));
            } else if upper.starts_with("TRACK ") {
                let num: u8 = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| LoadError::Cue(format!("bad TRACK: {line}")))?;
                let mode2 = upper.contains("MODE2");
                if let Some((_, pth, _)) = current.as_mut() {
                    // Keep path; attach track number.
                    let path = pth.clone();
                    current = Some((num, path, mode2));
                }
            }
        }
        if let Some((num, pth, mode2)) = current.take() {
            tracks.push(Self::load_track(num, pth, mode2)?);
        }
        if tracks.is_empty() {
            return Err(LoadError::Cue("no tracks".into()));
        }
        let total_sectors = tracks.iter().map(|t| t.sectors).sum();
        let mut crc_src = Vec::new();
        for t in &tracks {
            crc_src.extend_from_slice(&t.data);
        }
        let crc = crc32(&crc_src);
        Ok(Self {
            cue_path: Some(cue_path.to_path_buf()),
            tracks,
            single: None,
            total_sectors,
            crc,
            kind: DiscKind::CueMultiTrack,
        })
    }

    fn load_track(number: u8, path: PathBuf, mode2: bool) -> Result<Track, LoadError> {
        if !path.exists() {
            return Err(LoadError::MissingBin(path.display().to_string()));
        }
        let data = std::fs::read(&path)?;
        if data.len() % RAW_SECTOR != 0 {
            return Err(LoadError::BadDiscSize(data.len()));
        }
        let sectors = (data.len() / RAW_SECTOR) as u32;
        Ok(Track {
            number,
            path,
            data,
            sectors,
            mode2,
        })
    }

    /// Streaming track for HLE player: last MODE2 track is usually Track 2.
    pub fn stream_track(&self) -> Option<&Track> {
        if !self.tracks.is_empty() {
            // Prefer the largest track (stream data).
            return self
                .tracks
                .iter()
                .max_by_key(|t| t.data.len())
                .or_else(|| self.tracks.last());
        }
        None
    }

    pub fn data_track(&self) -> Option<&Track> {
        self.tracks.first()
    }

    /// Byte offset of LBA within a single image (raw LBA index).
    pub fn sector_offset(&self, lba: u32) -> Option<usize> {
        let idx = lba as usize;
        if let Some(single) = &self.single {
            let bps = if single.raw {
                RAW_SECTOR
            } else {
                COOKED_SECTOR
            };
            let off = idx.checked_mul(bps)?;
            if off + bps <= single.data.len() {
                return Some(off);
            }
        }
        None
    }

    pub fn read_sector(&self, lba: u32) -> Option<Sector> {
        if let Some(single) = &self.single {
            let off = self.sector_offset(lba)?;
            let bps = if single.raw {
                RAW_SECTOR
            } else {
                COOKED_SECTOR
            };
            let raw = &single.data[off..off + bps];
            return if single.raw {
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
            };
        }
        None
    }

    /// Read raw sector from a numbered track (1-based).
    pub fn read_track_sector(&self, track_no: u8, index: u32) -> Option<&[u8]> {
        let t = self.tracks.iter().find(|t| t.number == track_no)?;
        let off = index as usize * RAW_SECTOR;
        if off + RAW_SECTOR <= t.data.len() {
            Some(&t.data[off..off + RAW_SECTOR])
        } else {
            None
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
    let sub = &raw[16..24];
    let file_id = sub[0];
    let channel_id = sub[1];
    let submode = sub[2];
    let coding = sub[3];
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

pub fn load_bios(bytes: &[u8], allow_placeholder: bool) -> Result<Vec<u8>, LoadError> {
    if bytes.len() == BIOS_SIZE {
        return Ok(bytes.to_vec());
    }
    if allow_placeholder && bytes.is_empty() {
        return Ok(vec![0; BIOS_SIZE]);
    }
    Err(LoadError::BadBiosSize(bytes.len()))
}
