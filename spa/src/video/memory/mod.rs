/// Video memory

mod registers;
mod oam;
mod palette;

use crate::memory::WRAM;
use registers::VideoRegisters;

pub struct VideoMemory {
    pub registers: VideoRegisters,

    pub oam: oam::OAM,
    pub palette: palette::PaletteRAM,
    pub vram: WRAM,
}

impl VideoMemory {
    pub fn new() -> Self {
        Self {
            registers:  VideoRegisters::new(),

            oam:        oam::OAM::new(),
            palette:    palette::PaletteRAM::new(),
            vram:       WRAM::new(96 * 1024),
        }
    }
}
