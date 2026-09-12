//! CDXA device model: shared register window + memory-backed buffers.
//!
//! Register semantics are partially inferred from hardware access dumps.
//! Unknown accesses are counted, not treated as success.

use crate::diagnostics::Diagnostics;

/// Proven shared window is 0x000F0000-0x000F002F; we keep a full page.
#[derive(Debug, Clone)]
pub struct CdxDevice {
    /// Shared MMIO page.
    pub shared: [u8; 0x100],
    pub cmd: u16,
    pub status: u16,
    pub irq_status: u16,
    pub write_ptr: u32,
    pub read_ptr: u32,
    pub dma_src: u32,
    pub dma_dst: u32,
    pub dma_len: u32,
    pub stream_ctrl: u16,
    pub video_pending: bool,
    pub audio_pending: bool,
    pub unknown_reg_writes: u64,
    pub unknown_reg_reads: u64,
}

impl Default for CdxDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl CdxDevice {
    pub fn new() -> Self {
        Self {
            shared: [0; 0x100],
            cmd: 0,
            status: 0,
            irq_status: 0,
            write_ptr: 0,
            read_ptr: 0,
            dma_src: 0,
            dma_dst: 0,
            dma_len: 0,
            stream_ctrl: 0,
            video_pending: false,
            audio_pending: false,
            unknown_reg_writes: 0,
            unknown_reg_reads: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
        // Hardware-ready status bit0
        self.status = 1;
    }

    pub fn read8(&mut self, offset: u32, diag: &mut Diagnostics) -> u8 {
        if (offset as usize) < self.shared.len() {
            return self.shared[offset as usize];
        }
        self.unknown_reg_reads += 1;
        diag.record_unmapped_read(0x000F_0000 + offset);
        0xFF
    }

    pub fn write8(&mut self, offset: u32, val: u8, diag: &mut Diagnostics) {
        if (offset as usize) < self.shared.len() {
            self.shared[offset as usize] = val;
            return;
        }
        self.unknown_reg_writes += 1;
        diag.record_unmapped_write(0x000F_0000 + offset);
    }

    pub fn read16(&mut self, offset: u32, diag: &mut Diagnostics) -> u16 {
        let lo = self.read8(offset, diag) as u16;
        let hi = self.read8(offset + 1, diag) as u16;
        lo | (hi << 8)
    }

    pub fn write16(&mut self, offset: u32, val: u16, diag: &mut Diagnostics) {
        let [lo, hi] = val.to_le_bytes();
        // Known command/status regs modeled by 16-bit lane when offset aligned.
        match offset {
            0x00 => self.cmd = val,
            0x02 => {
                self.status = val;
            }
            0x04 => self.irq_status = val,
            0x06 => self.stream_ctrl = val,
            0x08 => self.write_ptr = (self.write_ptr & 0xFFFF_0000) | val as u32,
            0x0A => self.write_ptr = (self.write_ptr & 0x0000_FFFF) | ((val as u32) << 16),
            0x0C => self.read_ptr = (self.read_ptr & 0xFFFF_0000) | val as u32,
            0x0E => self.read_ptr = (self.read_ptr & 0x0000_FFFF) | ((val as u32) << 16),
            0x10 => self.dma_src = (self.dma_src & 0xFFFF_0000) | val as u32,
            0x12 => self.dma_src = (self.dma_src & 0x0000_FFFF) | ((val as u32) << 16),
            0x14 => self.dma_dst = (self.dma_dst & 0xFFFF_0000) | val as u32,
            0x16 => self.dma_dst = (self.dma_dst & 0x0000_FFFF) | ((val as u32) << 16),
            0x18 => self.dma_len = (self.dma_len & 0xFFFF_0000) | val as u32,
            0x1A => self.dma_len = (self.dma_len & 0x0000_FFFF) | ((val as u32) << 16),
            _ => {
                if (offset as usize) + 1 < self.shared.len() {
                    self.shared[offset as usize] = lo;
                    self.shared[offset as usize + 1] = hi;
                } else {
                    self.unknown_reg_writes += 1;
                    diag.record_unmapped_write(0x000F_0000 + offset);
                }
            }
        }
        let _ = (lo, hi);
    }

    pub fn mark_video_frame(&mut self) {
        self.video_pending = true;
    }

    pub fn mark_audio_block(&mut self) {
        self.audio_pending = true;
    }

    pub fn take_video(&mut self) -> bool {
        let v = self.video_pending;
        self.video_pending = false;
        v
    }
}
