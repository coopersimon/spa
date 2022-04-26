
use bitflags::bitflags;
use std::{
    convert::TryInto,
    rc::Rc,
    cell::RefCell,
    sync::{
        Arc, Mutex, atomic::{AtomicU8, Ordering}, MutexGuard
    },
};
use crate::utils::{
    meminterface::MemInterface16,
    bits::u8
};
use crate::common::{
    videomem::VRAM2D,
    wram::WRAM
};

bitflags!{
    #[derive(Default)]
    pub struct VRAMStatus: u8 {
        const VRAM_D    = u8::bit(1);
        const VRAM_C    = u8::bit(0);
    }
}

/// NDS VRAM.
/// Memory for BG, tiles, extended palettes, textures.
/// Also for LCDC transfer.
/// 
/// This struct represents the ARM9 side of the VRAM.
pub struct ARM9VRAM {
    // Memory blocks
    pub a:  VRAMSlot,
    pub b:  VRAMSlot,
    pub c:  VRAMSlot,
    pub d:  VRAMSlot,
    pub e:  VRAMSlot,
    pub f:  VRAMSlot,
    pub g:  VRAMSlot,
    pub h:  VRAMSlot,
    pub i:  VRAMSlot,
}

type VRAMSlot = Option<Box<WRAM>>; // TODO: does this need to be boxed even?

impl ARM9VRAM {
    pub fn new() -> (Self, ARM7VRAM, EngineAVRAM, EngineBVRAM) {
        let arm7_vram = ARM7VRAM::default();
        let engine_a_vram = EngineAVRAM::default();
        let engine_b_vram = EngineBVRAM::default();
        let arm9_vram = Self {
            a:  Some(Box::new(WRAM::new(128 * 1024))),
            b:  Some(Box::new(WRAM::new(128 * 1024))),
            c:  Some(Box::new(WRAM::new(128 * 1024))),
            d:  Some(Box::new(WRAM::new(128 * 1024))),
            e:  Some(Box::new(WRAM::new(64 * 1024))),
            f:  Some(Box::new(WRAM::new(16 * 1024))),
            g:  Some(Box::new(WRAM::new(16 * 1024))),
            h:  Some(Box::new(WRAM::new(32 * 1024))),
            i:  Some(Box::new(WRAM::new(16 * 1024))),

            //arm7_status:    Arc::new(AtomicU8::new(VRAMStatus::default().bits()))
        };
        (
            arm9_vram,
            arm7_vram,
            engine_a_vram,
            engine_b_vram
        )
    }
}

impl MemInterface16 for ARM9VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        /*match addr {
            0x0600_0000 => {
                // A BG-VRAM
            },
            0x20_0000..=
        }*/
        0
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        
    }
}

/// Interface for ARM7 to access regions C and D.
#[derive(Default)]
pub struct ARM7VRAM {
    pub mem: Arc<Mutex<ARM7VRAMSlots>>
}

#[derive(Default)]
pub struct ARM7VRAMSlots {
    pub c: VRAMSlot,
    pub d: VRAMSlot,
}

impl ARM7VRAM {
    pub fn get_status(&self) -> u8 {
        let slots = self.mem.lock().unwrap();
        let mut status = VRAMStatus::empty();
        status.set(VRAMStatus::VRAM_C, slots.c.is_some());
        status.set(VRAMStatus::VRAM_D, slots.d.is_some());
        status.bits()
    }
}

impl MemInterface16 for ARM7VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let slots = self.mem.lock().unwrap();
        match addr {
            0x0600_0000..=0x0601_FFFF => match slots.c {
                Some(vram) => vram.read_halfword(addr),
                None => 0,
            },
            0x0602_0000..=0x0603_FFFF => match slots.d {
                Some(vram) => vram.read_halfword(addr),
                None => 0,
            },
            _ => panic!("reading from weird VRAM addr (ARM7: {:X})", addr),
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let slots = self.mem.lock().unwrap();
        match addr {
            0x0600_0000..=0x0601_FFFF => match slots.c {
                Some(mut vram) => vram.write_halfword(addr, data),
                None => {},
            },
            0x0602_0000..=0x0603_FFFF => match slots.d {
                Some(mut vram) => vram.write_halfword(addr, data),
                None => {},
            },
            _ => panic!("reading from weird VRAM addr (ARM7: {:X})", addr),
        }
    }
}

/// VRAM accessible by 2D engine A.
#[derive(Default)]
pub struct EngineAVRAM {
    pub bg_slot_0:  VRAMSlot,   // Up to 128K,  from 0600_0000
    pub bg_slot_01: VRAMSlot,   // 16K,         from 0600_4000
    pub bg_slot_02: VRAMSlot,   // 16K,         from 0601_0000
    pub bg_slot_03: VRAMSlot,   // 16K,         from 0601_4000
    pub bg_slot_1:  VRAMSlot,   // 128K,        from 0602_0000
    pub bg_slot_2:  VRAMSlot,   // 128K,        from 0604_0000
    pub bg_slot_3:  VRAMSlot,   // 128K,        from 0606_0000

    pub obj_slot_0:  VRAMSlot,   // Up to 128K, from 0640_0000
    pub obj_slot_01: VRAMSlot,   // 16K,        from 0640_4000
    pub obj_slot_02: VRAMSlot,   // 16K,        from 0641_0000
    pub obj_slot_03: VRAMSlot,   // 16K,        from 0641_4000
    pub obj_slot_1:  VRAMSlot,   // 128K,       from 0642_0000

    pub ext_bg_palette_0: VRAMSlot, // Up to 32K
    pub ext_bg_palette_2: VRAMSlot, // 16K
    pub ext_obj_palette:  VRAMSlot, // 8K
}

impl EngineAVRAM {
    /// Get the VRAM slot for the BG addr provided.
    /// 
    /// Returns the offset (TODO: return mask?)
    fn lookup_bg<'a>(&'a self, addr: u32) -> (Option<&'a Box<WRAM>>, u32) {
        //let slots = self.mem.lock().unwrap();
        match addr {
            0x4000..=0x7FFF => match self.bg_slot_01 {
                None => (self.bg_slot_01.as_ref(), 0),
                vram => (vram.as_ref(), 0x4000)
            },
            0x1_0000..=0x1_3FFF => match self.bg_slot_02 {
                None => (self.bg_slot_0.as_ref(), 0),
                vram => (vram.as_ref(), 0x1_0000)
            },
            0x1_4000..=0x1_7FFF => match self.bg_slot_03 {
                None => (self.bg_slot_0.as_ref(), 0),
                vram => (vram.as_ref(), 0x1_4000)
            },
            0x0..=0x1_FFFF => (self.bg_slot_0.as_ref(), 0),
            0x2_0000..=0x3_FFFF => (self.bg_slot_1.as_ref(), 0x2_0000),
            0x4_0000..=0x5_FFFF => (self.bg_slot_2.as_ref(), 0x4_0000),
            0x6_0000..=0x7_FFFF => (self.bg_slot_3.as_ref(), 0x6_0000),
            _ => (None, 0)
        }
    }

    /// Get the VRAM slot for the OBJ addr provided.
    /// 
    /// Returns the offset (TODO: return mask?)
    fn lookup_obj<'a>(&'a self, addr: u32) -> (Option<&'a Box<WRAM>>, u32) {
        match addr {
            0x4000..=0x7FFF => match self.obj_slot_01 {
                None => (self.obj_slot_0.as_ref(), 0),
                vram => (vram.as_ref(), 0x4000)
            },
            0x1_0000..=0x1_3FFF => match self.obj_slot_02 {
                None => (self.obj_slot_0.as_ref(), 0),
                vram => (vram.as_ref(), 0x1_0000)
            },
            0x1_4000..=0x1_7FFF => match self.obj_slot_03 {
                None => (self.obj_slot_0.as_ref(), 0),
                vram => (vram.as_ref(), 0x1_4000)
            },
            0x0..=0x1_FFFF => (self.obj_slot_0.as_ref(), 0),
            0x2_0000..=0x3_FFFF => (self.obj_slot_1.as_ref(), 0x2_0000),
            _ => (None, 0)
        }
    }
}

impl VRAM2D for EngineAVRAM {
    /// Read a byte from VRAM.
    fn get_byte(&self, addr: u32) -> u8 {
        if let (Some(vram), offset) = self.lookup_bg(addr) {
            vram.read_byte(addr - offset)
        } else {
            panic!("reading from strange addr (ENG_A_BG: {:X})", addr)
        }
    }

    /// Read a halfword from VRAM.
    fn get_halfword(&self, addr: u32) -> u16 {
        if let (Some(vram), offset) = self.lookup_bg(addr) {
            vram.read_halfword(addr - offset)
        } else {
            panic!("reading from strange addr (ENG_A_BG: {:X})", addr)
        }
    }
}

/// VRAM accessible by 2D engine B.
#[derive(Default)]
pub struct EngineBVRAM {
    pub bg_slot_0:  VRAMSlot,   // Up to 128K,  from 0620_0000
    pub bg_slot_01: VRAMSlot,   // 16K,         from 0620_8000

    pub obj_slot:   VRAMSlot,   // Up to 128K, from 0660_0000

    pub ext_bg_palette:     VRAMSlot, // 32K
    pub ext_obj_palette:    VRAMSlot, // 16K
}

impl EngineBVRAM {
    /// Get the VRAM slot for the BG addr provided.
    /// 
    /// Returns the offset (TODO: return mask?)
    fn lookup_bg<'a>(&'a self, addr: u32) -> (Option<&'a Box<WRAM>>, u32) {
        match addr {
            0x8000..=0xBFFF => match self.bg_slot_01 {
                None => (self.bg_slot_0.as_ref(), 0),
                vram => (vram.as_ref(), 0x8000)
            },
            0x0..=0x1_FFFF => (self.bg_slot_0.as_ref(), 0),
            _ => (None, 0)
        }
    }
}

impl VRAM2D for EngineBVRAM {
    /// Read a byte from VRAM.
    fn get_byte(&self, addr: u32) -> u8 {
        if let (Some(vram), offset) = self.lookup_bg(addr) {
            vram.read_byte(addr - offset)
        } else {
            panic!("reading from strange addr (ENG_B_BG: {:X})", addr)
        }
    }

    /// Read a halfword from VRAM.
    fn get_halfword(&self, addr: u32) -> u16 {
        if let (Some(vram), offset) = self.lookup_bg(addr) {
            vram.read_halfword(addr - offset)
        } else {
            panic!("reading from strange addr (ENG_B_BG: {:X})", addr)
        }
    }
}
