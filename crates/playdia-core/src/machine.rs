//! Top-level machine: wires bus, CPU, CD, CDXA, video, audio, input.

use crate::audio::AudioDecoder;
use crate::bus::{Bus, BIOS_BASE};
use crate::cd::{scan_xa, XaDemux, XaPacket};
use crate::cdx::CdxDevice;
use crate::content::{load_bios, DiscImage, LoadError};
use crate::diagnostics::Diagnostics;
use crate::input::{InputButtons, InputState};
use crate::sh1::Sh1;
use crate::state::{decode_state, encode_state, ContentIdentity, SaveStateError};
use crate::video::VideoDecoder;
use crate::{CYCLES_PER_FRAME, FB_HEIGHT, FB_WIDTH};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MachineConfig {
    /// Allow empty BIOS for headless experiments without firmware.
    pub allow_placeholder_bios: bool,
    /// Feed decoded XA video into the framebuffer via software path.
    pub enable_xa_stream: bool,
    pub audio_test_tone: bool,
}

impl Default for MachineConfig {
    fn default() -> Self {
        Self {
            allow_placeholder_bios: false,
            enable_xa_stream: true,
            audio_test_tone: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStop {
    Ok,
    CpuHalted,
    EndOfDisc,
    Budget,
}

#[derive(Debug)]
pub struct Machine {
    pub config: MachineConfig,
    pub bus: Bus,
    pub cpu: Sh1,
    pub disc: Option<DiscImage>,
    pub demux: XaDemux,
    pub cdx: CdxDevice,
    pub video: VideoDecoder,
    pub audio: AudioDecoder,
    pub input: InputState,
    pub diag: Diagnostics,
    pub identity: ContentIdentity,
    pub frame: u64,
    pub lba_cursor: u32,
    pub bios_present: bool,
}

impl Machine {
    pub fn new(config: MachineConfig) -> Self {
        let bus = Bus::new(vec![0; crate::content::BIOS_SIZE]);
        Self {
            config,
            bus,
            cpu: Sh1::new(),
            disc: None,
            demux: XaDemux::new(),
            cdx: CdxDevice::new(),
            video: VideoDecoder::new(),
            audio: AudioDecoder::new(),
            input: InputState::default(),
            diag: Diagnostics::default(),
            identity: ContentIdentity::default(),
            frame: 0,
            lba_cursor: 0,
            bios_present: false,
        }
    }

    pub fn load_bios_bytes(&mut self, bytes: &[u8]) -> Result<(), LoadError> {
        let bios = load_bios(bytes, self.config.allow_placeholder_bios)?;
        self.bios_present = bytes.len() == crate::content::BIOS_SIZE;
        self.identity.bios_crc = Some(crate::state::crc32(&bios));
        self.bus.bios = bios;
        Ok(())
    }

    pub fn load_bios_path(&mut self, path: &Path) -> Result<(), LoadError> {
        let bytes = std::fs::read(path)?;
        self.load_bios_bytes(&bytes)
    }

    pub fn load_disc_bytes(&mut self, bytes: Vec<u8>) -> Result<(), LoadError> {
        let disc = DiscImage::from_bytes(bytes)?;
        self.identity.disc_crc = disc.crc;
        self.identity.disc_sectors = disc.total_sectors;
        self.demux = if disc.single.as_ref().map(|s| s.raw).unwrap_or(true) {
            // Lightweight scan only counts; full parse per frame.
            XaDemux::new()
        } else {
            XaDemux::new()
        };
        self.lba_cursor = 0;
        self.disc = Some(disc);
        Ok(())
    }

    pub fn load_disc_path(&mut self, path: &Path) -> Result<(), LoadError> {
        let bytes = std::fs::read(path)?;
        self.load_disc_bytes(bytes)
    }

    pub fn reset(&mut self) {
        self.bus.reset_ram();
        self.cdx.reset();
        self.video.reset();
        self.audio.reset();
        self.demux.reset();
        self.frame = 0;
        self.lba_cursor = 0;
        self.diag = Diagnostics::default();
        self.input.clear_edges();
        let entry = if self.bios_present {
            BIOS_BASE
        } else {
            // No firmware: CPU idles in a tight loop inside RAM for tests.
            crate::bus::RAM_BASE
        };
        self.cpu.reset(entry);
        if !self.bios_present {
            // Place an infinite BRA self-loop at RAM base: 0xAFFF = BRA -2?
            // BRA disp8: disp is from PC+4, -2 steps = stay. opcode 0xAFFF
            // Simpler: 0xAFFF is BRA with disp=-1 → pc = pc+4-2 = pc+2 (forward).
            // Infinite loop: 0xAFFF at addr, want pc back to self.
            // pc_new = pc+4 + disp*2 = pc ⇒ disp*2 = -4 ⇒ disp = -2 ⇒ opcode 0xAFFE
            // Actually BRA encoding disp is 8-bit: 0xAFFE has low=0xFE=-2.
            let mut d = Diagnostics::default();
            self.bus.write16(crate::bus::RAM_BASE, 0xAFFE, &mut d);
        }
    }

    pub fn set_input(&mut self, held: InputButtons) {
        self.input.update_held(held);
    }

    /// Advance one video frame: stream XA, step CPU under budget, flush video.
    pub fn run_frame(&mut self) -> RunStop {
        if self.config.enable_xa_stream {
            self.stream_xa();
        }
        let ops = CYCLES_PER_FRAME / 2;
        let executed = self.cpu.run(&mut self.bus, &mut self.diag, ops);
        self.video.flush_frame();
        self.input.clear_edges();
        self.frame += 1;
        if executed < ops && self.cpu.stopped.is_some() {
            return RunStop::CpuHalted;
        }
        if self.demux.end_flag {
            return RunStop::EndOfDisc;
        }
        RunStop::Ok
    }

    fn stream_xa(&mut self) {
        let Some(disc) = self.disc.as_ref() else {
            return;
        };
        // Realtime XA: ~75 sectors/sec ≈ 1.25 sectors/frame. Feed 2 when possible.
        for _ in 0..2 {
            let lba = self.lba_cursor;
            if lba >= disc.total_sectors {
                self.demux.end_flag = true;
                break;
            }
            let mut sec = match disc.read_sector(lba) {
                Some(s) => s,
                None => break,
            };
            sec.lba = lba;
            self.lba_cursor += 1;
            let pkt = self.demux.push(&sec);
            match &pkt {
                XaPacket::Video { .. }
                | XaPacket::FrameEnd { .. }
                | XaPacket::SceneReset { .. } => {
                    self.video.ingest_packet(&pkt);
                    if matches!(pkt, XaPacket::FrameEnd { .. }) {
                        self.cdx.mark_video_frame();
                    }
                }
                XaPacket::Audio { .. } => {
                    self.audio.ingest(&pkt);
                    self.cdx.mark_audio_block();
                }
                XaPacket::Interactive { .. } => {
                    let had_video = !self.video.acc.is_empty();
                    self.video.ingest_packet(&pkt);
                    if had_video {
                        self.cdx.mark_video_frame();
                    }
                    self.diag.note("xa_interactive");
                }
                XaPacket::Other { .. } => {}
            }
        }
    }

    pub fn framebuffer(&self) -> &[u32] {
        &self.video.framebuffer
    }

    pub fn drain_audio(&mut self) -> Vec<i16> {
        self.audio.drain()
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
        p.extend_from_slice(&self.lba_cursor.to_le_bytes());
        p.extend_from_slice(&(self.bios_present as u8).to_le_bytes());
        // CPU
        for r in &self.cpu.r {
            p.extend_from_slice(&r.to_le_bytes());
        }
        for v in [
            self.cpu.pc,
            self.cpu.pr,
            self.cpu.sr,
            self.cpu.gbr,
            self.cpu.mach,
            self.cpu.macl,
            self.cpu.vbr,
        ] {
            p.extend_from_slice(&v.to_le_bytes());
        }
        p.extend_from_slice(&self.cpu.cycles.to_le_bytes());
        // RAM
        p.extend_from_slice(&(self.bus.ram.len() as u32).to_le_bytes());
        p.extend_from_slice(&self.bus.ram);
        // CDXA shared
        p.extend_from_slice(&self.cdx.shared);
        // Video fb
        p.extend_from_slice(&(self.video.framebuffer.len() as u32).to_le_bytes());
        for px in &self.video.framebuffer {
            p.extend_from_slice(&px.to_le_bytes());
        }
        p.extend_from_slice(&self.video.frames_decoded.to_le_bytes());
        p
    }

    fn decode_payload(&mut self, p: &[u8]) -> Result<(), SaveStateError> {
        let mut o = 0usize;
        let take = |o: &mut usize, n: usize| -> Result<&[u8], SaveStateError> {
            let s = p.get(*o..*o + n).ok_or(SaveStateError::Truncated)?;
            *o += n;
            Ok(s)
        };
        let frame = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        let lba = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        let bios_present = take(&mut o, 1)?[0] != 0;
        for i in 0..16 {
            self.cpu.r[i] = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        }
        self.cpu.pc = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.pr = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.sr = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.gbr = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.mach = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.macl = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.vbr = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap());
        self.cpu.cycles = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        let ram_len = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()) as usize;
        let ram = take(&mut o, ram_len)?;
        self.bus.ram.copy_from_slice(ram);
        let shared = take(&mut o, 0x100)?;
        self.cdx.shared.copy_from_slice(shared);
        let fb_len = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()) as usize;
        for px in self.video.framebuffer.iter_mut().take(fb_len) {
            *px = u32::from_le_bytes(take(&mut o, 4)?.try_into().unwrap()) & 0x00FF_FFFF;
        }
        self.video.frames_decoded = u64::from_le_bytes(take(&mut o, 8)?.try_into().unwrap());
        self.frame = frame;
        self.lba_cursor = lba;
        self.bios_present = bios_present;
        self.cpu.stopped = None;
        Ok(())
    }

    pub fn frame_size() -> (usize, usize) {
        (FB_WIDTH, FB_HEIGHT)
    }

    /// Count XA packets on disc without mutating stream state permanently.
    pub fn disc_stats(&self) -> Option<XaDemux> {
        let disc = self.disc.as_ref()?;
        Some(scan_xa(disc))
    }
}
