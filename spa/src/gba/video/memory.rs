/// Video memory

use std::{
    convert::TryInto,
    rc::Rc,
    cell::RefCell
};
use crate::utils::meminterface::MemInterface16;
use crate::common::videomem::VRAM2D;

const VRAM_SIZE: u32 = 96 * 1024;
const OBJ_VRAM_SIZE: u32 = 32 * 1024;

/// VRAM. Contains tile data, background maps, and bitmaps.
pub struct VRAM {
    data: Rc<RefCell<Vec<u8>>>
}

// Memory interface
impl VRAM {
    pub fn new() -> (Self, VRAMRenderRef) {
        let data = Rc::new(RefCell::new(vec![0; VRAM_SIZE as usize]));
        (Self {
            data: data.clone()
        }, VRAMRenderRef{
            data: data
        })
    }
}

impl MemInterface16 for VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let start = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        } as usize;
        let end = start + 2;
        let data: [u8; 2] = (self.data.borrow()[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let start = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        } as usize;
        let end = start + 2;
        for (dest, byte) in self.data.borrow_mut()[start..end].iter_mut().zip(&data.to_le_bytes()) {
            *dest = *byte;
        }
    }
}

/// Used in the renderer.
pub struct VRAMRenderRef {
    data: Rc<RefCell<Vec<u8>>>
}

impl VRAM2D for VRAMRenderRef {
    fn get_byte(&self, addr: u32) -> u8 {
        self.data.borrow()[addr as usize]
    }

    fn get_halfword(&self, addr: u32) -> u16 {
        let start = addr as usize;
        let end = start + 2;
        let data: [u8; 2] = (self.data.borrow()[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }
}
