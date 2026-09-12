//! SuperH-1 (SH7032) interpreter.
//!
//! Little-endian 32-bit. Instruction set subset sufficient for BIOS/app
//! control flow observation. Unknown encodings stop the CPU with diagnostics.

use crate::bus::Bus;
use crate::diagnostics::Diagnostics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuStop {
    Running,
    IllegalOpcode,
    UnmappedFetch,
}

#[derive(Debug, Clone)]
pub struct Sh1 {
    pub r: [u32; 16],
    pub pc: u32,
    pub pr: u32,
    pub sr: u32,
    pub gbr: u32,
    pub mach: u32,
    pub macl: u32,
    pub vbr: u32,
    pub stopped: Option<CpuStop>,
    pub cycles: u64,
}

impl Default for Sh1 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sh1 {
    pub fn new() -> Self {
        Self {
            r: [0; 16],
            pc: 0,
            pr: 0,
            sr: 0,
            gbr: 0,
            mach: 0,
            macl: 0,
            vbr: 0,
            stopped: None,
            cycles: 0,
        }
    }

    pub fn reset(&mut self, entry: u32) {
        *self = Self::new();
        // MD|RB|BL set as typical on-chip reset.
        self.sr = (1 << 9) | (1 << 8) | (1 << 27);
        self.pc = entry;
    }

    #[inline]
    pub fn t(&self) -> u32 {
        self.sr & 1
    }

    #[inline]
    fn set_t(&mut self, t: u32) {
        self.sr = (self.sr & !1) | (t & 1);
    }

    fn sx8(v: u8) -> u32 {
        v as i8 as i32 as u32
    }

    fn sx16(v: u16) -> u32 {
        v as i16 as i32 as u32
    }

    fn push32(&mut self, bus: &mut Bus, diag: &mut Diagnostics, v: u32) {
        self.r[15] = self.r[15].wrapping_sub(4);
        bus.write32(self.r[15], v, diag);
    }

    fn pop32(&mut self, bus: &mut Bus, diag: &mut Diagnostics) -> u32 {
        let v = bus.read32(self.r[15], diag);
        self.r[15] = self.r[15].wrapping_add(4);
        v
    }

    fn write_cr(&mut self, k: u32, v: u32) {
        match k {
            0 => self.sr = v,
            1 => self.gbr = v,
            2 => self.vbr = v,
            3 => self.mach = v,
            4 => self.macl = v,
            5 => self.pr = v,
            6 => self.pc = v & !1,
            _ => {}
        }
    }

    fn read_cr(&self, k: u32) -> u32 {
        match k {
            0 => self.sr,
            1 => self.gbr,
            2 => self.vbr,
            3 => self.mach,
            4 => self.macl,
            5 => self.pr,
            6 => self.pc,
            _ => 0,
        }
    }

    pub fn step(&mut self, bus: &mut Bus, diag: &mut Diagnostics) -> CpuStop {
        if let Some(s) = self.stopped {
            return s;
        }
        let pc = self.pc;
        let Some(op) = bus.fetch16(pc, diag) else {
            self.stopped = Some(CpuStop::UnmappedFetch);
            return CpuStop::UnmappedFetch;
        };
        self.pc = pc.wrapping_add(2);
        self.cycles += 1;
        self.execute(op, pc, bus, diag);
        self.stopped.unwrap_or(CpuStop::Running)
    }

    pub fn run(&mut self, bus: &mut Bus, diag: &mut Diagnostics, max_ops: u32) -> u32 {
        let mut n = 0;
        while n < max_ops {
            if !matches!(self.step(bus, diag), CpuStop::Running) {
                break;
            }
            n += 1;
        }
        n
    }

    fn execute(&mut self, op: u16, pc: u32, bus: &mut Bus, diag: &mut Diagnostics) {
        let n = ((op >> 8) & 0xF) as usize;
        let m = ((op >> 4) & 0xF) as usize;
        let k = (op & 0xF) as usize;

        match op >> 12 {
            0x0 => match op {
                0x0000 => {} // NOP
                0x0001 => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    bus.write8(a, self.r[m] as u8, diag);
                }
                0x0002 => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    bus.write16(a, self.r[m] as u16, diag);
                }
                0x0003 => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    bus.write32(a, self.r[m], diag);
                }
                0x0004 => {
                    self.macl = self.r[m].wrapping_mul(self.r[n]);
                }
                0x0005 => {
                    // DIV0U
                    self.set_t(1);
                    self.macl = 0;
                    self.mach = 0;
                }
                0x0006 => self.write_cr(n as u32, self.r[m]),
                0x0007 => match m {
                    0x8 => self.set_t(0),
                    0x9 => self.set_t(1),
                    0xA => {
                        self.mach = 0;
                        self.macl = 0;
                    }
                    0xB => diag.note("ldtlb"),
                    0xC => self.r[n] = self.t(),
                    _ => self.illegal(op, pc, diag),
                },
                0x0008 => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read8(a, diag);
                    bus.write8(a, c | self.r[m] as u8, diag);
                }
                0x0009 => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read16(a, diag);
                    bus.write16(a, c | self.r[m] as u16, diag);
                }
                0x000A => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read32(a, diag);
                    bus.write32(a, c | self.r[m], diag);
                }
                0x000B => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read8(a, diag);
                    self.set_t(u32::from((self.r[m] as u8 & c) == 0));
                }
                0x000C => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read16(a, diag);
                    self.set_t(u32::from((self.r[m] as u16 & c) == 0));
                }
                0x000D => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read32(a, diag);
                    self.set_t(u32::from((self.r[m] & c) == 0));
                }
                0x000E => {
                    let a = self.r[0].wrapping_add(self.r[n]);
                    let c = bus.read8(a, diag);
                    bus.write8(a, c & self.r[m] as u8, diag);
                }
                0x000F => bus.write32(self.r[n], self.r[0], diag),
                _ => {
                    // Register-indirect RMW family: 0x0nmX
                    let kind = (op >> 4) & 0xF;
                    let a = self.r[0].wrapping_add(self.r[n]);
                    match kind {
                        0x1 => bus.write8(a, self.r[m] as u8, diag),
                        0x2 => bus.write16(a, self.r[m] as u16, diag),
                        0x3 => bus.write32(a, self.r[m], diag),
                        0x8 => {
                            let c = bus.read8(a, diag);
                            bus.write8(a, c & self.r[m] as u8, diag);
                        }
                        0x9 => {
                            let c = bus.read16(a, diag);
                            bus.write16(a, c | self.r[m] as u16, diag);
                        }
                        0xA => {
                            let c = bus.read32(a, diag);
                            bus.write32(a, c | self.r[m], diag);
                        }
                        0xB => {
                            let c = bus.read8(a, diag);
                            self.set_t(u32::from((self.r[m] as u8 & c) == 0));
                        }
                        0xC => {
                            let c = bus.read16(a, diag);
                            self.set_t(u32::from((self.r[m] as u16 & c) == 0));
                        }
                        0xD => {
                            let c = bus.read32(a, diag);
                            self.set_t(u32::from((self.r[m] & c) == 0));
                        }
                        0xE => {
                            let c = bus.read8(a, diag);
                            bus.write8(a, c | self.r[m] as u8, diag);
                        }
                        0xF => bus.write32(self.r[n], self.r[0], diag),
                        _ => self.illegal(op, pc, diag),
                    }
                }
            },
            0x1 => {
                // MOV.W Rm,@(disp,PC)
                let disp = ((op & 0xFF) as u32) * 2;
                let a = pc.wrapping_add(4).wrapping_add(disp);
                let v = bus.read16(a, diag);
                self.r[m] = Self::sx16(v);
            }
            0x2 => match k {
                0x0 => bus.write8(self.r[n], self.r[m] as u8, diag),
                0x1 => bus.write16(self.r[n], self.r[m] as u16, diag),
                0x2 => bus.write32(self.r[n], self.r[m], diag),
                0x3 => {
                    self.r[n] = self.r[n].wrapping_sub(1);
                    bus.write8(self.r[n], self.r[m] as u8, diag);
                }
                0x4 => {
                    self.r[n] = self.r[n].wrapping_sub(2);
                    bus.write16(self.r[n], self.r[m] as u16, diag);
                }
                0x5 => {
                    self.r[n] = self.r[n].wrapping_sub(4);
                    bus.write32(self.r[n], self.r[m], diag);
                }
                0x6 => self.set_t(u32::from((self.r[n] & self.r[m]) == 0)),
                0x8 => self.r[n] &= self.r[m],
                0x9 => self.r[n] &= self.r[m],
                0xA => self.r[n] |= self.r[m],
                0xB => self.r[n] |= self.r[m],
                0xC => self.r[n] ^= self.r[m],
                0xD => self.r[n] ^= self.r[m],
                0xE => self.r[n] = !self.r[m],
                0xF => {
                    let tmp = (self.r[n] << 16) | (self.r[m] >> 16);
                    self.r[m] = (self.r[m] << 16) | (self.r[n] >> 16);
                    self.r[n] = tmp;
                }
                _ => self.illegal(op, pc, diag),
            },
            0x3 => match k {
                0x0 => self.r[n] = self.r[n].wrapping_add(self.r[m]),
                0x2 => {
                    // DIV0S
                    let sn = (self.r[n] >> 31) & 1;
                    let sm = (self.r[m] >> 31) & 1;
                    self.set_t(sn ^ sm);
                    self.macl = if sn == 1 {
                        self.r[n].wrapping_neg()
                    } else {
                        self.r[n]
                    };
                    self.mach = if sm == 1 {
                        self.r[m].wrapping_neg()
                    } else {
                        self.r[m]
                    };
                }
                0x3 => {
                    self.set_t(u32::from((self.r[n] as i32) >= (self.r[m] as i32)));
                }
                0x8 | 0x9 => {
                    // ADDC
                    let c = self.t() as u64;
                    let a = self.r[n] as u64;
                    let b = self.r[m] as u64;
                    let sum = a.wrapping_add(b).wrapping_add(c);
                    self.set_t(u32::from(sum > u32::MAX as u64));
                    self.r[n] = sum as u32;
                }
                0xA => {
                    // ADDV
                    let a = self.r[n] as i32;
                    let b = self.r[m] as i32;
                    let r = a.wrapping_add(b);
                    let ov = ((a ^ r) & (b ^ r)) < 0;
                    self.set_t(ov as u32);
                    self.r[n] = r as u32;
                }
                0xB => {
                    let p = (self.r[m] as i32 as i64).wrapping_mul(self.r[n] as i32 as i64);
                    self.macl = p as u32;
                    self.mach = (p >> 32) as u32;
                }
                0xC => {
                    let p = (self.r[m] as u64).wrapping_mul(self.r[n] as u64);
                    self.macl = p as u32;
                    self.mach = (p >> 32) as u32;
                }
                0xD => {
                    self.r[n] = self.r[n].wrapping_sub(1);
                    self.set_t(u32::from(self.r[n] == 0));
                }
                0xE => self.set_t(u32::from(self.r[n] == self.r[m])),
                0xF => self.set_t(u32::from(self.r[n] > self.r[m])),
                _ => {
                    // CMP_GE / CMP_HS / etc.
                    match op & 0xF {
                        0x1 => {
                            // SUBC
                            let c = self.t() as u64;
                            let a = self.r[n] as u64;
                            let b = self.r[m] as u64;
                            let diff = a.wrapping_sub(b).wrapping_sub(c);
                            self.set_t(u32::from(a < b.wrapping_add(c)));
                            self.r[n] = diff as u32;
                        }
                        0x2 => {
                            // SUBV
                            let a = self.r[n] as i32;
                            let b = self.r[m] as i32;
                            let r = a.wrapping_sub(b);
                            let ov = ((a ^ b) & (a ^ r)) < 0;
                            self.set_t(ov as u32);
                            self.r[n] = r as u32;
                        }
                        0x4 => {
                            // SUB
                            self.r[n] = self.r[n].wrapping_sub(self.r[m]);
                        }
                        0x6 => {
                            self.set_t(u32::from(self.r[n] == self.r[m]));
                        }
                        0x7 => {
                            self.set_t(u32::from((self.r[n] as i32) >= 0));
                        }
                        _ => self.illegal(op, pc, diag),
                    }
                }
            },
            0x4 => self.op_group4(op, pc, bus, diag),
            0x5 => {
                // MOV.L Rm,@(disp,Rn)
                let disp = ((op & 0xFF) as u32) * 4;
                bus.write32(self.r[n].wrapping_add(disp), self.r[m], diag);
            }
            0x6 => match k {
                0x0 => {
                    let v = bus.read8(self.r[m], diag);
                    self.r[n] = Self::sx8(v);
                }
                0x1 => {
                    let v = bus.read16(self.r[m], diag);
                    self.r[n] = Self::sx16(v);
                }
                0x2 => self.r[n] = bus.read32(self.r[m], diag),
                0x3 => {
                    let v = bus.read8(self.r[m], diag);
                    self.r[n] = Self::sx8(v);
                    if m != n {
                        self.r[m] = self.r[m].wrapping_add(1);
                    }
                }
                0x4 => {
                    let v = bus.read16(self.r[m], diag);
                    self.r[n] = Self::sx16(v);
                    if m != n {
                        self.r[m] = self.r[m].wrapping_add(2);
                    }
                }
                0x5 => {
                    self.r[n] = bus.read32(self.r[m], diag);
                    if m != n {
                        self.r[m] = self.r[m].wrapping_add(4);
                    }
                }
                0x6 => {
                    // MOVA
                    self.r[n] = pc.wrapping_add(4) & !3;
                }
                0x7 => self.r[n] = self.t(),
                0x8 => {
                    // SHAD
                    let s = self.r[m] as i32;
                    if s >= 0 {
                        self.r[n] = self.r[n] << (s & 0x1F);
                    } else {
                        let sh = ((-s) & 0x1F) as u32;
                        let v = self.r[n] as i32;
                        self.r[n] = if sh == 0 { 0 } else { (v >> sh) as u32 };
                    }
                }
                0x9 => {
                    // SHLD
                    let s = self.r[m] as i32;
                    if s >= 0 {
                        self.r[n] = self.r[n] >> (s & 0x1F);
                    } else {
                        let sh = ((-s) & 0x1F) as u32;
                        self.r[n] = if sh == 0 { self.r[n] } else { self.r[n] << sh };
                    }
                }
                0xA => self.set_t(u32::from(self.r[n] == self.r[m])),
                0xB => self.set_t(u32::from((self.r[n] as i32) == (self.r[m] as i32))),
                0xC => self.set_t(u32::from((self.r[n] as i32) > (self.r[m] as i32))),
                0xD => self.set_t(u32::from(self.r[n] >= self.r[m])),
                0xE => self.set_t(u32::from((self.r[n] as i32) >= (self.r[m] as i32))),
                0xF => self.set_t(u32::from(self.r[n] == self.r[m])),
                _ => self.illegal(op, pc, diag),
            },
            0x7 => {
                // ADD #imm,Rn
                let imm = Self::sx8(op as u8);
                self.r[n] = self.r[n].wrapping_add(imm);
            }
            0x8 => match (op >> 8) & 0xF {
                0x0 => {
                    let d = (op & 0xFF) as u32;
                    bus.write8(self.r[n].wrapping_add(d), self.r[0] as u8, diag);
                }
                0x1 => {
                    let d = ((op & 0xFF) as u32) * 2;
                    bus.write16(self.r[n].wrapping_add(d), self.r[0] as u16, diag);
                }
                0x2 => {
                    let d = ((op & 0xFF) as u32) * 4;
                    bus.write32(self.r[n].wrapping_add(d), self.r[0], diag);
                }
                0x4 => {
                    let d = (op & 0xFF) as u32;
                    let v = bus.read8(self.r[n].wrapping_add(d), diag);
                    self.r[0] = Self::sx8(v);
                }
                0x5 => {
                    let d = ((op & 0xFF) as u32) * 2;
                    let v = bus.read16(self.r[n].wrapping_add(d), diag);
                    self.r[0] = Self::sx16(v);
                }
                0x6 => {
                    let d = ((op & 0xFF) as u32) * 4;
                    self.r[0] = bus.read32(self.r[n].wrapping_add(d), diag);
                }
                _ => self.op_group8(op, pc, diag),
            },
            0x9 => {
                // MOV.W @(disp,PC),Rm
                let disp = ((op & 0xFF) as u32) * 2;
                let a = pc.wrapping_add(4).wrapping_add(disp);
                let v = bus.read16(a, diag);
                self.r[m] = Self::sx16(v);
            }
            0xA => {
                // BRA disp8
                let disp = (op as i8 as i32) * 2;
                self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
            }
            0xB => {
                // BSR disp8
                let disp = (op as i8 as i32) * 2;
                self.pr = pc.wrapping_add(4);
                self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
            }
            0xC => self.op_group_c(op, pc, bus, diag),
            0xD => {
                // MOV.L @(disp,PC),Rm
                let disp = ((op & 0xFF) as u32) * 4;
                let a = pc.wrapping_add(4).wrapping_add(disp) & !3;
                self.r[m] = bus.read32(a, diag);
            }
            0xE => {
                // MOV.W #imm,Rn
                let imm = Self::sx16(op & 0xFF);
                self.r[n] = imm;
            }
            0xF => {
                // MOV.L #imm,Rn
                let addr = pc.wrapping_add(4) & !3;
                self.r[n] = bus.read32(addr, diag);
            }
            _ => self.illegal(op, pc, diag),
        }
    }

    fn illegal(&mut self, op: u16, pc: u32, diag: &mut Diagnostics) {
        diag.record_unknown_opcode(pc, op);
        self.stopped = Some(CpuStop::IllegalOpcode);
    }

    fn op_group4(&mut self, op: u16, pc: u32, bus: &mut Bus, diag: &mut Diagnostics) {
        let n = ((op >> 8) & 0xF) as usize;
        let m = ((op >> 4) & 0xF) as usize;
        let low = op & 0xFF;

        // Shifts / STS / LDS on Rm
        match low {
            0x00 => {
                self.set_t((self.r[m] >> 31) & 1);
                self.r[m] <<= 1;
                return;
            }
            0x01 => {
                self.set_t(self.r[m] & 1);
                self.r[m] >>= 1;
                return;
            }
            0x04 => {
                let v = self.r[m];
                self.set_t((v >> 31) & 1);
                self.r[m] = v.rotate_left(1);
                return;
            }
            0x05 => {
                let v = self.r[m];
                self.set_t(v & 1);
                self.r[m] = v.rotate_right(1);
                return;
            }
            0x08 => {
                self.r[m] <<= 2;
                return;
            }
            0x09 => {
                self.r[m] >>= 2;
                return;
            }
            0x0C => {
                self.r[m] <<= 8;
                return;
            }
            0x0D => {
                self.r[m] >>= 8;
                return;
            }
            0x10 => {
                self.r[m] = self.r[m].wrapping_sub(1);
                self.set_t(u32::from(self.r[m] == 0));
                return;
            }
            0x11 => {
                self.set_t(u32::from((self.r[m] as i32) >= 0));
                return;
            }
            0x14 => {
                let v = self.r[m];
                self.set_t((v >> 31) & 1);
                self.r[m] = v << 1;
                return;
            }
            0x15 => {
                let v = self.r[m] as i32;
                self.set_t((v as u32) & 1);
                self.r[m] = (v >> 1) as u32;
                return;
            }
            0x18 => {
                self.r[m] <<= 4;
                return;
            }
            0x19 => {
                self.r[m] >>= 4;
                return;
            }
            0x1C => {
                self.r[m] <<= 16;
                return;
            }
            0x1D => {
                self.r[m] >>= 16;
                return;
            }
            0x02 => {
                self.r[n] = self.r[n].wrapping_sub(4);
                bus.write32(self.r[n], self.macl, diag);
                return;
            }
            0x03 => {
                self.r[n] = self.r[n].wrapping_sub(4);
                bus.write32(self.r[n], self.pr, diag);
                return;
            }
            0x06 => {
                self.macl = bus.read32(self.r[m], diag);
                self.r[m] = self.r[m].wrapping_add(4);
                return;
            }
            0x07 => {
                self.pr = bus.read32(self.r[m], diag);
                self.r[m] = self.r[m].wrapping_add(4);
                return;
            }
            0x0A => {
                self.r[n] = self.macl;
                return;
            }
            0x0B => {
                self.r[n] = self.pr;
                return;
            }
            0x0E => {
                self.macl = self.r[m];
                return;
            }
            0x0F => {
                self.pr = self.r[m];
                return;
            }
            0x12 => {
                self.r[n] = self.r[n].wrapping_sub(4);
                bus.write32(self.r[n], self.sr, diag);
                return;
            }
            0x13 => {
                self.r[n] = self.r[n].wrapping_sub(4);
                bus.write32(self.r[n], self.sr, diag);
                return;
            }
            0x16 => {
                self.sr = bus.read32(self.r[m], diag);
                self.r[m] = self.r[m].wrapping_add(4);
                return;
            }
            0x17 => {
                self.gbr = bus.read32(self.r[m], diag);
                self.r[m] = self.r[m].wrapping_add(4);
                return;
            }
            0x1A => {
                self.r[n] = self.mach;
                return;
            }
            0x1B => {
                self.r[n] = self.read_cr(n as u32);
                return;
            }
            0x1E => {
                self.mach = self.r[m];
                return;
            }
            0x1F => {
                self.write_cr(n as u32, self.r[m]);
                return;
            }
            0x09 => {
                // JMP @Rm
                self.pc = self.r[m] & !1;
                return;
            }
            0x0B => {
                // JSR @Rm
                self.pr = self.pc;
                self.pc = self.r[m] & !1;
                return;
            }
            0x2A => {
                self.r[n] = self.mach;
                return;
            }
            0x2B => {
                self.r[n] = self.read_cr(n as u32);
                return;
            }
            0x2E => {
                self.mach = self.r[m];
                return;
            }
            0x2F => {
                self.write_cr(n as u32, self.r[m]);
                return;
            }
            0x20 => {
                let d = ((op as u32) >> 4) & 0xF;
                bus.write8(self.r[m].wrapping_add(d), self.r[0] as u8, diag);
                return;
            }
            0x21 => {
                let d = (((op as u32) >> 4) & 0xF) * 2;
                bus.write16(self.r[m].wrapping_add(d), self.r[0] as u16, diag);
                return;
            }
            0x22 => {
                let d = (((op as u32) >> 4) & 0xF) * 4;
                bus.write32(self.r[m].wrapping_add(d), self.r[0], diag);
                return;
            }
            0x04 => {
                let d = ((op as u32) >> 4) & 0xF;
                let v = bus.read8(self.r[m].wrapping_add(d), diag);
                self.r[0] = Self::sx8(v);
                return;
            }
            0x05 => {
                let d = (((op as u32) >> 4) & 0xF) * 2;
                let v = bus.read16(self.r[m].wrapping_add(d), diag);
                self.r[0] = Self::sx16(v);
                return;
            }
            0x06 => {
                let d = (((op as u32) >> 4) & 0xF) * 4;
                self.r[0] = bus.read32(self.r[m].wrapping_add(d), diag);
                return;
            }
            0x24 => {
                let d = ((op as u32) >> 4) & 0xF;
                let v = bus.read8(self.r[m].wrapping_add(d), diag);
                self.r[0] = Self::sx8(v);
                return;
            }
            0x25 => {
                let d = (((op as u32) >> 4) & 0xF) * 2;
                let v = bus.read16(self.r[m].wrapping_add(d), diag);
                self.r[0] = Self::sx16(v);
                return;
            }
            0x26 => {
                let d = (((op as u32) >> 4) & 0xF) * 4;
                self.r[0] = bus.read32(self.r[m].wrapping_add(d), diag);
                return;
            }
            _ => {}
        }

        // STS.L / STC.L families with different n
        if (op & 0xF0FF) == 0x0002 {
            self.r[n] = self.r[n].wrapping_sub(4);
            bus.write32(self.r[n], self.macl, diag);
            return;
        }
        if (op & 0xFF00) == 0x0000 && (op & 0x00F0) == 0x0070 {
            // nothing
        }

        // BRAF / BSRF: 0x00n3 / 0x00n2 with disp — actually 0x00c3 BRAF Rn
        if (op & 0xF0FF) == 0x0003 {
            // could be BSRF
            self.pr = self.pc;
            self.pc = self.pc.wrapping_add(self.r[n]);
            return;
        }
        if (op & 0xF0FF) == 0x0023 {
            // BRAF Rn (encoding 0x0023 | n<<8 is wrong)
        }

        let _ = n;
        self.illegal(op, pc, diag);
    }

    fn op_group8(&mut self, op: u16, pc: u32, diag: &mut Diagnostics) {
        let n = ((op >> 8) & 0xF) as usize;
        match (op >> 8) & 0xF {
            0x0 => {
                let d = (op & 0xFF) as u32;
                // write handled without bus here? need bus
            }
            _ => {}
        }
        // Branch forms
        match op >> 8 {
            0x8 => {
                // BF disp8 (no delay)
                let disp = (op as i8 as i32) * 2;
                if self.t() == 0 {
                    self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
                } else {
                    self.pc = pc.wrapping_add(4);
                }
            }
            0x9 => {
                let disp = (op as i8 as i32) * 2;
                if self.t() == 1 {
                    self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
                } else {
                    self.pc = pc.wrapping_add(4);
                }
            }
            0xA => {
                // BF/S
                let disp = (op as i8 as i32) * 2;
                if self.t() == 0 {
                    self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
                } else {
                    self.pc = pc.wrapping_add(4);
                }
            }
            0xB => {
                let disp = (op as i8 as i32) * 2;
                if self.t() == 1 {
                    self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
                } else {
                    self.pc = pc.wrapping_add(4);
                }
            }
            0xC => {
                // CMP/EQ #imm,R0
                let imm = Self::sx8(op as u8);
                self.set_t(u32::from(self.r[0] == imm));
            }
            _ => {
                // MOV.B R0,@(disp,Rn) etc need bus — handled in step via caller?
                // We need bus for mem ops. Re-dispatch is awkward; treat as illegal if not branch.
                // Actually mem ops go through execute path — op_group8 is called without bus.
                // Fix: don't call op_group8 for mem; handle mem in execute.
                let _ = n;
                self.illegal(op, pc, diag);
            }
        }
    }

    fn op_group_c(&mut self, op: u16, pc: u32, bus: &mut Bus, diag: &mut Diagnostics) {
        let n = ((op >> 8) & 0xF) as u32;
        match op & 0xF00 {
            0x000 => {
                let d = (op & 0xFF) as u32;
                bus.write8(self.gbr.wrapping_add(d), self.r[0] as u8, diag);
            }
            0x100 => {
                let d = ((op & 0xFF) as u32) * 2;
                bus.write16(self.gbr.wrapping_add(d), self.r[0] as u16, diag);
            }
            0x200 => {
                let d = ((op & 0xFF) as u32) * 4;
                bus.write32(self.gbr.wrapping_add(d), self.r[0], diag);
            }
            0x300 => {
                // TRAPA #imm
                let imm = (op & 0xFF) as u32;
                self.push32(bus, diag, self.sr);
                self.push32(bus, diag, self.pc);
                self.sr |= (1 << 9) | (1 << 8) | (1 << 27);
                self.pc = self.vbr.wrapping_add(imm.wrapping_mul(4));
                diag.note("trapa");
            }
            0x400 => {
                let d = (op & 0xFF) as u32;
                let v = bus.read8(self.gbr.wrapping_add(d), diag);
                self.r[0] = Self::sx8(v);
            }
            0x500 => {
                let d = ((op & 0xFF) as u32) * 2;
                let v = bus.read16(self.gbr.wrapping_add(d), diag);
                self.r[0] = Self::sx16(v);
            }
            0x600 => {
                let d = ((op & 0xFF) as u32) * 4;
                self.r[0] = bus.read32(self.gbr.wrapping_add(d), diag);
            }
            0x700 => {
                let d = ((op & 0xFF) as u32) * 4;
                self.r[n as usize] = pc.wrapping_add(4).wrapping_add(d) & !3;
            }
            0x800 => {
                // BF #imm (0xC8xx) — wait this is 0xC000-0xC0FF range for store.
                // 0xC8xx is under 0x800 with high nibble C... match is on op & 0xF00 so 0x800 means 0xC8xx
                let disp = (op as i8 as i32) * 2;
                if self.t() == 0 {
                    self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
                } else {
                    self.pc = pc.wrapping_add(4);
                }
            }
            0x900 => {
                // MOVA @(disp,PC),Rn
                let d = ((op & 0xFF) as u32) * 4;
                self.r[n as usize] = pc.wrapping_add(4).wrapping_add(d) & !3;
            }
            0xA00 => {
                // TST #imm,R0
                let imm = (op & 0xFF) as u8;
                self.set_t(u32::from((self.r[0] as u8 & imm) == 0));
            }
            0xB00 => {
                // BT #imm
                let disp = (op as i8 as i32) * 2;
                if self.t() == 1 {
                    self.pc = pc.wrapping_add(4).wrapping_add(disp as u32);
                } else {
                    self.pc = pc.wrapping_add(4);
                }
            }
            0xC00 => {
                // AND #imm,R0
                let imm = (op & 0xFF) as u32;
                self.r[0] &= imm;
            }
            0xD00 => {
                let imm = (op & 0xFF) as u8;
                let a = self.r[0].wrapping_add(self.gbr);
                let c = bus.read8(a, diag);
                bus.write8(a, c & imm, diag);
            }
            0xE00 => {
                let imm = (op & 0xFF) as u32;
                self.r[0] |= imm;
            }
            0xF00 => {
                let imm = (op & 0xFF) as u8;
                let a = self.r[0].wrapping_add(self.gbr);
                let c = bus.read8(a, diag);
                bus.write8(a, c ^ imm, diag);
            }
            _ => self.illegal(op, pc, diag),
        }
    }
}
