mod vram;

use crate::utils::meminterface::MemInterface16;
use crate::common::videomem::{
    VideoMemory, VideoRegisters
};
use vram::{ARM9VRAM, EngineAVRAM, EngineBVRAM};
pub use vram::ARM7VRAM;

/// The NDS video memory.
/// Contains all of the registers, palette, OAM, and VRAM.
/// 
/// Acts as the interface between ARM9 and PPU/GPU.
pub struct DSVideoMemory {
    vram:           ARM9VRAM,

    pub engine_a_mem:   VideoMemory<EngineAVRAM>,
    pub engine_b_mem:   VideoMemory<EngineBVRAM>,

    // TODO: other + 3D
}

impl DSVideoMemory {
    pub fn new() -> (Self, ARM7VRAM) {
        let (arm9_vram, arm7_vram, eng_a_vram, eng_b_vram) = ARM9VRAM::new();

        (Self {
            vram:           arm9_vram,

            engine_a_mem:   VideoMemory::new(eng_a_vram),
            engine_b_mem:   VideoMemory::new(eng_b_vram)
        }, arm7_vram)
    }
}

// Mem interface
impl DSVideoMemory {
    pub fn mut_engine_a_regs<'a>(&'a mut self) -> &'a mut VideoRegisters {
        &mut self.engine_a_mem.registers
    }
    pub fn mut_engine_b_regs<'a>(&'a mut self) -> &'a mut VideoRegisters {
        &mut self.engine_b_mem.registers
    }

    pub fn read_halfword_palette(&mut self, addr: u32) -> u16 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.palette.read_halfword(addr)
        } else {
            self.engine_b_mem.palette.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_palette(&mut self, addr: u32, data: u16) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.palette.write_halfword(addr, data);
        } else {
            self.engine_b_mem.palette.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_palette(&mut self, addr: u32) -> u32 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.palette.read_word(addr)
        } else {
            self.engine_b_mem.palette.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_palette(&mut self, addr: u32, data: u32) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.palette.write_word(addr, data);
        } else {
            self.engine_b_mem.palette.write_word(addr & 0x3FF, data);
        }
    }

    pub fn read_halfword_oam(&mut self, addr: u32) -> u16 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.oam.read_halfword(addr)
        } else {
            self.engine_b_mem.oam.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_oam(&mut self, addr: u32, data: u16) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.oam.write_halfword(addr, data);
        } else {
            self.engine_b_mem.oam.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_oam(&mut self, addr: u32) -> u32 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.oam.read_word(addr)
        } else {
            self.engine_b_mem.oam.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_oam(&mut self, addr: u32, data: u32) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.oam.write_word(addr, data);
        } else {
            self.engine_b_mem.oam.write_word(addr & 0x3FF, data);
        }
    }
}