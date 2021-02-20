/// Work RAM
use std::convert::TryInto;

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
        let end = start + 2;
        let data = (self.0[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        let start = addr as usize;
        let end = start + 2;
        for (dest, byte) in self.0[start..end].iter_mut().zip(&data.to_le_bytes()) {
            *dest = *byte;
        }
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        let start = addr as usize;
        let end = start + 4;
        let data = (self.0[start..end]).try_into().unwrap();
        u32::from_le_bytes(data)
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        let start = addr as usize;
        let end = start + 4;
        for (dest, byte) in self.0[start..end].iter_mut().zip(&data.to_le_bytes()) {
            *dest = *byte;
        }
    }
}