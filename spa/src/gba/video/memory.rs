/// Video memory

use std::convert::TryInto;
use crate::common::videomem::VRAM2D;

const VRAM_SIZE: u32 = 96 * 1024;
const OBJ_VRAM_SIZE: u32 = 32 * 1024;

/// VRAM. Contains tile data, background maps, and bitmaps.
pub struct VRAM {
    data: Vec<u8>
}

// Memory interface
impl VRAM {
    pub fn new() -> Self {
        Self {
            data: vec![0; VRAM_SIZE as usize]
        }
    }

    /*pub fn read_halfword(&self, addr: u32) -> u16 {
        let start = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        } as usize;
        let end = start + 2;
        let data: [u8; 2] = (self.data[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        let start = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        } as usize;
        let end = start + 2;
        for (dest, byte) in self.data[start..end].iter_mut().zip(&data.to_le_bytes()) {
            *dest = *byte;
        }
    }*/
}

impl VRAM2D for VRAM {
    /// Read a byte from VRAM.
    fn get_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    //fn read_halfword(&self, addr: u32) -> u16 {
    //    let start = addr as usize;
    //    let end = start + 2;
    //    let data: [u8; 2] = (self.data[start..end]).try_into().unwrap();
    //    u16::from_le_bytes(data)
    //}

    fn get_halfword(&self, addr: u32) -> u16 {
        let start = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        } as usize;
        let end = start + 2;
        let data: [u8; 2] = (self.data[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        let start = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        } as usize;
        let end = start + 2;
        for (dest, byte) in self.data[start..end].iter_mut().zip(&data.to_le_bytes()) {
            *dest = *byte;
        }
    }
}
