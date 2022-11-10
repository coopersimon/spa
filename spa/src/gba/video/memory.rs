/// Video memory

use std::{
    rc::Rc,
    cell::RefCell
};
use crate::utils::meminterface::MemInterface16;
use crate::common::{
    mem::ram::RAM,
    video::mem::VRAM2D
};

const VRAM_SIZE: u32 = 96 * 1024;
const OBJ_VRAM_SIZE: u32 = 32 * 1024;

/// VRAM. Contains tile data, background maps, and bitmaps.
pub struct VRAM {
    data: Rc<RefCell<RAM>>
}

// Memory interface
impl VRAM {
    pub fn new() -> (Self, VRAMRenderRef) {
        let data = Rc::new(RefCell::new(RAM::new(VRAM_SIZE as usize)));
        (Self {
            data: data.clone()
        }, VRAMRenderRef{
            data: data
        })
    }
}

impl MemInterface16 for VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let read_addr = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        };

        self.data.borrow().read_halfword(read_addr)
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let write_addr = if addr < VRAM_SIZE {
            addr
        } else {
            addr - OBJ_VRAM_SIZE
        };

        self.data.borrow_mut().write_halfword(write_addr, data);
    }
}

/// Used in the renderer.
pub struct VRAMRenderRef {
    data: Rc<RefCell<RAM>>
}

const OBJECT_VRAM_BASE: u32 = 64 * 1024;
impl VRAM2D for VRAMRenderRef {
    fn get_bg_byte(&self, addr: u32) -> u8 {
        self.data.borrow().read_byte(addr)
    }

    fn get_bg_halfword(&self, addr: u32) -> u16 {
        self.data.borrow().read_halfword(addr)
    }

    fn get_obj_byte(&self, addr: u32) -> u8 {
        self.data.borrow().read_byte(OBJECT_VRAM_BASE + addr)
    }

    fn get_obj_halfword(&self, addr: u32) -> u16 {
        self.data.borrow().read_halfword(OBJECT_VRAM_BASE + addr)
    }
}
