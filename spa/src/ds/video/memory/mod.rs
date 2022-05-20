mod vram;
mod control;

use std::{
    sync::{
        Arc, Mutex, MutexGuard
    },
};
use crate::utils::{
    meminterface::MemInterface16,
    bits::u8
};
use crate::common::wram::WRAM;
use crate::common::videomem::VideoMemory;
use vram::{ARM9VRAM, ARM7VRAMSlots, EngineAVRAM, EngineBVRAM};
pub use vram::ARM7VRAM;
use control::*;

#[repr(usize)]
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum VRAMRegion {
    A = 0,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I
}

struct VRAMControlModule {
    cnt:    VRAMControl,
    slot:   Slot,
}

impl VRAMControlModule {
    fn new(at_slot: Slot) -> Self {
        Self {
            cnt:    VRAMControl::default(),
            slot:   at_slot,
        }
    }
}

/// The NDS video memory.
/// Contains all of the registers, palette, OAM, and VRAM.
/// 
/// Acts as the interface between ARM9 and PPU/GPU.
pub struct DSVideoMemory {
    vram:           ARM9VRAM,

    mem_control:   [VRAMControlModule; 9],

    arm7_mem:           Arc<Mutex<ARM7VRAMSlots>>,
    pub engine_a_mem:   Arc<Mutex<VideoMemory<EngineAVRAM>>>,
    pub engine_b_mem:   Arc<Mutex<VideoMemory<EngineBVRAM>>>,

    // TODO: other + 3D
}

impl DSVideoMemory {
    pub fn new() -> (Self, ARM7VRAM) {
        let (arm9_vram, arm7_vram, eng_a_vram, eng_b_vram) = ARM9VRAM::new();

        (Self {
            vram:           arm9_vram,

            mem_control:    [
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::A)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::B)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::C)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::D)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::E)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::F)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::G)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::H)),
                VRAMControlModule::new(Slot::LCDC(VRAMRegion::I)),
            ],

            arm7_mem:       arm7_vram.mem.clone(),
            engine_a_mem:   Arc::new(Mutex::new(VideoMemory::new(eng_a_vram))),
            engine_b_mem:   Arc::new(Mutex::new(VideoMemory::new(eng_b_vram)))
        }, arm7_vram)
    }
}

impl DSVideoMemory {
    pub fn get_cnt(&self, region: VRAMRegion) -> u8 {
        self.mem_control[region as usize].cnt.bits()
    }

    pub fn set_cnt(&mut self, region: VRAMRegion, data: u8) {
        // Get mem from current slot.
        let mem = self.swap_mem(self.mem_control[region as usize].slot, None);
        let cnt = VRAMControl::from_bits_truncate(data);
        // Set mem in new slot.
        let to_slot = cnt.get_slot(region);
        //println!("move {:?} | {:?} => {:?}", region, self.mem_control[region as usize].slot, to_slot);
        self.mem_control[region as usize].cnt = cnt;
        self.mem_control[region as usize].slot = to_slot;
        let prev_mem = self.swap_mem(to_slot, mem);
        if prev_mem.is_some() {
            // There was already something in the slot.
            let old = self.lookup_at_slot(to_slot).unwrap();
            self.vram.lcdc[old] = prev_mem;
            self.mem_control[old].slot = Slot::LCDC(region);    // TODO: convert old to VRAMRegion
        }
    }
}

// Mem interface: VRAM
impl DSVideoMemory {

    pub fn read_byte_vram(&mut self, addr: u32) -> u8 {
        (match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_bg(addr)
                    .map(|(vram, offset)| vram.read_byte(addr - offset))
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.lookup_bg(addr)
                    .map(|(vram, offset)| vram.read_byte(addr - offset))
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_obj(addr)
                    .map(|(vram, offset)| vram.read_byte(addr - offset))
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.obj_slot.as_mut().map(|v| v.read_byte(addr))
            },
            _ => {
                let (vram, offset) = self.ref_lcdc_vram(addr);
                vram.map(|v| v.read_byte(addr - offset))
            }
        }).unwrap_or(0)
    }
    pub fn write_byte_vram(&mut self, addr: u32, data: u8) {
        match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_bg_mut(addr)
                    .map(|(vram, offset)| vram.write_byte(addr - offset, data));
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.lookup_bg_mut(addr)
                    .map(|(vram, offset)| vram.write_byte(addr - offset, data));
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_obj_mut(addr)
                    .map(|(vram, offset)| vram.write_byte(addr - offset, data));
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.obj_slot.as_mut().map(|v| v.write_byte(addr, data));
            },
            _ => {
                let (vram, offset) = self.ref_lcdc_vram(addr);
                vram.map(|v| v.write_byte(addr - offset, data));
            }
        }
    }

    pub fn read_halfword_vram(&mut self, addr: u32) -> u16 {
        (match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_bg(addr)
                    .map(|(vram, offset)| vram.read_halfword(addr - offset))
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.lookup_bg(addr)
                    .map(|(vram, offset)| vram.read_halfword(addr - offset))
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_obj(addr)
                    .map(|(vram, offset)| vram.read_halfword(addr - offset))
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.obj_slot.as_mut().map(|v| v.read_halfword(addr))
            },
            _ => {
                let (vram, offset) = self.ref_lcdc_vram(addr);
                vram.map(|v| v.read_halfword(addr - offset))
            }
        }).unwrap_or(0)
    }
    pub fn write_halfword_vram(&mut self, addr: u32, data: u16) {
        match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_bg_mut(addr)
                    .map(|(vram, offset)| vram.write_halfword(addr - offset, data));
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.lookup_bg_mut(addr)
                    .map(|(vram, offset)| vram.write_halfword(addr - offset, data));
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_obj_mut(addr)
                    .map(|(vram, offset)| vram.write_halfword(addr - offset, data));
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.obj_slot.as_mut().map(|v| v.write_halfword(addr, data));
            },
            _ => {
                let (vram, offset) = self.ref_lcdc_vram(addr);
                vram.map(|v| v.write_halfword(addr - offset, data));
            }
        }
    }

    pub fn read_word_vram(&mut self, addr: u32) -> u32 {
        (match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_bg(addr)
                    .map(|(vram, offset)| vram.read_word(addr - offset))
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.lookup_bg(addr)
                    .map(|(vram, offset)| vram.read_word(addr - offset))
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_obj(addr)
                    .map(|(vram, offset)| vram.read_word(addr - offset))
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.obj_slot.as_mut().map(|v| v.read_word(addr))
            },
            _ => {
                let (vram, offset) = self.ref_lcdc_vram(addr);
                vram.map(|v| v.read_word(addr - offset))
            }
        }).unwrap_or(0)
    }
    pub fn write_word_vram(&mut self, addr: u32, data: u32) {
        match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_bg_mut(addr)
                    .map(|(vram, offset)| vram.write_word(addr - offset, data));
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.lookup_bg_mut(addr)
                    .map(|(vram, offset)| vram.write_word(addr - offset, data));
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                engine_a.vram.lookup_obj_mut(addr)
                    .map(|(vram, offset)| vram.write_word(addr - offset, data));
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                engine_b.vram.obj_slot.as_mut().map(|v| v.write_word(addr, data));
            },
            _ => {
                let (vram, offset) = self.ref_lcdc_vram(addr);
                vram.map(|v| v.write_word(addr - offset, data));
            }
        }
    }
}

// Mem interface: engine memory
impl DSVideoMemory {
    pub fn mut_engine_a<'a>(&'a mut self) -> MutexGuard<'a, VideoMemory<EngineAVRAM>> {
        self.engine_a_mem.lock().unwrap()
    }
    pub fn mut_engine_b<'a>(&'a mut self) -> MutexGuard<'a, VideoMemory<EngineBVRAM>> {
        self.engine_b_mem.lock().unwrap()
    }

    pub fn read_byte_palette(&mut self, addr: u32) -> u8 {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.read_byte(addr)
        } else {
            self.engine_b_mem.lock().unwrap().palette.read_byte(addr & 0x3FF)
        }
    }
    pub fn write_byte_palette(&mut self, addr: u32, data: u8) {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.write_byte(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().palette.write_byte(addr & 0x3FF, data);
        }
    }

    pub fn read_halfword_palette(&mut self, addr: u32) -> u16 {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.read_halfword(addr)
        } else {
            self.engine_b_mem.lock().unwrap().palette.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_palette(&mut self, addr: u32, data: u16) {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.write_halfword(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().palette.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_palette(&mut self, addr: u32) -> u32 {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.read_word(addr)
        } else {
            self.engine_b_mem.lock().unwrap().palette.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_palette(&mut self, addr: u32, data: u32) {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.write_word(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().palette.write_word(addr & 0x3FF, data);
        }
    }

    pub fn read_byte_oam(&mut self, addr: u32) -> u8 {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.read_byte(addr)
        } else {
            self.engine_b_mem.lock().unwrap().oam.read_byte(addr & 0x3FF)
        }
    }
    pub fn write_byte_oam(&mut self, addr: u32, data: u8) {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.write_byte(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().oam.write_byte(addr & 0x3FF, data);
        }
    }

    pub fn read_halfword_oam(&mut self, addr: u32) -> u16 {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.read_halfword(addr)
        } else {
            self.engine_b_mem.lock().unwrap().oam.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_oam(&mut self, addr: u32, data: u16) {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.write_halfword(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().oam.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_oam(&mut self, addr: u32) -> u32 {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.read_word(addr)
        } else {
            self.engine_b_mem.lock().unwrap().oam.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_oam(&mut self, addr: u32, data: u32) {
        if addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.write_word(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().oam.write_word(addr & 0x3FF, data);
        }
    }
}

impl DSVideoMemory {
    fn swap_mem(&mut self, from_slot: Slot, new: Option<Box<WRAM>>) -> Option<Box<WRAM>> {
        match from_slot {
            Slot::LCDC(lcdc) => std::mem::replace(&mut self.vram.lcdc[lcdc as usize], new),
            Slot::ARM7(arm7) => match arm7 {
                ARM7::Lo => std::mem::replace(&mut self.arm7_mem.lock().unwrap().c, new),
                ARM7::Hi => std::mem::replace(&mut self.arm7_mem.lock().unwrap().d, new),
            },
            Slot::EngineA(slot) => {
                use EngineA::*;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                match slot {
                    Bg0    => std::mem::replace(&mut engine_a.vram.bg_slot_0, new),
                    Bg01   => std::mem::replace(&mut engine_a.vram.bg_slot_01, new),
                    Bg02   => std::mem::replace(&mut engine_a.vram.bg_slot_02, new),
                    Bg03   => std::mem::replace(&mut engine_a.vram.bg_slot_03, new),
                    Bg1    => std::mem::replace(&mut engine_a.vram.bg_slot_1, new),
                    Bg2    => std::mem::replace(&mut engine_a.vram.bg_slot_2, new),
                    Bg3    => std::mem::replace(&mut engine_a.vram.bg_slot_3, new),
                
                    Obj0   => std::mem::replace(&mut engine_a.vram.obj_slot_0, new),
                    Obj01  => std::mem::replace(&mut engine_a.vram.obj_slot_01, new),
                    Obj02  => std::mem::replace(&mut engine_a.vram.obj_slot_02, new),
                    Obj03  => std::mem::replace(&mut engine_a.vram.obj_slot_03, new),
                    Obj1   => std::mem::replace(&mut engine_a.vram.obj_slot_1, new),
                
                    BgExtPalette0 => {
                        engine_a.vram.ext_bg_palette_dirty = true;
                        std::mem::replace(&mut engine_a.vram.ext_bg_palette_0, new)
                    },
                    BgExtPalette2 => {
                        engine_a.vram.ext_bg_palette_dirty = true;
                        std::mem::replace(&mut engine_a.vram.ext_bg_palette_2, new)
                    },
                
                    ObjExtPalette => {
                        engine_a.vram.ext_obj_palette_dirty = true;
                        std::mem::replace(&mut engine_a.vram.ext_obj_palette, new)
                    }
                }
            },
            Slot::EngineB(slot) => {
                use EngineB::*;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                match slot {
                    Bg0    => std::mem::replace(&mut engine_b.vram.bg_slot_0, new),
                    Bg01   => std::mem::replace(&mut engine_b.vram.bg_slot_01, new),
                
                    Obj   => std::mem::replace(&mut engine_b.vram.obj_slot, new),
                
                    BgExtPalette  => {
                        engine_b.vram.ext_bg_palette_dirty = true;
                        std::mem::replace(&mut engine_b.vram.ext_bg_palette, new)
                    },
                    ObjExtPalette => {
                        engine_b.vram.ext_bg_palette_dirty = true;
                        std::mem::replace(&mut engine_b.vram.ext_obj_palette, new)
                    }
                }
            },
            Slot::Texture(_) => panic!("TEX unsupported right now"),
        }
    }

    /// Find which VRAM region is at slot
    fn lookup_at_slot(&mut self, slot: Slot) -> Option<usize> {
        for (n, region) in self.mem_control.iter().enumerate() {
            if region.slot == slot {
                return Some(n);
            }
        }
        None
    }

    /// Get a reference to the relevant lcdc memory region.
    fn ref_lcdc_vram<'a>(&'a mut self, addr: u32) -> (Option<&'a mut Box<WRAM>>, u32) {
        match addr {
            0x0680_0000..=0x0681_FFFF => (self.vram.lcdc[VRAMRegion::A as usize].as_mut(), 0x0680_0000),
            0x0682_0000..=0x0683_FFFF => (self.vram.lcdc[VRAMRegion::B as usize].as_mut(), 0x0682_0000),
            0x0684_0000..=0x0685_FFFF => (self.vram.lcdc[VRAMRegion::C as usize].as_mut(), 0x0684_0000),
            0x0686_0000..=0x0687_FFFF => (self.vram.lcdc[VRAMRegion::D as usize].as_mut(), 0x0686_0000),
            0x0688_0000..=0x0688_FFFF => (self.vram.lcdc[VRAMRegion::E as usize].as_mut(), 0x0688_0000),
            0x0689_0000..=0x0689_3FFF => (self.vram.lcdc[VRAMRegion::F as usize].as_mut(), 0x0689_0000),
            0x0689_4000..=0x0689_7FFF => (self.vram.lcdc[VRAMRegion::G as usize].as_mut(), 0x0689_4000),
            0x0689_8000..=0x0689_FFFF => (self.vram.lcdc[VRAMRegion::H as usize].as_mut(), 0x0689_8000),
            0x068A_0000..=0x068A_3FFF => (self.vram.lcdc[VRAMRegion::I as usize].as_mut(), 0x068A_0000),
            _ => panic!("accessing LCDC image"),
        }
    }
}
