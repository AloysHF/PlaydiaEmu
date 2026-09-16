//! MSB-first picture framing; entropy symbols and pixels remain unverified.

pub const PICTURE_HEADER_BYTES: usize = 36;
pub const PICTURE_ROWS: usize = 27;
pub const VIDEO_PACKET_CAP: usize = 256 * 1024;

/// Strip sector control bytes without including Form2/ECC bytes.
pub fn video_fragment(data: &[u8]) -> &[u8] {
    let offset = match data.first() {
        Some(0xF1) => 1,
        Some(0xF2) => 0x23,
        _ => return &[],
    };
    data.get(offset..data.len().min(2048)).unwrap_or(&[])
}

/// Observed F3 filler carries a two-byte word followed by FF padding.
pub fn is_video_padding(data: &[u8]) -> bool {
    data.len() == 2048 && data[0] == 0xF3 && data[3..].iter().all(|&b| b == 0xFF)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PictureHeader {
    pub picture_type: u8,
    pub quantizer_shift: u8,
    pub factor: u8,
    pub quant_luma: [u8; 16],
    pub quant_chroma: [u8; 16],
}

impl PictureHeader {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < PICTURE_HEADER_BYTES || read_bits(data, 0, 19)? != 0x400 {
            return None;
        }
        Some(Self {
            picture_type: (data[2] >> 2) & 7,
            quantizer_shift: data[2] & 3,
            factor: data[3],
            quant_luma: data[4..20].try_into().ok()?,
            quant_chroma: data[20..36].try_into().ok()?,
        })
    }
}

pub fn read_bits(data: &[u8], offset: usize, count: usize) -> Option<u32> {
    if count > 32 || offset.checked_add(count)? > data.len().checked_mul(8)? {
        return None;
    }
    let mut value = 0;
    for pos in offset..offset + count {
        value = (value << 1) | u32::from((data[pos / 8] >> (7 - pos % 8)) & 1);
    }
    Some(value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PictureRows {
    /// Earliest ordered candidates. Entropy validation is still required.
    pub starts: [usize; PICTURE_ROWS],
    pub terminator_bit: usize,
    pub zero_padding_bits: usize,
    /// Rows for which another ordered marker sequence is possible.
    pub ambiguous_rows: usize,
}

/// Find 27 ordered row markers and a terminal marker anchored to padding.
/// Marker-shaped bits can occur inside coefficients; do not infer valid pixels.
pub fn scan_picture_rows(data: &[u8]) -> Option<PictureRows> {
    PictureHeader::parse(data)?;
    let used = data.iter().rposition(|&b| b != 0xFF)? + 1;
    let last_nonzero = data[..used].iter().rposition(|&b| b != 0)?;
    let end = last_nonzero * 8 + 8 - data[last_nonzero].trailing_zeros() as usize;
    let padding = used * 8 - end;
    let terminator = end.checked_sub(14)?;
    if padding > 15 || read_bits(data, terminator, 14)? != 0x21 {
        return None;
    }
    let first = PICTURE_HEADER_BYTES * 8;
    if read_bits(data, first, 19)? != (0x20 << 5) | 1 {
        return None;
    }
    let mut candidates: [Vec<usize>; PICTURE_ROWS] = std::array::from_fn(|_| Vec::new());
    candidates[0].push(first);
    let mut window = 0u32;
    for pos in first + 19..terminator {
        window = ((window << 1) | u32::from((data[pos / 8] >> (7 - pos % 8)) & 1)) & 0x7FFFF;
        if pos >= first + 37 && window >> 5 == 0x20 {
            let row = (window & 31) as usize;
            if (2..=PICTURE_ROWS).contains(&row) {
                candidates[row - 1].push(pos + 1 - 19);
            }
        }
    }
    let mut starts = [first; PICTURE_ROWS];
    for row in 1..PICTURE_ROWS {
        starts[row] = *candidates[row]
            .iter()
            .find(|&&p| p >= starts[row - 1] + 19)?;
    }
    let mut latest = terminator;
    let mut ambiguous_rows = 0;
    for row in (1..PICTURE_ROWS).rev() {
        latest = *candidates[row].iter().rev().find(|&&p| p + 19 <= latest)?;
        ambiguous_rows += usize::from(latest != starts[row]);
    }
    Some(PictureRows {
        starts,
        terminator_bit: terminator,
        zero_padding_bits: padding,
        ambiguous_rows,
    })
}
