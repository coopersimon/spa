/// Video memory

mod registers;
mod oam;
mod palette;
mod vram;

pub use registers::*;
pub use palette::PaletteRAM;
pub use oam::OAM;
pub use vram::VRAM;

pub struct VideoMemory {
    pub registers:  VideoRegisters,

    pub oam:        OAM,
    pub palette:    PaletteRAM,
    pub vram:       VRAM,
}

impl VideoMemory {
    pub fn new() -> Self {
        Self {
            registers:  VideoRegisters::new(),

            oam:        OAM::new(),
            palette:    PaletteRAM::new(),
            vram:       VRAM::new(),
        }
    }
}
