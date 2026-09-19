//! Pure HLE disc player (no BIOS / no main CPU required).
//!
//! Streams Track 2 of a MODE2 CUE, routes F1/F2/F3 video and XA audio,
//! and produces a 320×240 XRGB8888 framebuffer + stereo PCM.

use crate::audio::AudioDecoder;
use crate::cd::{XaDemux, XaPacket};
use crate::content::{DiscImage, LoadError, RAW_SECTOR};
use crate::input::{InputButtons, InputState};
use crate::state::{crc32, decode_state, encode_state, ContentIdentity, SaveStateError};
use crate::video::VideoDecoder;
use crate::{FB_HEIGHT, FB_WIDTH};

/// Realtime-ish pacing: ~3 frames of 12282-byte video ≈ 6 F1 + audio interleaving.
/// From Mari-nee: ~8 video slots + occasional audio every ~16 sectors.
const SECTORS_PER_FRAME: u32 = 8;

/// Host frames to wait at F2 choice/quiz prompts before applying timeout.
/// Matches the ~10 s default used by the reference interactive handler.
const CHOICE_TIMEOUT_FRAMES: u32 = 300;

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
    waiting: Option<[Option<u32>; 7]>,
    wait_frames: u32,
    timeout_dest: Option<u32>,
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
            waiting: None,
            wait_frames: 0,
            timeout_dest: None,
        }
    }

    pub fn load_path(&mut self, path: &std::path::Path) -> Result<(), LoadError> {
        let disc = DiscImage::from_path(path)?;
        self.identity.disc_crc = disc.crc;
        self.identity.disc_sectors = disc.total_sectors;
        self.disc = Some(disc);
        self.reset();
        Ok(())
    }

    pub fn load_bytes(&mut self, bytes: Vec<u8>) -> Result<(), LoadError> {
        let disc = DiscImage::from_bytes(bytes)?;
        self.identity.disc_crc = disc.crc;
        self.identity.disc_sectors = disc.total_sectors;
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
        self.waiting = None;
        self.wait_frames = 0;
        self.timeout_dest = None;
    }

    pub fn set_input(&mut self, held: InputButtons) {
        self.input.update_held(held);
    }

    pub fn run_frame(&mut self) -> PlayerStop {
        if self.waiting.is_some() {
            // Level-triggered while waiting: a button already held when the menu
            // opens must still select (matching the interactive controller poll).
            let mut seek_result: Option<bool> = None;
            if let Some(button) = choice_slot(self.input.held) {
                if let Some(destinations) = self.waiting {
                    if let Some(target) = destinations[button] {
                        seek_result = Some(self.seek_lba(target));
                        if seek_result == Some(true) {
                            self.waiting = None;
                            self.wait_frames = 0;
                        }
                    }
                }
            }
            // Count timeout whenever still waiting (including failed seeks).
            if self.waiting.is_some() && seek_result != Some(true) {
                self.wait_frames += 1;
                if self.wait_frames >= CHOICE_TIMEOUT_FRAMES {
                    if let Some(target) = self.timeout_choice_target() {
                        log::info!(
                            "F2 choice timeout after {} frames → {}",
                            self.wait_frames,
                            target
                        );
                        if self.seek_lba(target) {
                            self.waiting = None;
                            self.wait_frames = 0;
                        }
                    } else {
                        log::warn!(
                            "F2 choice timeout has no valid target after {} frames",
                            self.wait_frames
                        );
                        // Keep counting; retry once a target becomes valid.
                        self.wait_frames = CHOICE_TIMEOUT_FRAMES;
                    }
                }
            }
            if self.waiting.is_some() {
                self.input.clear_edges();
                self.frame += 1;
                return PlayerStop::Ok;
            }
        }
        // Collect raw sectors first to avoid borrow conflicts.
        let mut batch: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut single_mode = false;
        {
            let Some(disc) = self.disc.as_ref() else {
                return PlayerStop::NoStream;
            };
            if disc.kind == crate::content::DiscKind::CueMultiTrack {
                let Some(track) = disc.stream_track() else {
                    return PlayerStop::NoStream;
                };
                let track_base = disc
                    .tracks
                    .iter()
                    .take_while(|t| t.number != track.number)
                    .map(|t| t.sectors)
                    .sum::<u32>();
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
                    batch.push((track_base + self.track_index - 1, raw.to_vec()));
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
                    batch.push((lba, item));
                }
            }
        }

        for (lba, item) in batch {
            if single_mode {
                if item.len() < 5 || item[4] == 0 {
                    continue;
                }
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
                if self.handle_sector(sec) {
                    break;
                }
            } else {
                if self.handle_raw_sector(&item, lba) {
                    break;
                }
            }
        }

        // Present one decoded sub-frame per host tick when available.
        let _ = self.video.present_next();
        self.input.clear_edges();
        self.frame += 1;

        if self.demux.end_flag {
            PlayerStop::EndOfDisc
        } else {
            PlayerStop::Ok
        }
    }

    fn handle_raw_sector(&mut self, raw: &[u8], lba: u32) -> bool {
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
            lba,
        };
        self.handle_sector(sec)
    }

    fn handle_sector(&mut self, sec: crate::content::Sector) -> bool {
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
                self.video.ingest_packet(&pkt);
                self.video.present_latest();
                self.interactive.push((*lba, data.clone()));
                return self.apply_interactive(*lba, data);
            }
            XaPacket::Other { .. } => {}
        }
        false
    }

    fn apply_interactive(&mut self, lba: u32, data: &[u8]) -> bool {
        if data.len() < 31 || data[0] != 0xF2 {
            log::warn!("short F2 command at LBA {lba}");
            return false;
        }
        let mut destinations = [None; 7];
        for (i, dest) in destinations.iter_mut().enumerate() {
            let off = 3 + i * 4;
            *dest = command_address_to_lba(&data[off..off + 3])
                .filter(|&target| self.valid_target(target));
        }
        match data[1] {
            0x44 | 0x50 => {
                if destinations.iter().all(Option::is_none) {
                    log::warn!("F2 choice without valid destinations at LBA {lba}");
                    return false;
                }
                self.resume_after(lba);
                self.waiting = Some(destinations);
                self.wait_frames = 0;
                true
            }
            0x40 | 0x60 | 0x90 | 0xA0 => {
                if data[1] == 0xA0 && data[2] == 0xF0 {
                    return false;
                }
                let Some(target) = destinations[0] else {
                    log::warn!("F2 jump without valid destination at LBA {lba}");
                    return false;
                };
                self.resume_after(lba);
                // Backward jumps loop until any held button breaks out.
                if target <= lba && self.input.held != InputButtons::default() {
                    self.video.discard_pending();
                    return true;
                }
                let _ = self.seek_lba(target);
                true
            }
            0x80 => {
                // Timeout modifier for a preceding/subsequent F2 44/50 choice:
                // first destination slot is the fallback LBA (not a navigation command).
                self.timeout_dest = destinations[0];
                false
            }
            kind => {
                log::warn!("unknown F2 command {kind:#04x} at LBA {lba}");
                false
            }
        }
    }

    fn timeout_choice_target(&self) -> Option<u32> {
        if let Some(target) = self.timeout_dest {
            if self.valid_target(target) {
                return Some(target);
            }
        }
        match self.waiting {
            Some(destinations) => destinations[0].filter(|&target| self.valid_target(target)),
            None => None,
        }
    }

    fn resume_after(&mut self, lba: u32) {
        let Some(disc) = self.disc.as_ref() else {
            return;
        };
        if disc.kind == crate::content::DiscKind::CueMultiTrack {
            if let Some(track) = disc.stream_track() {
                let base = disc
                    .tracks
                    .iter()
                    .take_while(|t| t.number != track.number)
                    .map(|t| t.sectors)
                    .sum::<u32>();
                self.track_index = lba.saturating_sub(base) + 1;
            }
        } else {
            self.sector_cursor = lba + 1;
        }
    }

    fn seek_lba(&mut self, lba: u32) -> bool {
        let Some(disc) = self.disc.as_ref() else {
            return false;
        };
        if disc.kind == crate::content::DiscKind::CueMultiTrack {
            let Some(track) = disc.stream_track() else {
                return false;
            };
            let base = disc
                .tracks
                .iter()
                .take_while(|t| t.number != track.number)
                .map(|t| t.sectors)
                .sum::<u32>();
            if lba < base || lba - base >= track.sectors {
                log::warn!("F2 destination outside stream track: LBA {lba}");
                return false;
            }
            self.track_index = lba - base;
        } else {
            if lba >= disc.total_sectors {
                log::warn!("F2 destination outside disc: LBA {lba}");
                return false;
            }
            self.sector_cursor = lba;
        }
        self.video.discard_pending();
        self.demux.end_flag = false;
        true
    }

    fn valid_target(&self, lba: u32) -> bool {
        let Some(disc) = self.disc.as_ref() else {
            return false;
        };
        if disc.kind == crate::content::DiscKind::CueMultiTrack {
            let Some(track) = disc.stream_track() else {
                return false;
            };
            let base = disc
                .tracks
                .iter()
                .take_while(|t| t.number != track.number)
                .map(|t| t.sectors)
                .sum::<u32>();
            lba >= base && lba - base < track.sectors
        } else {
            lba < disc.total_sectors
        }
    }

    pub fn is_waiting_for_input(&self) -> bool {
        self.waiting.is_some()
    }

    pub fn framebuffer(&self) -> &[u32] {
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
            "frames={} failed={} video_sec={} audio_sec={} interactive={} waiting={} fb_crc={:08x}",
            self.video.frames_decoded,
            self.video.frames_failed,
            self.demux.video_sectors,
            self.demux.audio_sectors,
            self.demux.interactive_cmds,
            self.is_waiting_for_input(),
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

    pub fn save_state(&self) -> Vec<u8> {
        let payload = self.encode_payload();
        encode_state(&self.identity, &payload)
    }

    pub fn load_state(&mut self, buf: &[u8]) -> Result<(), SaveStateError> {
        let payload = decode_state(buf, &self.identity)?;
        self.decode_payload(&payload)?;
        Ok(())
    }

    fn encode_payload(&self) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(&self.frame.to_le_bytes());
        p.extend_from_slice(&self.sector_cursor.to_le_bytes());
        p.extend_from_slice(&self.track_index.to_le_bytes());
        match self.waiting {
            Some(dest) => {
                p.push(1);
                for d in dest {
                    match d {
                        Some(lba) => {
                            p.push(1);
                            p.extend_from_slice(&lba.to_le_bytes());
                        }
                        None => p.push(0),
                    }
                }
            }
            None => p.push(0),
        }
        p.extend_from_slice(&self.wait_frames.to_le_bytes());
        match self.timeout_dest {
            Some(lba) => {
                p.push(1);
                p.extend_from_slice(&lba.to_le_bytes());
            }
            None => p.push(0),
        }
        for v in [
            self.demux.video_sectors,
            self.demux.audio_sectors,
            self.demux.other_sectors,
            self.demux.frame_ends,
            self.demux.interactive_cmds,
            self.demux.scene_resets,
        ] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        match self.demux.last_video_lba {
            Some(lba) => {
                p.push(1);
                p.extend_from_slice(&lba.to_le_bytes());
            }
            None => p.push(0),
        }
        match self.demux.last_audio_lba {
            Some(lba) => {
                p.push(1);
                p.extend_from_slice(&lba.to_le_bytes());
            }
            None => p.push(0),
        }
        p.push(u8::from(self.demux.end_flag));
        self.video.encode_body(&mut p);
        self.audio.encode_body(&mut p);
        p.push(buttons_mask(self.input.held));
        p
    }

    fn decode_payload(&mut self, p: &[u8]) -> Result<(), SaveStateError> {
        let mut o = 0usize;
        let take = |o: &mut usize, n: usize| -> Result<&[u8], SaveStateError> {
            let s = p.get(*o..*o + n).ok_or(SaveStateError::Truncated)?;
            *o += n;
            Ok(s)
        };
        self.frame = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.sector_cursor = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.track_index = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.waiting = if take(&mut o, 1)?[0] == 0 {
            None
        } else {
            let mut dest = [None; 7];
            for slot in &mut dest {
                if take(&mut o, 1)?[0] == 0 {
                    *slot = None;
                } else {
                    *slot = Some(u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()));
                }
            }
            Some(dest)
        };
        self.wait_frames = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.timeout_dest = if take(&mut o, 1)?[0] == 0 {
            None
        } else {
            Some(u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()))
        };
        self.demux.video_sectors = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.demux.audio_sectors = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.demux.other_sectors = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.demux.frame_ends = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.demux.interactive_cmds = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.demux.scene_resets = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.demux.last_video_lba = if take(&mut o, 1)?[0] == 0 {
            None
        } else {
            Some(u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()))
        };
        self.demux.last_audio_lba = if take(&mut o, 1)?[0] == 0 {
            None
        } else {
            Some(u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()))
        };
        self.demux.end_flag = take(&mut o, 1)?[0] != 0;
        self.video.decode_body(p, &mut o)?;
        self.audio.decode_body(p, &mut o)?;
        self.input.held = buttons_from_mask(take(&mut o, 1)?[0]);
        self.input.pressed = InputButtons::default();
        self.input.released = InputButtons::default();
        self.interactive.clear();
        Ok(())
    }
}

fn buttons_mask(b: InputButtons) -> u8 {
    let mut m = 0u8;
    if b.up {
        m |= 1;
    }
    if b.down {
        m |= 2;
    }
    if b.left {
        m |= 4;
    }
    if b.right {
        m |= 8;
    }
    if b.a {
        m |= 16;
    }
    if b.b {
        m |= 32;
    }
    if b.start {
        m |= 64;
    }
    if b.select {
        m |= 128;
    }
    m
}

fn buttons_from_mask(m: u8) -> InputButtons {
    InputButtons {
        up: m & 1 != 0,
        down: m & 2 != 0,
        left: m & 4 != 0,
        right: m & 8 != 0,
        a: m & 16 != 0,
        b: m & 32 != 0,
        start: m & 64 != 0,
        select: m & 128 != 0,
    }
}

fn command_address_to_lba(address: &[u8]) -> Option<u32> {
    if address.len() != 3 {
        return None;
    }
    // F2 destinations are raw binary M/S/unit bytes (not BCD, not range-checked
    // CD MSF): real discs encode second-like fields above 59.
    // Unit is a five-sector counter: LBA = M×4500 + S×75 + unit×5 − 150.
    (u32::from(address[0]) * 4500 + u32::from(address[1]) * 75 + u32::from(address[2]) * 5)
        .checked_sub(150)
}

fn choice_slot(buttons: InputButtons) -> Option<usize> {
    // B1=Start/default, B2=Up, B3=Down, B4=Left, B5=Right, B6=A, B7=B.
    if buttons.start {
        Some(0)
    } else if buttons.up {
        Some(1)
    } else if buttons.down {
        Some(2)
    } else if buttons.left {
        Some(3)
    } else if buttons.right {
        Some(4)
    } else if buttons.a {
        Some(5)
    } else if buttons.b {
        Some(6)
    } else {
        None
    }
}
