//! Core diagnostics. Default is quiet; counters stay cheap.

use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct Diagnostics {
    pub unmapped_reads: u64,
    pub unmapped_writes: u64,
    pub unknown_opcodes: u64,
    pub illegal_instructions: u64,
    pub hle_early_return: u64,
    pub frame_budget_exhausted: u64,
    pub last_unmapped_read: Option<u32>,
    pub last_unmapped_write: Option<u32>,
    pub last_unknown_pc: Option<u32>,
    pub notes: BTreeMap<&'static str, u64>,
}

impl Diagnostics {
    pub fn note(&mut self, key: &'static str) {
        *self.notes.entry(key).or_insert(0) += 1;
    }

    pub fn record_unmapped_read(&mut self, addr: u32) {
        self.unmapped_reads += 1;
        self.last_unmapped_read = Some(addr);
    }

    pub fn record_unmapped_write(&mut self, addr: u32) {
        self.unmapped_writes += 1;
        self.last_unmapped_write = Some(addr);
    }

    pub fn record_unknown_opcode(&mut self, pc: u32, opcode: u16) {
        self.unknown_opcodes += 1;
        self.last_unknown_pc = Some(pc);
        let _ = opcode;
    }

    pub fn is_quiet(&self) -> bool {
        self.unmapped_reads == 0
            && self.unmapped_writes == 0
            && self.unknown_opcodes == 0
            && self.illegal_instructions == 0
            && self.hle_early_return == 0
            && self.frame_budget_exhausted == 0
    }
}
