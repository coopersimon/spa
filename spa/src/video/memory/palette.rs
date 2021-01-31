/// Palette memory

use crate::common::meminterface::MemInterface16;

pub struct PaletteRAM {

}

impl MemInterface16 for PaletteRAM {
    fn read_halfword(&self, addr: u32) -> u16 {
        0
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {

    }
}