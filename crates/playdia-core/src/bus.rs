//! Address bus with proven Playdia / SH7032 regions.
//!
//! Regions marked `proven` come from hardware access captures.
//! Unmapped access is diagnosed and returns open-bus 0xFF / discarded writes.

use crate::diagnostics::Diagnostics;

/// Proven main-CPU map (from hardware access dump).
#[derive(Debug, Clone)]
pub struct Bus {
    /// 0x007FFC00-0x007FFFFF — SH1 internal RAM (1 KiB proven).
    pub ram: Vec<u8>,
    pub ram_base: u32,
    /// 0xE0000000-0xE007FFFF — main EPROM (512 KiB).
    pub bios: Vec<u8>,
    pub bios_base: u32,
    /// 0x000F0000-0x000F00FF — CDXA shared window (full page for simplicity).
    pub cdx_shared: [u8; 0x100],
    pub cdx_shared_base: u32,
    /// 0x00100000-0x0015FFFF — CDXA sector/stream DRAM (384 KiB).
    pub cdx_dram: Vec<u8>,
    pub cdx_dram_base: u32,
    /// 0x00180000-0x00197FFF — CDXA VRAM (96 KiB).
    pub cdx_vram: Vec<u8>,
    pub cdx_vram_base: u32,
    /// 0x00200000-0x0027FFFF — CDXA work DRAM (512 KiB).
    pub cdx_dram2: Vec<u8>,
    pub cdx_dram2_base: u32,
}

pub const RAM_BASE: u32 = 0x007F_FC00;
pub const RAM_SIZE: usize = 0x400;
pub const BIOS_BASE: u32 = 0xE000_0000;
pub const BIOS_SIZE: usize = 0x8_0000;
pub const CDX_SHARED_BASE: u32 = 0x000F_0000;
pub const CDX_DRAM_BASE: u32 = 0x0010_0000;
pub const CDX_DRAM_SIZE: usize = 0x6_0000;
pub const CDX_VRAM_BASE: u32 = 0x0018_0000;
pub const CDX_VRAM_SIZE: usize = 0x1_8000;
pub const CDX_DRAM2_BASE: u32 = 0x0020_0000;
pub const CDX_DRAM2_SIZE: usize = 0x8_0000;

impl Bus {
    pub fn new(bios: Vec<u8>) -> Self {
        Self {
            ram: vec![0; RAM_SIZE],
            ram_base: RAM_BASE,
            bios,
            bios_base: BIOS_BASE,
            cdx_shared: [0; 0x100],
            cdx_shared_base: CDX_SHARED_BASE,
            cdx_dram: vec![0; CDX_DRAM_SIZE],
            cdx_dram_base: CDX_DRAM_BASE,
            cdx_vram: vec![0; CDX_VRAM_SIZE],
            cdx_vram_base: CDX_VRAM_BASE,
            cdx_dram2: vec![0; CDX_DRAM2_SIZE],
            cdx_dram2_base: CDX_DRAM2_BASE,
        }
    }

    pub fn reset_ram(&mut self) {
        self.ram.fill(0);
    }

    fn map_slice(&mut self, addr: u32, len: usize) -> Option<(&mut [u8], usize)> {
        fn pick<'a>(
            base: u32,
            buf: &'a mut [u8],
            addr: u32,
            len: usize,
        ) -> Option<(&'a mut [u8], usize)> {
            if addr < base {
                return None;
            }
            let off = (addr - base) as usize;
            if off >= buf.len() {
                return None;
            }
            let n = len.min(buf.len() - off);
            Some((&mut buf[off..off + n], n))
        }
        if let Some(x) = pick(self.ram_base, &mut self.ram, addr, len) {
            return Some(x);
        }
        if let Some(x) = pick(self.bios_base, &mut self.bios, addr, len) {
            return Some(x);
        }
        if let Some(x) = pick(self.cdx_shared_base, &mut self.cdx_shared, addr, len) {
            return Some(x);
        }
        if let Some(x) = pick(self.cdx_dram_base, &mut self.cdx_dram, addr, len) {
            return Some(x);
        }
        if let Some(x) = pick(self.cdx_vram_base, &mut self.cdx_vram, addr, len) {
            return Some(x);
        }
        if let Some(x) = pick(self.cdx_dram2_base, &mut self.cdx_dram2, addr, len) {
            return Some(x);
        }
        None
    }

    pub fn read8(&mut self, addr: u32, diag: &mut Diagnostics) -> u8 {
        if let Some((s, _)) = self.map_slice(addr, 1) {
            return s[0];
        }
        diag.record_unmapped_read(addr);
        0xFF
    }

    pub fn write8(&mut self, addr: u32, val: u8, diag: &mut Diagnostics) {
        // BIOS is read-only.
        if addr >= self.bios_base && addr < self.bios_base + self.bios.len() as u32 {
            diag.note("bios_write_ignored");
            return;
        }
        if let Some((s, _)) = self.map_slice(addr, 1) {
            s[0] = val;
            return;
        }
        diag.record_unmapped_write(addr);
    }

    pub fn read16(&mut self, addr: u32, diag: &mut Diagnostics) -> u16 {
        let a = self.read8(addr, diag);
        let b = self.read8(addr.wrapping_add(1), diag);
        u16::from_le_bytes([a, b])
    }

    pub fn write16(&mut self, addr: u32, val: u16, diag: &mut Diagnostics) {
        let [a, b] = val.to_le_bytes();
        self.write8(addr, a, diag);
        self.write8(addr.wrapping_add(1), b, diag);
    }

    pub fn read32(&mut self, addr: u32, diag: &mut Diagnostics) -> u32 {
        let lo = self.read16(addr, diag) as u32;
        let hi = self.read16(addr.wrapping_add(2), diag) as u32;
        lo | (hi << 16)
    }

    pub fn write32(&mut self, addr: u32, val: u32, diag: &mut Diagnostics) {
        self.write16(addr, val as u16, diag);
        self.write16(addr.wrapping_add(2), (val >> 16) as u16, diag);
    }

    /// Fetch instruction halfword (PC region must be executable).
    pub fn fetch16(&mut self, addr: u32, diag: &mut Diagnostics) -> Option<u16> {
        if addr >= self.bios_base && addr < self.bios_base + self.bios.len() as u32 {
            let off = (addr - self.bios_base) as usize;
            if off + 1 < self.bios.len() {
                return Some(u16::from_le_bytes([self.bios[off], self.bios[off + 1]]));
            }
        }
        if addr >= self.ram_base && addr + 1 < self.ram_base + self.ram.len() as u32 {
            let off = (addr - self.ram_base) as usize;
            return Some(u16::from_le_bytes([self.ram[off], self.ram[off + 1]]));
        }
        diag.record_unmapped_read(addr);
        None
    }
}
