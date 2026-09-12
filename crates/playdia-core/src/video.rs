//! Playdia CDXA video path: F1 sector assembly + proprietary DCT frame decode.
//!
//! Native output surface is 320×240 RGB555. Encoded frames are typically a
//! centered 192×144 (or similar) 4:2:0 image inside that surface.
//!
//! Packet assembly uses Mode2 Form1 data (channel 0, submode Data bit):
//! F1 sectors append payload[1..] into an accumulator; F2 (submode 0x08)
//! marks frame end (decode if buffer starts with 00 80 04); F3 resets the
//! accumulator.
//!
//! The bitstream decoder implements a reverse-engineered MPEG-1-like DC VLC
//! and zig-zag AC path with integer IDCT. Exact AC dequant tables are still
//! being locked against real-hardware reference frames.

use crate::cd::XaPacket;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;
pub const ENC_W: usize = 192;
pub const ENC_H: usize = 144;
const ACC_CAP: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    None,
    Still,
    Progressive,
}

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// cos((2n+1)kπ/16) × 2048, k=0 scaled by 1/√2.
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
    pub bytes_ingested: u64,
    pub last_kind: FrameKind,
    pub acc: Vec<u8>,
    pub acc_sectors: u32,
    pub pending_decode: bool,
    pub decode_errors: u64,
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
            acc: Vec::new(),
            acc_sectors: 0,
            pending_decode: false,
            decode_errors: 0,
        }
    }

    pub fn reset(&mut self) {
        self.framebuffer.fill(0);
        self.frames_decoded = 0;
        self.bytes_ingested = 0;
        self.last_kind = FrameKind::None;
        self.acc.clear();
        self.acc_sectors = 0;
        self.pending_decode = false;
        self.decode_errors = 0;
    }

    pub fn ingest_packet(&mut self, packet: &XaPacket) {
        match packet {
            XaPacket::Video { data, .. } => {
                self.bytes_ingested += data.len() as u64;
                // Payload[0] is the F1 marker; remaining bytes are bitstream.
                if data.len() > 1 {
                    let chunk = &data[1..];
                    if self.acc.len() + chunk.len() <= ACC_CAP {
                        self.acc.extend_from_slice(chunk);
                        self.acc_sectors += 1;
                    }
                } else {
                    // Classic file-id path without marker: treat whole payload.
                    if self.acc.len() + data.len() <= ACC_CAP {
                        self.acc.extend_from_slice(data);
                        self.acc_sectors += 1;
                    }
                }
            }
            XaPacket::FrameEnd { data, .. } => {
                let starts = self.acc.len() >= 4
                    && self.acc[0] == 0x00
                    && self.acc[1] == 0x80
                    && self.acc[2] == 0x04;
                if starts && self.acc_sectors >= 6 {
                    self.decode_accumulated();
                }
                self.acc.clear();
                self.acc_sectors = 0;
                let _ = data;
            }
            XaPacket::SceneReset { .. } => {
                self.acc.clear();
                self.acc_sectors = 0;
            }
            _ => {}
        }
    }

    pub fn flush_frame(&mut self) {
        // End-of-frame is driven by F2 markers; nothing to force here.
    }

    fn decode_accumulated(&mut self) {
        let buf = std::mem::take(&mut self.acc);
        match decode_packet(&buf) {
            Some(rgb) => {
                self.blit_encoded(&rgb);
                self.frames_decoded += 1;
                self.last_kind = FrameKind::Progressive;
            }
            None => {
                self.decode_errors += 1;
            }
        }
        self.acc = Vec::new();
    }

    fn blit_encoded(&mut self, rgb: &[u8]) {
        // rgb is ENC_W×ENC_H RGB555 LE or we store RGB888 and convert.
        // decode_packet returns RGB555 LE packed ENC_W*ENC_H*2.
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

    pub fn crc(&self) -> u32 {
        crate::state::crc32(&self.framebuffer_bytes())
    }
}

struct Bs<'a> {
    data: &'a [u8],
    pos: usize,
    bits: usize,
}

impl<'a> Bs<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            bits: data.len() * 8,
        }
    }

    fn get1(&mut self) -> u32 {
        if self.pos >= self.bits {
            return 0;
        }
        let p = self.pos;
        self.pos += 1;
        ((self.data[p >> 3] >> (7 - (p & 7))) & 1) as u32
    }

    fn read(&mut self, n: u32) -> i32 {
        let mut v = 0i32;
        for _ in 0..n {
            v = (v << 1) | self.get1() as i32;
        }
        v
    }

    /// MPEG-1 luminance DC size VLC with Playdia size-7/8 6-bit forms.
    fn read_dc_size(&mut self) -> Option<i32> {
        if self.pos >= self.bits {
            return None;
        }
        
        let size = if self.get1() == 0 {
            if self.get1() == 1 { 2 } else { 1 }
        } else if self.get1() == 0 {
            if self.get1() == 1 { 3 } else { 0 }
        } else if self.get1() == 0 {
            4
        } else if self.get1() == 0 {
            5
        } else if self.get1() == 0 {
            6
        } else {
            if self.get1() == 1 { 8 } else { 7 }
        };
        Some(size)
    }

    fn read_dc_diff(&mut self) -> Option<i32> {
        let size = self.read_dc_size()?;
        if size == 0 {
            return Some(0);
        }
        let val = self.read(size as u32);
        if val < (1 << (size - 1)) {
            Some(val - ((1 << size) - 1))
        } else {
            Some(val)
        }
    }
}

fn idct_block(coeff: &[i32; 64], out: &mut [u8; 64]) {
    let mut matrix = [[0i32; 8]; 8];
    for i in 0..64 {
        let z = ZIGZAG[i];
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
            out[i * 8 + j] = ((sum + 2048) >> 12).clamp(0, 255) as u8;
        }
    }
}

fn rgb888_to_555(r: u8, g: u8, b: u8) -> u16 {
    let r5 = (r >> 3) as u16;
    let g5 = (g >> 3) as u16;
    let b5 = (b >> 3) as u16;
    r5 | (g5 << 5) | (b5 << 10)
}

/// Decode an assembled F1 packet into ENC_W×ENC_H RGB555 LE.
pub fn decode_packet(buf: &[u8]) -> Option<Vec<u8>> {
    if buf.len() < 40 {
        return None;
    }
    if !(buf[0] == 0x00 && buf[1] == 0x80 && buf[2] == 0x04) {
        return None;
    }
    let qscale = buf[3] as i32;
    let mut qtable = [0u8; 16];
    qtable.copy_from_slice(&buf[4..20]);
    let bs_off = 40usize.min(buf.len());
    let mut bs = Bs::new(&buf[bs_off..]);

    // Macroblocks: 12×9 for 192×144, 6 blocks (4Y+Cb+Cr).
    let mw = 12usize;
    let mh = 9usize;
    let bpm = 6usize;
    let mut y_plane = vec![0u8; ENC_W * ENC_H];
    let mut cb_plane = vec![0u8; (ENC_W / 2) * (ENC_H / 2)];
    let mut cr_plane = vec![0u8; (ENC_W / 2) * (ENC_H / 2)];
    let mut dc_pred = [0i32; 3usize];
    let mut blocks_ok = 0usize;

    for mb_y in 0..mh {
        for mb_x in 0..mw {
            for bi in 0..bpm {
                let mut coeff = [0i32; 64];
                // DC
                let Some(diff) = bs.read_dc_diff() else {
                    return if blocks_ok > 0 {
                        Some(compose(&y_plane, &cb_plane, &cr_plane))
                    } else {
                        None
                    };
                };
                let comp = if bi < 4 {
                    0
                } else if bi == 4 {
                    1
                } else {
                    2
                };
                dc_pred[comp] += diff;
                coeff[0] = dc_pred[comp] * 8;

                // AC until EOB (size0 after DC) — limit to 63.
                for zi in 1..64 {
                    let Some(diff) = bs.read_dc_diff() else {
                        break;
                    };
                    // For AC we reuse the same VLC as "size" then signed value.
                    // read_dc_diff already returns signed residual; treat 0 as EOB.
                    if diff == 0 {
                        break;
                    }
                    let pos = ZIGZAG[zi];
                    let q = qtable[((pos / 4) + (pos % 4)) % 16] as i32;
                    let q = if q == 0 { 1 } else { q };
                    coeff[zi] = diff * q * qscale / 8;
                }

                let mut blk = [0u8; 64];
                idct_block(&coeff, &mut blk);
                blit_block(
                    &mut y_plane,
                    &mut cb_plane,
                    &mut cr_plane,
                    mb_x,
                    mb_y,
                    bi,
                    &blk,
                );
                blocks_ok += 1;
            }
        }
    }
    let _ = bpm;
    Some(compose(&y_plane, &cb_plane, &cr_plane))
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
            // BT.601-ish limited range.
            let r = yy + 1.402 * (cri - 128.0);
            let g = yy - 0.344136 * (cbi - 128.0) - 0.714136 * (cri - 128.0);
            let b = yy + 1.772 * (cbi - 128.0);
            let r = r.clamp(0.0, 255.0) as u8;
            let g = g.clamp(0.0, 255.0) as u8;
            let b = b.clamp(0.0, 255.0) as u8;
            let px = rgb888_to_555(r, g, b);
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
