//! Pure HLE disc player (no BIOS / no main CPU required).
//!
//! Streams Track 2 of a MODE2 CUE, routes F1/F2/F3 video and XA audio,
//! and produces a 320×240 RGB555 framebuffer + stereo PCM.

use crate::audio::AudioDecoder;
use crate::cd::{XaDemux, XaPacket};
use crate::content::{DiscImage, LoadError, RAW_SECTOR};
use crate::input::{InputButtons, InputState};
use crate::state::{crc32, ContentIdentity};
use crate::video::VideoDecoder;
use crate::{FB_HEIGHT, FB_WIDTH};

/// Realtime-ish pacing: ~3 frames of 12282-byte video ≈ 6 F1 + audio interleaving.
/// From Mari-nee: ~8 video slots + occasional audio every ~16 sectors.
const SECTORS_PER_FRAME: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerStop {
    Ok,
    EndOfDisc,
    NoStream,
}

#[derive(Debug)]
pub struct DiscPlayer {
    pub disc: Option<DiscImage>,
    pub demux: XaDemux,
    pub video: VideoDecoder,
    pub audio: AudioDecoder,
    pub input: InputState,
    pub identity: ContentIdentity,
    pub frame: u64,
    pub sector_cursor: u32,
    pub track_index: u32,
    pub interactive: Vec<(u32, Vec<u8>)>,
    pub pcm: Vec<i16>,
}

impl Default for DiscPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscPlayer {
    pub fn new() -> Self {
        Self {
            disc: None,
            demux: XaDemux::new(),
            video: VideoDecoder::new(),
            audio: AudioDecoder::new(),
            input: InputState::default(),
            identity: ContentIdentity::default(),
            frame: 0,
            sector_cursor: 0,
            track_index: 0,
            interactive: Vec::new(),
            pcm: Vec::new(),
        }
    }

    pub fn load_path(&mut self, path: &std::path::Path) -> Result<(), LoadError> {
        let disc = DiscImage::from_path(path)?;
        self.identity.disc_crc = disc.crc;
        self.identity.disc_sectors = disc.total_sectors;
        self.identity.bios_crc = None;
        self.disc = Some(disc);
        self.reset();
        Ok(())
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) -> Result<(), LoadError> {
        let disc = DiscImage::from_bytes(bytes)?;
        self.identity.disc_crc = disc.crc;
        self.identity.disc_sectors = disc.total_sectors;
        self.identity.bios_crc = None;
        self.disc = Some(disc);
        self.reset();
        Ok(())
    }

    pub fn reset(&mut self) {
        self.demux.reset();
        self.video.reset();
        self.audio.reset();
        self.input.clear_edges();
        self.frame = 0;
        self.sector_cursor = 0;
        self.track_index = 0;
        self.interactive.clear();
        self.pcm.clear();
    }

    pub fn set_input(&mut self, held: InputButtons) {
        self.input.update_held(held);
    }

    pub fn run_frame(&mut self) -> PlayerStop {
        // Collect raw sectors first to avoid borrow conflicts.
        let mut batch: Vec<Vec<u8>> = Vec::new();
        let mut single_mode = false;
        {
            let Some(disc) = self.disc.as_ref() else {
                return PlayerStop::NoStream;
            };
            if disc.kind == crate::content::DiscKind::CueMultiTrack {
                let Some(track) = disc.stream_track().cloned() else {
                    return PlayerStop::NoStream;
                };
                // Skip lead-in / TOC (file_id 0) until stream content (file_id 1) starts.
                if self.track_index == 0 {
                    for i in 0..track.sectors {
                        let o = i as usize * 2352;
                        if o + 16 >= track.data.len() {
                            break;
                        }
                        if track.data[o + 16] == 1 {
                            self.track_index = i;
                            break;
                        }
                    }
                }
                for _ in 0..SECTORS_PER_FRAME {
                    if self.track_index >= track.sectors {
                        self.demux.end_flag = true;
                        break;
                    }
                    let Some(raw) = disc.read_track_sector(track.number, self.track_index) else {
                        break;
                    };
                    self.track_index += 1;
                    batch.push(raw.to_vec());
                }
            } else {
                single_mode = true;
                let total = disc.total_sectors;
                for _ in 0..SECTORS_PER_FRAME {
                    if self.sector_cursor >= total {
                        self.demux.end_flag = true;
                        break;
                    }
                    let lba = self.sector_cursor;
                    self.sector_cursor += 1;
                    // Encode lba as first 4 bytes + sector for single path.
                    let mut item = lba.to_le_bytes().to_vec();
                    if let Some(sec) = disc.read_sector(lba) {
                        item.push(1u8);
                        item.extend_from_slice(&sec.data);
                        item.extend_from_slice(&[
                            sec.mode,
                            sec.form,
                            sec.file_id,
                            sec.channel_id,
                            sec.submode,
                            sec.coding,
                        ]);
                    } else {
                        item.push(0u8);
                    }
                    batch.push(item);
                }
            }
        }

        for item in batch {
            if single_mode {
                if item.len() < 5 || item[4] == 0 {
                    continue;
                }
                let lba = u32::from_le_bytes(item[0..4].try_into().unwrap());
                let data_end = item.len() - 6;
                let data = item[5..data_end].to_vec();
                let meta = &item[data_end..];
                let sec = crate::content::Sector {
                    mode: meta[0],
                    form: meta[1],
                    subheader: [0; 8],
                    file_id: meta[2],
                    channel_id: meta[3],
                    submode: meta[4],
                    coding: meta[5],
                    data,
                    lba,
                };
                self.handle_sector(sec);
            } else {
                self.handle_raw_sector(&item);
            }
        }

        self.input.clear_edges();
        self.frame += 1;

        if self.demux.end_flag {
            PlayerStop::EndOfDisc
        } else {
            PlayerStop::Ok
        }
    }

    fn handle_raw_sector(&mut self, raw: &[u8]) {
        // Build a Sector from raw MODE2 without allocating path through DiscImage::single.
        let mode = raw.get(15).copied().unwrap_or(0);
        let file_id = raw.get(16).copied().unwrap_or(0);
        let channel_id = raw.get(17).copied().unwrap_or(0);
        let submode = raw.get(18).copied().unwrap_or(0);
        let coding = raw.get(19).copied().unwrap_or(0);
        let data = if raw.len() >= 24 + 2048 && (submode & 0x20) == 0 {
            // Form1: marker in first user byte at +24; keep full 2048 for marker routing.
            raw[24..24 + 2048].to_vec()
        } else if raw.len() >= 24 + 2324 {
            raw[24..24 + 2324].to_vec()
        } else {
            raw.get(24..).unwrap_or(&[]).to_vec()
        };
        let mut subheader = [0u8; 8];
        if raw.len() >= 24 {
            subheader.copy_from_slice(&raw[16..24]);
        }
        let sec = crate::content::Sector {
            mode,
            form: if submode & 0x20 != 0 { 2 } else { 1 },
            subheader,
            file_id,
            channel_id,
            submode,
            coding,
            data,
            lba: self.track_index.saturating_sub(1),
        };
        self.handle_sector(sec);
    }

    fn handle_sector(&mut self, sec: crate::content::Sector) {
        let pkt = self.demux.push(&sec);
        match &pkt {
            XaPacket::Video { data, .. } => {
                // Form1 user data starts with F1 marker at data[0].
                self.video.ingest_packet(&pkt);
                let _ = data;
            }
            XaPacket::FrameEnd { .. } | XaPacket::SceneReset { .. } => {
                self.video.ingest_packet(&pkt);
            }
            XaPacket::Audio { data, coding, .. } => {
                // Real discs put XA ADPCM in Form2 payload starting at byte 0 of data
                // (which includes marker nibble region). Feed full payload.
                let audio_pkt = XaPacket::Audio {
                    lba: sec.lba,
                    channel: sec.channel_id,
                    submode: sec.submode,
                    coding: *coding,
                    data: data.clone(),
                };
                self.audio.ingest(&audio_pkt);
            }
            XaPacket::Interactive { lba, data } => {
                self.interactive.push((*lba, data.clone()));
                self.demux.note_interactive();
            }
            XaPacket::Other { .. } => {}
        }
    }

    pub fn framebuffer(&self) -> &[u16] {
        &self.video.framebuffer
    }

    pub fn drain_audio(&mut self) -> Vec<i16> {
        self.audio.drain()
    }

    pub fn frame_crc(&self) -> u32 {
        self.video.crc()
    }

    pub fn stats_line(&self) -> String {
        format!(
            "frames={} failed={} video_sec={} audio_sec={} interactive={} fb_crc={:08x}",
            self.video.frames_decoded,
            self.video.frames_failed,
            self.demux.video_sectors,
            self.demux.audio_sectors,
            self.demux.interactive_cmds,
            self.frame_crc()
        )
    }

    pub fn frame_size() -> (usize, usize) {
        (FB_WIDTH, FB_HEIGHT)
    }

    pub fn crc32(data: &[u8]) -> u32 {
        crc32(data)
    }

    pub fn raw_sector_size() -> usize {
        RAW_SECTOR
    }
}
