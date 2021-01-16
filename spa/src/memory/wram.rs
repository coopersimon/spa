/// Work RAM
use bytemuck::{
    try_from_bytes, try_from_bytes_mut
};

/// Work RAM.
/// Can read and write quantities of 8, 16, and 32 bits.
/// 
/// Note that 16 and 32-bit accesses must be aligned, or the program will panic.
pub struct WRAM(Vec<u8>);

impl WRAM {
    pub fn new(size: usize) -> Self {
        WRAM(vec![0; size])
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.0[addr as usize]
    }
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.0[addr as usize] = data;
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        let start = addr as usize;
        let end = (addr + 2) as usize;
        *try_from_bytes(&self.0[start..end]).expect(&format!("cannot read halfword at 0x{:X}", addr))
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        let start = addr as usize;
        let end = (addr + 2) as usize;
        let dest = try_from_bytes_mut(&mut self.0[start..end]).expect(&format!("cannot write halfword at 0x{:X}", addr));
        *dest = data;
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        let start = addr as usize;
        let end = (addr + 4) as usize;
        *try_from_bytes(&self.0[start..end]).expect(&format!("cannot read word at 0x{:X}", addr))
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        let start = addr as usize;
        let end = (addr + 4) as usize;
        let dest = try_from_bytes_mut(&mut self.0[start..end]).expect(&format!("cannot write word at 0x{:X}", addr));
        *dest = data;
    }
}