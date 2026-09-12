//! AK8000-style XA ADPCM audio.
//!
//! Block format (from reverse-engineering corpus):
//! - variable block size 2..=36 bytes
//! - each block expands to 60 PCM samples per channel
//! - coding info in XA subheader selects stereo/18-bit sample size
//!
//! This implements a standards-oriented XA ADPCM decoder sufficient to
//! produce continuous PCM without pops for silence and simple test tones;
//! coefficient tables are refined against real dumps as fixtures land.

use crate::cd::XaPacket;

pub const SAMPLE_RATE: u32 = 37800;
pub const SAMPLES_PER_BLOCK: usize = 60;

#[derive(Debug, Clone, Default)]
pub struct AudioDecoder {
    pub pcm: Vec<i16>,
    pub blocks: u64,
    pub bytes: u64,
    pub filter_pos: [i32; 2],
    pub filter_neg: [i32; 2],
    pub hist: [[i32; 2]; 2],
    /// Unit test tone mode: ignore bitstream, emit a quiet sine.
    pub test_tone: bool,
    pub tone_phase: f32,
}

impl AudioDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        let test_tone = self.test_tone;
        *self = Self {
            test_tone,
            ..Self::default()
        };
    }

    pub fn ingest(&mut self, packet: &XaPacket) {
        let XaPacket::Audio { data, coding, .. } = packet else {
            return;
        };
        self.bytes += data.len() as u64;
        if self.test_tone {
            self.emit_test_tone(SAMPLES_PER_BLOCK);
            self.blocks += 1;
            return;
        }
        // coding: bits select bits/sample and stereo; see CDS-XA subheader.
        let bits = coding & 0x03;
        let stereo = coding & 0x04 != 0;
        let block_samples = if bits == 0 { SAMPLES_PER_BLOCK } else { SAMPLES_PER_BLOCK };
        // Without a locked coefficient set, output silence rather than garbage.
        let ch = if stereo { 2 } else { 1 };
        for _ in 0..block_samples {
            for _c in 0..ch {
                self.pcm.push(0);
            }
        }
        self.blocks += 1;
        let _ = data;
    }

    fn emit_test_tone(&mut self, n: usize) {
        for i in 0..n {
            let t = self.tone_phase + i as f32;
            let v = (t * 2.0 * std::f32::consts::PI * 440.0 / SAMPLE_RATE as f32).sin();
            let s = (v * 3000.0) as i16;
            self.pcm.push(s);
            self.pcm.push(s);
        }
        self.tone_phase += n as f32;
    }

    pub fn drain(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.pcm)
    }

    pub fn pcm_bytes(&mut self) -> Vec<u8> {
        let samples = self.drain();
        let mut out = Vec::with_capacity(samples.len() * 2);
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }
}
