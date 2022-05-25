
mod oam;
mod palette;
mod registers;
mod vram;
mod dispcap;

pub use oam::OAM;
pub use palette::PaletteRAM;
pub use registers::*;
pub use vram::{VRAM2D, LCDCMem};
pub use dispcap::{DispCapMode, DispCapSourceA, DispCapSourceB};

pub struct VideoMemory<V: VRAM2D> {
    pub registers:  VideoRegisters,

    pub oam:        OAM,
    pub palette:    PaletteRAM,
    pub vram:       V,
}

impl<V: VRAM2D> VideoMemory<V> {
    pub fn new(vram: V) -> Self {
        Self {
            registers:  VideoRegisters::new(),

            oam:        OAM::new(),
            palette:    PaletteRAM::new(),
            vram:       vram,
        }
    }
}
