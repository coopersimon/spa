
mod oam;
mod palette;
mod registers;
mod vram;

pub use oam::OAM;
pub use palette::PaletteRAM;
pub use registers::*;
pub use vram::VRAM2D;

// TODO: make generic for VRAM
pub struct VideoMemory {
    pub registers:  VideoRegisters,

    pub oam:        OAM,
    pub palette:    PaletteRAM,
    pub vram:       Box<dyn VRAM2D>,
}

impl VideoMemory {
    pub fn new(vram: Box<dyn VRAM2D>) -> Self {
        Self {
            registers:  VideoRegisters::new(),

            oam:        OAM::new(),
            palette:    PaletteRAM::new(),
            vram:       vram,
        }
    }
}
