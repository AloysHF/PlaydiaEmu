//! Recovered 4x4 picture syntax; hardware rounding and rare VLCs need validation.

use super::structure::{read_bits, PictureHeader, PICTURE_HEADER_BYTES, PICTURE_ROWS};

pub const WIDTH: usize = 248;
pub const HEIGHT: usize = PICTURE_ROWS * 8;
pub const BLOCKS_PER_ROW: usize = 31 * 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Header,
    UnsupportedHeader,
    Truncated,
    InvalidCode,
    CoefficientOverflow,
    RowMarker,
    Terminator,
    Padding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError {
    pub kind: ErrorKind,
    pub bit: usize,
    pub row: usize,
    pub block: usize,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
    row: usize,
    block: usize,
}

impl Reader<'_> {
    fn error(&self, kind: ErrorKind) -> DecodeError {
        DecodeError {
            kind,
            bit: self.pos,
            row: self.row,
            block: self.block,
        }
    }

    fn read(&mut self, n: usize) -> Result<u32, DecodeError> {
        let value =
            read_bits(self.data, self.pos, n).ok_or_else(|| self.error(ErrorKind::Truncated))?;
        self.pos += n;
        Ok(value)
    }

    fn symbol(&mut self) -> Result<Option<(usize, i32)>, DecodeError> {
        let available = (self.data.len() * 8).saturating_sub(self.pos).min(13);
        let prefix = read_bits(self.data, self.pos, available).unwrap_or(0) << (13 - available);
        let entry = LOOKUP[prefix as usize];
        match entry.bits {
            0 => Err(self.error(if available < 13 {
                ErrorKind::Truncated
            } else {
                ErrorKind::InvalidCode
            })),
            2 if entry.level == 0 => {
                self.read(2)?;
                Ok(None)
            }
            6 if entry.level == 0 => {
                self.read(6)?;
                let run = self.read(4)? as usize;
                let raw = self.read(10)? as i32;
                Ok(Some((run, (raw << 22) >> 22)))
            }
            n => {
                self.read(n as usize)?;
                let sign = self.read(1)?;
                Ok(Some((
                    entry.run as usize,
                    i32::from(entry.level) * if sign == 0 { 1 } else { -1 },
                )))
            }
        }
    }

    fn coefficients(&mut self) -> Result<[i32; 16], DecodeError> {
        let mut coefficients = [0; 16];
        let mut position = 0;
        while position < 16 {
            let Some((run, level)) = self.symbol()? else {
                break;
            };
            position += run;
            if position >= 16 {
                return Err(self.error(ErrorKind::CoefficientOverflow));
            }
            coefficients[SCAN[position]] = level;
            position += 1;
        }
        Ok(coefficients)
    }
}

const SCAN: [usize; 16] = [0, 1, 4, 8, 5, 2, 3, 6, 9, 12, 13, 10, 7, 11, 14, 15];
const BASIS: [[i64; 4]; 4] = [
    [8192, 8192, 8192, 8192],
    [10703, 4433, -4433, -10703],
    [8192, -8192, -8192, 8192],
    [4433, -10703, 10703, -4433],
];

fn inverse(coefficients: &[i32; 16], quant: &[u8; 16], factor: u8) -> [i32; 16] {
    let mut intermediate = [[0i64; 4]; 4];
    for (v, line) in intermediate.iter_mut().enumerate() {
        for (x, value) in line.iter_mut().enumerate() {
            for (u, basis) in BASIS.iter().enumerate() {
                let i = v * 4 + u;
                *value +=
                    i64::from(coefficients[i]) * i64::from(quant[i]) * i64::from(factor) * basis[x];
            }
        }
    }
    let mut pixels = [0; 16];
    for y in 0..4 {
        for x in 0..4 {
            let sum: i64 = (0..4).map(|v| intermediate[v][x] * BASIS[v][y]).sum();
            pixels[y * 4 + x] = ((sum + (1 << 33)) >> 34) as i32;
        }
    }
    pixels
}

/// Decode exactly one picture, rejecting incomplete rows and trailing entropy.
pub fn decode(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    decode_inner(data, Output::Rgb555)
}

/// Export native pixels without reducing the reconstructed channels to five bits.
pub fn decode_rgb888(data: &[u8]) -> Result<Vec<u8>, DecodeError> {
    decode_inner(data, Output::Rgb888)
}

/// Check entropy and framing without running the pixel transform.
pub fn validate(data: &[u8]) -> Result<(), DecodeError> {
    decode_inner(data, Output::Validate).map(|_| ())
}

#[derive(Clone, Copy)]
enum Output {
    Validate,
    Rgb555,
    Rgb888,
}

fn decode_inner(data: &[u8], output: Output) -> Result<Vec<u8>, DecodeError> {
    let render = !matches!(output, Output::Validate);
    let mut reader = Reader {
        data,
        pos: 0,
        row: 0,
        block: 0,
    };
    let header = PictureHeader::parse(data).ok_or_else(|| reader.error(ErrorKind::Header))?;
    if header.picture_type != 1 || header.quantizer_shift != 0 || header.factor == 0 {
        return Err(reader.error(ErrorKind::UnsupportedHeader));
    }
    reader.pos = PICTURE_HEADER_BYTES * 8;
    let mut planes = if render {
        [
            vec![0i32; WIDTH * HEIGHT],
            vec![0; WIDTH * HEIGHT / 4],
            vec![0; WIDTH * HEIGHT / 4],
        ]
    } else {
        [Vec::new(), Vec::new(), Vec::new()]
    };
    for row in 0..PICTURE_ROWS {
        reader.row = row + 1;
        reader.block = 0;
        if reader.read(19)? != (0x20 << 5) | (row as u32 + 1) {
            return Err(reader.error(ErrorKind::RowMarker));
        }
        let mut predictors = [0; 3];
        for mb in 0..31 {
            for block in 0..6 {
                reader.block = mb * 6 + block;
                let component = if block < 4 { 0 } else { block - 3 };
                let mut coefficients = reader.coefficients()?;
                coefficients[0] += predictors[component];
                // Y1 predicts Y2/Y3/Y4 and the next macroblock's Y1.
                if block == 0 || component != 0 {
                    predictors[component] = coefficients[0];
                }
                if !render {
                    continue;
                }
                let quant = if component == 0 {
                    &header.quant_luma
                } else {
                    &header.quant_chroma
                };
                let pixels = inverse(&coefficients, quant, header.factor);
                let stride = if component == 0 { WIDTH } else { WIDTH / 2 };
                let x = if component == 0 {
                    mb * 8 + (block % 2) * 4
                } else {
                    mb * 4
                };
                let y = if component == 0 {
                    row * 8 + (block / 2) * 4
                } else {
                    row * 4
                };
                for py in 0..4 {
                    planes[component][(y + py) * stride + x..(y + py) * stride + x + 4]
                        .copy_from_slice(&pixels[py * 4..py * 4 + 4]);
                }
            }
        }
    }
    if reader.read(14)? != 0x21 {
        return Err(reader.error(ErrorKind::Terminator));
    }
    // The observed trailer has at most 15 zero bits before byte-aligned FF fill.
    let used = data.iter().rposition(|&b| b != 0xFF).map_or(0, |p| p + 1) * 8;
    let padding = used
        .checked_sub(reader.pos)
        .filter(|&n| n <= 15)
        .ok_or_else(|| reader.error(ErrorKind::Padding))?;
    if reader.read(padding)? != 0 {
        return Err(reader.error(ErrorKind::Padding));
    }
    if !render {
        return Ok(Vec::new());
    }
    let channels = if matches!(output, Output::Rgb888) {
        3
    } else {
        2
    };
    let mut rgb = Vec::with_capacity(WIDTH * HEIGHT * channels);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let luma = planes[0][y * WIDTH + x] + 128;
            let cb = i64::from(planes[1][(y / 2) * (WIDTH / 2) + x / 2]);
            let cr = i64::from(planes[2][(y / 2) * (WIDTH / 2) + x / 2]);
            let r = i64::from(luma) + ((91881 * cr) >> 16);
            let g = i64::from(luma) - ((22554 * cb + 46802 * cr) >> 16);
            let b = i64::from(luma) + ((116130 * cb) >> 16);
            let (r, g, b) = (
                r.clamp(0, 255) as u8,
                g.clamp(0, 255) as u8,
                b.clamp(0, 255) as u8,
            );
            if matches!(output, Output::Rgb888) {
                rgb.extend_from_slice(&[r, g, b]);
            } else {
                rgb.extend_from_slice(&super::rgb888_to_555(r, g, b).to_le_bytes());
            }
        }
    }
    Ok(rgb)
}

#[derive(Clone, Copy)]
struct Entry {
    bits: u8,
    run: u8,
    level: i16,
}

const fn lookup() -> [Entry; 8192] {
    let mut table = [Entry {
        bits: 0,
        run: 0,
        level: 0,
    }; 8192];
    let mut i = 0;
    while i < 8192 {
        if i >> 11 == 1 {
            table[i].bits = 2;
        }
        if i >> 7 == 8 {
            table[i].bits = 6;
        }
        let mut c = 0;
        while c < COEFFICIENT_CODES.len() {
            let (code, bits, run, level) = COEFFICIENT_CODES[c];
            if i >> (13 - bits) == code as usize {
                table[i] = Entry { bits, run, level };
            }
            c += 1;
        }
        i += 1;
    }
    table
}

static LOOKUP: [Entry; 8192] = lookup();
// Codewords inferred from disc row and coefficient boundaries.
const COEFFICIENT_CODES: [(u16, u8, u8, i16); 63] = [
    (0b0000000010000, 13, 0, 25),
    (0b0000000010001, 13, 5, 4),
    (0b0000000010010, 13, 0, 24),
    (0b0000000010011, 13, 0, 23),
    (0b0000000010100, 13, 3, 8),
    (0b0000000010101, 13, 3, 7),
    (0b0000000010110, 13, 3, 6),
    (0b0000000010111, 13, 2, 8),
    (0b0000000011000, 13, 2, 7),
    (0b0000000011001, 13, 2, 6),
    (0b0000000011010, 13, 1, 9),
    (0b0000000011011, 13, 1, 8),
    (0b0000000011100, 13, 1, 7),
    (0b0000000011101, 13, 0, 22),
    (0b0000000011110, 13, 0, 21),
    (0b0000000011111, 13, 0, 20),
    (0b000000010000, 12, 9, 1),
    (0b000000010001, 12, 8, 1),
    (0b000000010010, 12, 7, 1),
    (0b000000010011, 12, 5, 3),
    (0b000000010100, 12, 5, 2),
    (0b000000010101, 12, 4, 5),
    (0b000000010110, 12, 4, 4),
    (0b000000010111, 12, 4, 3),
    (0b000000011000, 12, 3, 5),
    (0b000000011001, 12, 3, 4),
    (0b000000011010, 12, 2, 5),
    (0b000000011011, 12, 1, 6),
    (0b000000011100, 12, 0, 19),
    (0b000000011101, 12, 0, 18),
    (0b000000011110, 12, 0, 17),
    (0b000000011111, 12, 0, 16),
    (0b0000001000, 10, 3, 3),
    (0b0000001001, 10, 2, 4),
    (0b0000001010, 10, 2, 3),
    (0b0000001011, 10, 1, 5),
    (0b0000001100, 10, 0, 15),
    (0b0000001101, 10, 0, 14),
    (0b0000001110, 10, 0, 13),
    (0b0000001111, 10, 0, 12),
    (0b00000100, 8, 6, 1),
    (0b00000101, 8, 4, 2),
    (0b00000110, 8, 3, 2),
    (0b00000111, 8, 1, 4),
    (0b0000100, 7, 5, 1),
    (0b0000101, 7, 2, 2),
    (0b0000110, 7, 0, 8),
    (0b0000111, 7, 0, 7),
    (0b000100, 6, 4, 1),
    (0b000101, 6, 1, 2),
    (0b000110, 6, 0, 6),
    (0b000111, 6, 0, 5),
    (0b00100100, 8, 1, 3),
    (0b00100101, 8, 0, 11),
    (0b00100110, 8, 0, 10),
    (0b00100111, 8, 0, 9),
    (0b00101, 5, 3, 1),
    (0b00110, 5, 2, 1),
    (0b00111, 5, 0, 4),
    (0b1000, 4, 1, 1),
    (0b1001, 4, 0, 3),
    (0b101, 3, 0, 2),
    (0b11, 2, 0, 1),
];
