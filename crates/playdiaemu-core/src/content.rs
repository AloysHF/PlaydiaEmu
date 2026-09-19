//! Content loading: raw/cooked discs, multi-track CUE+BIN, and ZIP archives.

use crate::state::crc32;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const RAW_SECTOR: usize = 2352;
pub const COOKED_SECTOR: usize = 2048;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("empty content")]
    Empty,
    #[error("unsupported disc size {0}")]
    BadDiscSize(usize),
    #[error("cue parse: {0}")]
    Cue(String),
    #[error("missing bin for cue track: {0}")]
    MissingBin(String),
    #[error("zip: {0}")]
    Zip(String),
    #[error("missing zip entry for cue track: {0}")]
    MissingZipEntry(String),
    #[error("no disc image in zip")]
    NoDiscInZip,
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

struct CueFileRef {
    number: u8,
    file_name: String,
    mode2: bool,
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

    /// Load `.cue` (+ sibling `.bin`), `.zip` (CUE/BIN inside), or raw/cooked image.
    pub fn from_path(path: &Path) -> Result<Self, LoadError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "cue" => Self::from_cue(path),
            "zip" => Self::from_zip(path),
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
        for CueFileRef {
            number,
            file_name,
            mode2,
        } in parse_cue_file_refs(&text)?
        {
            let path = base.join(&file_name);
            tracks.push(Self::load_track_file(number, path, mode2)?);
        }
        Self::finish_cue(cue_path.to_path_buf(), tracks)
    }

    /// Load a Redump-style ZIP containing a `.cue` + track bins, or a lone image.
    pub fn from_zip(path: &Path) -> Result<Self, LoadError> {
        let file = File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| LoadError::Zip(format!("{}: {e}", path.display())))?;

        let names: Vec<String> = archive.file_names().map(|s| s.to_owned()).collect();
        if let Some(cue_name) = pick_cue_name(&names) {
            let text = read_zip_entry(&mut archive, &cue_name)?;
            let text = String::from_utf8_lossy(&text).into_owned();
            let mut tracks: Vec<Track> = Vec::new();
            for CueFileRef {
                number,
                file_name,
                mode2,
            } in parse_cue_file_refs(&text)?
            {
                let entry = find_zip_entry(&names, &file_name).ok_or_else(|| {
                    LoadError::MissingZipEntry(format!("{file_name} (in {})", path.display()))
                })?;
                let data = read_zip_entry(&mut archive, &entry)?;
                tracks.push(Self::track_from_bytes(
                    number,
                    path.join(&file_name),
                    data,
                    mode2,
                )?);
            }
            return Self::finish_cue(path.to_path_buf(), tracks);
        }

        // Fallback: single image inside the archive.
        let image_name = pick_single_image_name(&names).ok_or(LoadError::NoDiscInZip)?;
        let data = read_zip_entry(&mut archive, &image_name)?;
        Self::from_bytes(data)
    }

    fn finish_cue(cue_path: PathBuf, tracks: Vec<Track>) -> Result<Self, LoadError> {
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
            cue_path: Some(cue_path),
            tracks,
            single: None,
            total_sectors,
            crc,
            kind: DiscKind::CueMultiTrack,
        })
    }

    fn load_track_file(number: u8, path: PathBuf, mode2: bool) -> Result<Track, LoadError> {
        if !path.exists() {
            return Err(LoadError::MissingBin(path.display().to_string()));
        }
        let data = std::fs::read(&path)?;
        Self::track_from_bytes(number, path, data, mode2)
    }

    fn track_from_bytes(
        number: u8,
        path: PathBuf,
        data: Vec<u8>,
        mode2: bool,
    ) -> Result<Track, LoadError> {
        if !data.len().is_multiple_of(RAW_SECTOR) {
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

fn parse_cue_file_refs(text: &str) -> Result<Vec<CueFileRef>, LoadError> {
    let mut refs: Vec<CueFileRef> = Vec::new();
    let mut current: Option<(u8, String, bool)> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("FILE ") {
            if let Some((num, name, mode2)) = current.take() {
                refs.push(CueFileRef {
                    number: num,
                    file_name: name,
                    mode2,
                });
            }
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
            current = Some((0, name.to_owned(), false));
        } else if upper.starts_with("TRACK ") {
            let num: u8 = line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| LoadError::Cue(format!("bad TRACK: {line}")))?;
            let mode2 = upper.contains("MODE2");
            if let Some((_, name, _)) = current.as_mut() {
                let name = name.clone();
                current = Some((num, name, mode2));
            }
        }
    }
    if let Some((num, name, mode2)) = current.take() {
        refs.push(CueFileRef {
            number: num,
            file_name: name,
            mode2,
        });
    }
    Ok(refs)
}

fn pick_cue_name(names: &[String]) -> Option<String> {
    names
        .iter()
        .filter(|n| {
            Path::new(n.as_str())
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("cue"))
                .unwrap_or(false)
        })
        .min_by_key(|n| n.len())
        .cloned()
}

fn pick_single_image_name(names: &[String]) -> Option<String> {
    names
        .iter()
        .filter(|n| {
            Path::new(n.as_str())
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| {
                    let e = e.to_ascii_lowercase();
                    e == "bin" || e == "iso" || e == "img"
                })
                .unwrap_or(false)
        })
        .max_by_key(|n| n.len())
        .cloned()
}

fn find_zip_entry(names: &[String], want: &str) -> Option<String> {
    let want_norm = normalize_zip_name(want);
    let want_base = base_name(&want_norm);
    names
        .iter()
        .map(|n| (n, normalize_zip_name(n)))
        .find(|(_, norm)| *norm == want_norm || base_name(norm) == want_base)
        .map(|(n, _)| n.clone())
}

fn normalize_zip_name(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

fn base_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

fn read_zip_entry(archive: &mut zip::ZipArchive<File>, name: &str) -> Result<Vec<u8>, LoadError> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| LoadError::Zip(format!("entry {name}: {e}")))?;
    let mut data = Vec::with_capacity(file.size() as usize);
    file.read_to_end(&mut data)?;
    Ok(data)
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
