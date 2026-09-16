//! Bit reader for experimental AK8000 entropy decoding.

/// Read bits from each byte in the selected order.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    bits: usize,
    order: [u8; 256],
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], lsb_first: bool) -> Self {
        let mut order = [0u8; 256];
        for (i, byte) in order.iter_mut().enumerate() {
            *byte = if lsb_first {
                (i as u8).reverse_bits()
            } else {
                i as u8
            };
        }
        Self {
            data,
            pos: 0,
            bits: data.len() * 8,
            order,
        }
    }

    #[inline]
    fn byte_at(&self, i: usize) -> u8 {
        self.order[self.data[i] as usize]
    }

    pub fn get1(&mut self) -> u32 {
        if self.pos >= self.bits {
            return 0;
        }
        let p = self.pos;
        self.pos += 1;
        let b = self.byte_at(p >> 3);
        ((b >> (7 - (p & 7))) & 1) as u32
    }

    pub fn read(&mut self, n: u32) -> i32 {
        let mut v = 0i32;
        for _ in 0..n {
            v = (v << 1) | self.get1() as i32;
        }
        v
    }

    /// MPEG-1-like size VLC (fallback until custom table recovered).
    pub fn read_vlc(&mut self) -> Option<i32> {
        if self.pos >= self.bits {
            return None;
        }
        let size = if self.get1() == 0 {
            if self.get1() == 1 {
                2
            } else {
                1
            }
        } else if self.get1() == 0 {
            if self.get1() == 1 {
                3
            } else {
                0
            }
        } else if self.get1() == 0 {
            4
        } else if self.get1() == 0 {
            5
        } else if self.get1() == 0 {
            6
        } else if self.get1() == 1 {
            8
        } else {
            7
        };
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

    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn bits(&self) -> usize {
        self.bits
    }
}
