//! CD-ROM XA ADPCM (Green Book) audio decoder.
//!
//! Each Form2 audio sector carries 18 sound groups of 128 bytes.
//! Native rate is 37800 or 18900 Hz; output is resampled to 44100 stereo.

use crate::cd::XaPacket;

pub const OUT_RATE: u32 = 44100;
pub const GROUPS_PER_SECTOR: usize = 18;
pub const BYTES_PER_GROUP: usize = 128;
pub const SAMPLES_PER_UNIT: usize = 28;

/// Fixed-point filter coefficients (×64), standard XA set.
const K0: [i32; 4] = [0, 60, 115, 98];
const K1: [i32; 4] = [0, 0, -52, -55];

#[derive(Debug, Clone, Default)]
pub struct AudioDecoder {
    pub pcm: Vec<i16>,
    pub blocks: u64,
    pub bytes: u64,
    pub prev: [i32; 2],
    pub prev2: [i32; 2],
}

impl AudioDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn ingest(&mut self, packet: &XaPacket) {
        let XaPacket::Audio { data, coding, .. } = packet else {
            return;
        };
        self.bytes += data.len() as u64;
        self.decode_sector(data, *coding);
        self.blocks += 1;
    }

    fn decode_sector(&mut self, sector_data: &[u8], coding: u8) {
        if sector_data.len() < GROUPS_PER_SECTOR * BYTES_PER_GROUP {
            return;
        }
        let stereo = coding & 1 != 0;
        let half_rate = coding & 4 != 0;
        let native_rate: f64 = if half_rate { 18900.0 } else { 37800.0 };

        let mut raw_l = vec![0i16; 4096];
        let mut raw_r = vec![0i16; 4096];
        let mut raw_count = 0usize;

        for sg in 0..GROUPS_PER_SECTOR {
            let grp = &sector_data[sg * BYTES_PER_GROUP..(sg + 1) * BYTES_PER_GROUP];
            for unit in 0..8 {
                let hi = if unit < 4 { unit } else { unit + 4 };
                let h = grp[hi];
                let filter = ((h >> 4) & 3) as usize;
                let range = (h & 0xF).min(12) as i32;
                let k0 = K0[filter];
                let k1 = K1[filter];
                let ch = if stereo { unit & 1 } else { 0 };

                for s in 0..SAMPLES_PER_UNIT {
                    let byte_idx = 16 + s * 4 + (unit / 2);
                    let byte = grp[byte_idx];
                    let nibble: i32 = if unit & 1 != 0 {
                        ((byte as i8) >> 4) as i32
                    } else {
                        ((byte << 4) as i8 >> 4) as i32
                    };
                    let sample = nibble << (12 - range);
                    let mut out = sample + (k0 * self.prev[ch] + k1 * self.prev2[ch] + 32) / 64;
                    out = out.clamp(-32768, 32767);
                    self.prev2[ch] = self.prev[ch];
                    self.prev[ch] = out;

                    if stereo {
                        let idx = (sg * 4 + unit / 2) * SAMPLES_PER_UNIT + s;
                        if idx < raw_l.len() {
                            if unit & 1 != 0 {
                                raw_r[idx] = out as i16;
                            } else {
                                raw_l[idx] = out as i16;
                            }
                            if idx + 1 > raw_count {
                                raw_count = idx + 1;
                            }
                        }
                    } else {
                        let idx = (sg * 8 + unit) * SAMPLES_PER_UNIT + s;
                        if idx < raw_l.len() {
                            raw_l[idx] = out as i16;
                            raw_r[idx] = out as i16;
                            if idx + 1 > raw_count {
                                raw_count = idx + 1;
                            }
                        }
                    }
                }
            }
        }

        // Linear resample to 44100.
        let out_count = (raw_count as f64 * OUT_RATE as f64 / native_rate) as usize;
        for i in 0..out_count {
            let pos = i as f64 * native_rate / OUT_RATE as f64;
            let idx = pos as usize;
            let frac = pos - idx as f64;
            let (sl, sr) = if idx + 1 < raw_count {
                (
                    (raw_l[idx] as f64 * (1.0 - frac) + raw_l[idx + 1] as f64 * frac) as i16,
                    (raw_r[idx] as f64 * (1.0 - frac) + raw_r[idx + 1] as f64 * frac) as i16,
                )
            } else if idx < raw_count {
                (raw_l[idx], raw_r[idx])
            } else {
                break;
            };
            self.pcm.push(sl);
            self.pcm.push(sr);
        }
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

    pub fn encode_body(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.blocks.to_le_bytes());
        out.extend_from_slice(&self.bytes.to_le_bytes());
        for i in 0..2 {
            out.extend_from_slice(&self.prev[i].to_le_bytes());
            out.extend_from_slice(&self.prev2[i].to_le_bytes());
        }
        out.extend_from_slice(&(self.pcm.len() as u32).to_le_bytes());
        for s in &self.pcm {
            out.extend_from_slice(&s.to_le_bytes());
        }
    }

    pub fn decode_body(
        &mut self,
        p: &[u8],
        o: &mut usize,
    ) -> Result<(), crate::state::SaveStateError> {
        use crate::state::SaveStateError;
        let take = |o: &mut usize, n: usize| -> Result<&[u8], SaveStateError> {
            let s = p.get(*o..*o + n).ok_or(SaveStateError::Truncated)?;
            *o += n;
            Ok(s)
        };
        self.blocks = u64::from_le_bytes(take(o, 8)?.try_into().unwrap());
        self.bytes = u64::from_le_bytes(take(o, 8)?.try_into().unwrap());
        for i in 0..2 {
            self.prev[i] = i32::from_le_bytes(take(o, 4)?.try_into().unwrap());
            self.prev2[i] = i32::from_le_bytes(take(o, 4)?.try_into().unwrap());
        }
        let n = u32::from_le_bytes(take(o, 4)?.try_into().unwrap()) as usize;
        self.pcm = Vec::with_capacity(n);
        for _ in 0..n {
            self.pcm
                .push(i16::from_le_bytes(take(o, 2)?.try_into().unwrap()));
        }
        Ok(())
    }
}
