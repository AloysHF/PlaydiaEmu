//! Playdia CDXA video decoder (HLE).
//!
//! Packet format confirmed on Redump titles:
//! `00 80 04 | QS | qtable[16]×2 | 00 80 24 | 00 | dcY | dcCb | dcCr | flags | bitstream`
//!
//! Default codec profile follows reverse-engineered defaults that produce
//! structured output on real discs (192×144 4:2:0, MB-interleaved, fixed
//! AC count, init+diff DC, MPEG-1-like size VLC).

use crate::bitstream::LsbBitReader;
use crate::cd::XaPacket;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;
pub const ENC_W: usize = 192;
pub const ENC_H: usize = 144;
const ACC_CAP: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoPacketHeader {
    pub qscale: u8,
    pub quant_luma: [u8; 16],
    pub quant_chroma: [u8; 16],
    pub segment_code: u8,
    pub flags: u8,
}

pub fn parse_packet_header(buf: &[u8]) -> Option<VideoPacketHeader> {
    if buf.len() < 40 || buf[..3] != [0x00, 0x80, 0x04] || buf[36..38] != [0x00, 0x80] {
        return None;
    }
    let mut quant_luma = [0; 16];
    let mut quant_chroma = [0; 16];
    quant_luma.copy_from_slice(&buf[4..20]);
    quant_chroma.copy_from_slice(&buf[20..36]);
    Some(VideoPacketHeader {
        qscale: buf[3],
        quant_luma,
        quant_chroma,
        segment_code: buf[38],
        flags: buf[39],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    None,
    Still,
    Progressive,
}

/// Tunable decode profile (defaults = reverse-engineered working set).
#[derive(Debug, Clone, Copy)]
pub struct CodecParams {
    pub width: usize,
    pub height: usize,
    pub bs_offset: usize,
    pub dc_mode_accum: bool,
    pub dc_scale: i32,
    pub ac_count: usize,
    /// 0 = raw AC (no dequant), 1 = q*qscale/8
    pub ac_dequant: u8,
    pub use_eob: bool,
    pub level_shift: i32,
    pub chroma_420: bool,
    pub mb_interleaved: bool,
    /// 0=MPEG zigzag, 1=alt, 2=raster
    pub scan_order: u8,
    /// Bit-reverse each byte before reading (AK8000 spec).
    pub lsb_first: bool,
}

impl Default for CodecParams {
    fn default() -> Self {
        Self {
            width: ENC_W,
            height: ENC_H,
            // After 00 80 24 start-code (WizzardSK: body ~byte 40-44).
            bs_offset: 40,
            dc_mode_accum: true,
            dc_scale: 8,
            ac_count: 10,
            ac_dequant: 0,
            use_eob: false,
            level_shift: 0,
            chroma_420: true,
            mb_interleaved: true,
            scan_order: 0,
            // WizzardSK documents MSB-first modified MPEG-1 DC VLC.
            lsb_first: false,
        }
    }
}

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];
const ZIGZAG_ALT: [usize; 64] = [
    0, 8, 16, 24, 1, 9, 2, 10, 17, 25, 32, 40, 48, 56, 57, 49, 41, 33, 26, 18, 3, 11, 4, 12, 19,
    27, 34, 42, 50, 58, 35, 43, 51, 59, 20, 28, 5, 13, 6, 14, 21, 29, 36, 44, 52, 60, 37, 45, 53,
    61, 22, 30, 7, 15, 23, 31, 38, 46, 54, 62, 39, 47, 55, 63,
];
const ZIGZAG_RASTER: [usize; 64] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63,
];

fn scan_table(order: u8) -> &'static [usize; 64] {
    match order {
        1 => &ZIGZAG_ALT,
        2 => &ZIGZAG_RASTER,
        _ => &ZIGZAG,
    }
}

const COS: [[i32; 8]; 8] = [
    [1448, 1448, 1448, 1448, 1448, 1448, 1448, 1448],
    [2009, 1703, 1138, 400, -400, -1138, -1703, -2009],
    [1892, 784, -784, -1892, -1892, -784, 784, 1892],
    [1703, -400, -2009, -1138, 1138, 2009, 400, -1703],
    [1448, -1448, -1448, 1448, 1448, -1448, -1448, 1448],
    [1138, -2009, 400, 1703, -1703, -400, 2009, -1138],
    [784, -1892, 1892, -784, -784, 1892, -1892, 784],
    [400, -1138, 1703, -2009, 2009, -1703, 1138, -400],
];

#[derive(Debug, Clone)]
pub struct VideoDecoder {
    pub framebuffer: Vec<u16>,
    pub frames_decoded: u64,
    pub frames_failed: u64,
    pub bytes_ingested: u64,
    pub last_kind: FrameKind,
    pub last_packet_len: usize,
    pub acc: Vec<u8>,
    pub acc_sectors: u32,
    pub last_qs: u8,
    pub last_qtable: [u8; 16],
    pub params: CodecParams,
    pub last_blocks: usize,
    /// Cache: packet CRC → all complete frames in packet.
    cache: std::collections::HashMap<u32, Vec<Vec<u8>>>,
    /// Frames from the current packet not yet presented.
    pending: std::collections::VecDeque<Vec<u8>>,
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
            frames_failed: 0,
            bytes_ingested: 0,
            last_kind: FrameKind::None,
            last_packet_len: 0,
            acc: Vec::new(),
            acc_sectors: 0,
            last_qs: 0,
            last_qtable: [0; 16],
            params: CodecParams::default(),
            last_blocks: 0,
            cache: std::collections::HashMap::new(),
            pending: std::collections::VecDeque::new(),
        }
    }

    pub fn reset(&mut self) {
        let params = self.params;
        *self = Self {
            params,
            ..Self::new()
        };
    }

    pub fn ingest_packet(&mut self, packet: &XaPacket) {
        match packet {
            XaPacket::Video { data, .. } => {
                self.bytes_ingested += data.len() as u64;
                if data.len() > 1 {
                    let chunk = &data[1..];
                    if self.acc.len() + chunk.len() <= ACC_CAP {
                        self.acc.extend_from_slice(chunk);
                        self.acc_sectors += 1;
                    }
                }
            }
            XaPacket::FrameEnd { .. } => {
                let starts = self.acc.len() >= 4
                    && self.acc[0] == 0x00
                    && self.acc[1] == 0x80
                    && self.acc[2] == 0x04;
                if starts && self.acc.len() >= 44 {
                    self.decode_accumulated();
                } else if !self.acc.is_empty() {
                    self.frames_failed += 1;
                }
                self.acc.clear();
                self.acc_sectors = 0;
            }
            XaPacket::SceneReset { .. } => {
                self.discard_pending();
            }
            _ => {}
        }
    }

    pub fn flush_frame(&mut self) {}

    /// Drop incomplete and queued pictures when playback seeks to a new scene.
    pub fn discard_pending(&mut self) {
        self.acc.clear();
        self.acc_sectors = 0;
        self.pending.clear();
    }

    fn decode_accumulated(&mut self) {
        let buf = std::mem::take(&mut self.acc);
        self.last_packet_len = buf.len();
        let crc = crate::state::crc32(&buf);
        if let Some(frames) = self.cache.get(&crc).cloned() {
            for f in frames {
                self.pending.push_back(f);
            }
            self.last_kind = FrameKind::Still;
            self.acc = Vec::new();
            return;
        }
        let frames = decode_packet_frames(&buf, self.params);
        if frames.is_empty() {
            self.frames_failed += 1;
        } else {
            if buf.len() > 3 {
                self.last_qs = buf[3];
            }
            if buf.len() >= 20 {
                self.last_qtable.copy_from_slice(&buf[4..20]);
            }
            self.last_blocks = frames.last().map(|f| f.1).unwrap_or(0);
            let rgb_list: Vec<Vec<u8>> = frames.iter().map(|f| f.0.clone()).collect();
            self.cache.insert(crc, rgb_list);
            for (rgb, _) in frames {
                self.pending.push_back(rgb);
            }
            self.last_kind = FrameKind::Progressive;
        }
        self.acc = Vec::new();
    }

    /// Present the next pending frame (call once per host frame).
    pub fn present_next(&mut self) -> bool {
        if let Some(rgb) = self.pending.pop_front() {
            self.blit_encoded(&rgb);
            self.frames_decoded += 1;
            true
        } else {
            false
        }
    }

    fn blit_encoded(&mut self, rgb: &[u8]) {
        if rgb.len() < ENC_W * ENC_H * 2 {
            return;
        }
        let ox = (WIDTH - ENC_W) / 2;
        let oy = (HEIGHT - ENC_H) / 2;
        for y in 0..ENC_H {
            for x in 0..ENC_W {
                let i = (y * ENC_W + x) * 2;
                let px = u16::from_le_bytes([rgb[i], rgb[i + 1]]);
                self.framebuffer[(oy + y) * WIDTH + (ox + x)] = px;
            }
        }
    }

    pub fn framebuffer_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.framebuffer.len() * 2);
        for px in &self.framebuffer {
            out.extend_from_slice(&px.to_le_bytes());
        }
        out
    }

    pub fn dump_ppm(&self, path: &std::path::Path) -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        write!(f, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
        for px in &self.framebuffer {
            let (r, g, b) = rgb555_to_rgb888(*px);
            f.write_all(&[r, g, b])?;
        }
        Ok(())
    }

    pub fn crc(&self) -> u32 {
        crate::state::crc32(&self.framebuffer_bytes())
    }
}

fn idct_block(coeff: &[i32; 64], out: &mut [u8; 64], level_shift: i32, scan: &[usize; 64]) {
    let mut matrix = [[0i32; 8]; 8];
    for i in 0..64 {
        let z = scan[i];
        matrix[z / 8][z % 8] = coeff[i];
    }
    let mut temp = [[0i32; 8]; 8];
    for i in 0..8 {
        for j in 0..8 {
            let mut sum = 0i32;
            for k in 0..8 {
                sum += matrix[i][k] * COS[k][j];
            }
            temp[i][j] = (sum + 2048) >> 12;
        }
    }
    for j in 0..8 {
        for i in 0..8 {
            let mut sum = 0i32;
            for k in 0..8 {
                sum += temp[k][j] * COS[k][i];
            }
            out[i * 8 + j] = (((sum + 2048) >> 12) + level_shift).clamp(0, 255) as u8;
        }
    }
}

fn rgb888_to_555(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g5 = (g >> 3) as u16;
    let b5 = (b >> 3) as u16;
    r5 | (g5 << 5) | (b5 << 10)
}

/// Decode all complete frames inside one F1 packet (typically ~3).
pub fn decode_packet_frames(buf: &[u8], p: CodecParams) -> Vec<(Vec<u8>, usize)> {
    if buf.len() < 44 {
        return Vec::new();
    }
    let Some(header) = parse_packet_header(buf) else {
        return Vec::new();
    };
    let qscale = header.qscale.max(1) as i32;
    let qtable = header.quant_luma;
    let mut qm = [[0i32; 8]; 8];
    for i in 0..8 {
        for j in 0..8 {
            qm[i][j] = qtable[(i / 2) * 4 + (j / 2)] as i32;
        }
    }
    let dc_init = [buf[40] as i32, buf[41] as i32, buf[42] as i32];
    let bso = p.bs_offset.min(buf.len());
    let mut end = buf.len();
    while end > bso && buf[end - 1] == 0xFF {
        end -= 1;
    }
    if end <= bso {
        return Vec::new();
    }
    // AK8000 entropy body is LSB-first after start-codes.
    let mut bs = LsbBitReader::new(&buf[bso..end]);
    let mw = p.width / 16;
    let mh = p.height / 16;
    let bpm = if p.chroma_420 { 6 } else { 4 };
    if mw == 0 || mh == 0 {
        return Vec::new();
    }
    let nblocks = mw * mh * bpm;
    let scan = scan_table(p.scan_order);
    let mut out = Vec::new();

    for _frame in 0..6 {
        let mut y_plane = vec![(128 + p.level_shift).clamp(0, 255) as u8; ENC_W * ENC_H];
        let mut cb_plane = vec![128u8; (ENC_W / 2) * (ENC_H / 2)];
        let mut cr_plane = vec![128u8; (ENC_W / 2) * (ENC_H / 2)];
        let mut dc_pred = dc_init;
        let mut blocks_ok = 0usize;

        'mb: for mb_y in 0..mh {
            for mb_x in 0..mw {
                for bl in 0..bpm {
                    let comp = if bl < 4 {
                        0
                    } else if bl == 4 {
                        1
                    } else {
                        2
                    };
                    let Some(diff) = bs.read_vlc() else {
                        break 'mb;
                    };
                    let dc_val = if p.dc_mode_accum {
                        dc_pred[comp] = (dc_pred[comp] + diff).clamp(0, 255);
                        dc_pred[comp]
                    } else {
                        (dc_init[comp] + diff).clamp(0, 255)
                    };
                    let mut coeff = [0i32; 64];
                    coeff[0] = dc_val * p.dc_scale;
                    if p.use_eob {
                        for k in 1..64 {
                            let Some(v) = bs.read_vlc() else {
                                break 'mb;
                            };
                            if v == 0 {
                                break;
                            }
                            let mut val = v;
                            if p.ac_dequant == 1 {
                                let pos = scan[k];
                                val = val * qm[pos / 8][pos % 8] * qscale / 8;
                            } else if p.ac_dequant == 2 {
                                let pos = scan[k];
                                val *= qm[pos / 8][pos % 8];
                            }
                            coeff[k] = val;
                        }
                    } else {
                        for k in 1..=p.ac_count.min(63) {
                            let Some(v) = bs.read_vlc() else {
                                break 'mb;
                            };
                            let mut val = v;
                            if p.ac_dequant == 1 {
                                let pos = scan[k];
                                val = val * qm[pos / 8][pos % 8] * qscale / 8;
                            } else if p.ac_dequant == 2 {
                                let pos = scan[k];
                                val *= qm[pos / 8][pos % 8];
                            }
                            coeff[k] = val;
                        }
                    }
                    let mut blk = [0u8; 64];
                    idct_block(&coeff, &mut blk, p.level_shift, scan);
                    blit_block(
                        &mut y_plane,
                        &mut cb_plane,
                        &mut cr_plane,
                        mb_x,
                        mb_y,
                        bl,
                        &blk,
                    );
                    blocks_ok += 1;
                }
            }
        }
        if blocks_ok == 0 {
            break;
        }
        if blocks_ok >= nblocks * 9 / 10 {
            out.push((compose(&y_plane, &cb_plane, &cr_plane), blocks_ok));
        }
        if blocks_ok < nblocks || bs.bits().saturating_sub(bs.pos()) < 64 {
            break;
        }
    }
    out
}

/// Decode packet → last complete frame.
pub fn decode_packet(buf: &[u8], p: CodecParams) -> Option<(Vec<u8>, usize)> {
    decode_packet_frames(buf, p).into_iter().next_back()
}

fn blit_block(
    y: &mut [u8],
    cb: &mut [u8],
    cr: &mut [u8],
    mb_x: usize,
    mb_y: usize,
    bi: usize,
    blk: &[u8; 64],
) {
    if bi < 4 {
        let sx = (bi & 1) * 8;
        let sy = (bi >> 1) * 8;
        let x0 = mb_x * 16 + sx;
        let y0 = mb_y * 16 + sy;
        for row in 0..8 {
            for col in 0..8 {
                let x = x0 + col;
                let yy = y0 + row;
                if x < ENC_W && yy < ENC_H {
                    y[yy * ENC_W + x] = blk[row * 8 + col];
                }
            }
        }
    } else {
        let plane = if bi == 4 { cb } else { cr };
        let cw = ENC_W / 2;
        let x0 = mb_x * 8;
        let y0 = mb_y * 8;
        for row in 0..8 {
            for col in 0..8 {
                let x = x0 + col;
                let yy = y0 + row;
                if x < cw && yy < ENC_H / 2 {
                    plane[yy * cw + x] = blk[row * 8 + col];
                }
            }
        }
    }
}

fn compose(y: &[u8], cb: &[u8], cr: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; ENC_W * ENC_H * 2];
    let cw = ENC_W / 2;
    for row in 0..ENC_H {
        for col in 0..ENC_W {
            let yy = y[row * ENC_W + col] as f32;
            let cbi = cb[(row / 2) * cw + (col / 2)] as f32;
            let cri = cr[(row / 2) * cw + (col / 2)] as f32;
            let r = yy + 1.402 * (cri - 128.0);
            let g = yy - 0.344136 * (cbi - 128.0) - 0.714136 * (cri - 128.0);
            let b = yy + 1.772 * (cbi - 128.0);
            let px = rgb888_to_555(
                r.clamp(0.0, 255.0) as u8,
                g.clamp(0.0, 255.0) as u8,
                b.clamp(0.0, 255.0) as u8,
            );
            let i = (row * ENC_W + col) * 2;
            out[i..i + 2].copy_from_slice(&px.to_le_bytes());
        }
    }
    out
}

pub fn rgb555_to_rgb888(v: u16) -> (u8, u8, u8) {
    let r = (v & 0x1F) as u8;
    let g = ((v >> 5) & 0x1F) as u8;
    let b = ((v >> 10) & 0x1F) as u8;
    (
        (r << 3) | (r >> 2),
        (g << 3) | (g >> 2),
        (b << 3) | (b >> 2),
    )
}
