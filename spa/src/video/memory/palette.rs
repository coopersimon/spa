/// Palette memory

use crate::common::meminterface::MemInterface16;

pub struct PaletteRAM {
    palette_ram:     Vec<u16>,
}

impl PaletteRAM {
    pub fn new() -> Self {
        Self {
            palette_ram:    vec![0, 512],
        }
    }
}

impl MemInterface16 for PaletteRAM {
    fn read_halfword(&self, addr: u32) -> u16 {
        self.palette_ram[addr as usize]
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.palette_ram[addr as usize] = data;
    }
}
