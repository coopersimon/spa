
use std::{
    convert::TryInto,
    rc::Rc,
    cell::RefCell
};
use crate::utils::meminterface::MemInterface16;
use crate::common::videomem::VRAM2D;

/// NDS VRAM.
/// Memory for BG, tiles, extended palettes, textures.
/// Also for LCDC transfer.
/// 
/// This struct represents the ARM9 side of the VRAM.
pub struct ARM9VRAM {
    a:  Vec<u8>,
    b:  Vec<u8>,
    c:  Vec<u8>,
    d:  Vec<u8>,
    e:  Vec<u8>,
    f:  Vec<u8>,
    g:  Vec<u8>,
    h:  Vec<u8>,
    i:  Vec<u8>,
}

impl ARM9VRAM {
    pub fn new() -> (Self, ARM7VRAM, EngineAVRAM, EngineBVRAM) {
        (Self {
            a:  vec![0; 128 * 1024],
            b:  vec![0; 128 * 1024],
            c:  vec![0; 128 * 1024],
            d:  vec![0; 128 * 1024],
            e:  vec![0; 64 * 1024],
            f:  vec![0; 16 * 1024],
            g:  vec![0; 16 * 1024],
            h:  vec![0; 32 * 1024],
            i:  vec![0; 16 * 1024],
        }, ARM7VRAM{},
            EngineAVRAM{},
            EngineBVRAM{}
        )
    }
}

impl MemInterface16 for ARM9VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        /*match addr {
            0x0..=0x1F_FFFF => 
        }*/
        0
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        
    }
}

/// Interface for ARM7 to access regions C and D.
pub struct ARM7VRAM {
    // c + d
}

impl MemInterface16 for ARM7VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        /*match addr {
            0x0..=0x1F_FFFF => 
        }*/
        0
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        
    }
}

/// VRAM accessible by 2D engine A.
pub struct EngineAVRAM {

}

impl VRAM2D for EngineAVRAM {
    /// Read a byte from VRAM.
    fn get_byte(&self, addr: u32) -> u8 {
        0
    }

    /// Read a halfword from VRAM.
    fn get_halfword(&self, addr: u32) -> u16 {
        0
    }
}

/// VRAM accessible by 2D engine B.
pub struct EngineBVRAM {

}

impl VRAM2D for EngineBVRAM {
    /// Read a byte from VRAM.
    fn get_byte(&self, addr: u32) -> u8 {
        0
    }

    /// Read a halfword from VRAM.
    fn get_halfword(&self, addr: u32) -> u16 {
        0
    }
}
