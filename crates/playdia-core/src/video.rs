//! CDXA still/stream video path.
//!
//! Independent model of the research corpus:
//! - display surface is 320x240 RGB555
//! - disc packets arrive as Mode2 Form2 XA payloads (file 0x61)
//! - a hardware-decoder HLE accepts raw frames for offline validation
//!
//! Full proprietary block decoder is tracked in docs/PROJECT-STATUS.md as
//! pending reverse-engineering milestones; this module provides the
//! framebuffer contract and packet ingestion for the machine.

use crate::cd::XaPacket;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    None,
    Still,
    Progressive,
    Reference,
}

#[derive(Debug, Clone)]
pub struct VideoDecoder {
    pub framebuffer: Vec<u16>,
    pub frames_decoded: u64,
    pub bytes_ingested: u64,
    pub last_kind: FrameKind,
    pub pending: Vec<u8>,
}

impl Default for VideoDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoDecoder {
    pub fn new() -> Self {
        Self {
            framebuffer: vec![0; WIDTH * HEIGHT],
            frames_decoded: 0,
            bytes_ingested: 0,
            last_kind: FrameKind::None,
            pending: Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        self.framebuffer.fill(0);
        self.frames_decoded = 0;
        self.bytes_ingested = 0;
        self.last_kind = FrameKind::None;
        self.pending.clear();
    }

    /// Ingest one XA video sector payload.
    pub fn ingest_packet(&mut self, packet: &XaPacket) {
        let XaPacket::Video { data, submode, .. } = packet else {
            return;
        };
        self.bytes_ingested += data.len() as u64;
        self.pending.extend_from_slice(data);
        // End-of-approx-frame heuristic from research: realtime + end bits or buffer high-water.
        let approx_frame = 2336 * 12; // ~12 sectors of form2 payload
        if self.pending.len() >= approx_frame || (submode & 0x80) != 0 {
            self.flush_frame();
        }
    }

    /// Force-decode whatever is buffered (used by machine once per frame).
    pub fn flush_frame(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        // HLE placeholder: do not invent pixels. Mark progress and clear.
        // Real decode path plugs in here after fixture-backed decoder lands.
        self.frames_decoded += 1;
        self.last_kind = FrameKind::Progressive;
        self.pending.clear();
    }

    /// RGB555 framebuffer as bytes (LE) for host presenters / CRC.
    pub fn framebuffer_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.framebuffer.len() * 2);
        for px in &self.framebuffer {
            out.extend_from_slice(&px.to_le_bytes());
        }
        out
    }

    pub fn crc(&self) -> u32 {
        crate::state::crc32(&self.framebuffer_bytes())
    }
}

/// Convert RGB555 word to 8-bit RGB (for PNG dumps / debugging).
pub fn rgb555_to_rgb888(v: u16) -> (u8, u8, u8) {
    let r = ((v >> 0) & 0x1F) as u8;
    let g = ((v >> 5) & 0x1F) as u8;
    let b = ((v >> 10) & 0x1F) as u8;
    (
        (r << 3) | (r >> 2),
        (g << 3) | (g >> 2),
        (b << 3) | (b >> 2),
    )
}
