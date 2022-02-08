/// Palette memory

use crate::common::meminterface::MemInterface16;

/// Total size of bg or object palettes.
const PALETTE_SIZE: usize = 256;

pub struct PaletteRAM {
    palette_ram:    Vec<u16>,
    /// Set to true when the bg palette is overwritten.
    bg_palette_dirty:   bool,
    /// Set to true when the obj palette is overwritten.
    obj_palette_dirty:  bool,
}

impl PaletteRAM {
    pub fn new() -> Self {
        Self {
            palette_ram:        vec![0; PALETTE_SIZE * 2],

            bg_palette_dirty:   true,
            obj_palette_dirty:  true,
        }
    }

    pub fn ref_bg_palette<'a>(&'a mut self) -> Option<&'a [u16]> {
        if self.bg_palette_dirty {
            self.bg_palette_dirty = false;
            Some(&self.palette_ram[0..PALETTE_SIZE])
        } else {
            None
        }
    }

    pub fn ref_obj_palette<'a>(&'a mut self) -> Option<&'a [u16]> {
        if self.obj_palette_dirty {
            self.obj_palette_dirty = false;
            Some(&self.palette_ram[PALETTE_SIZE..])
        } else {
            None
        }
    }
}

impl MemInterface16 for PaletteRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let colour = (addr >> 1) as usize;
        self.palette_ram[colour]
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let colour = (addr >> 1) as usize;
        self.palette_ram[colour] = data;
        if colour < PALETTE_SIZE {
            self.bg_palette_dirty = true;
        } else {
            self.obj_palette_dirty = true;
        }
    }
}
