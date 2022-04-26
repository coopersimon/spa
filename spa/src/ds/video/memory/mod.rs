mod vram;
mod control;

use std::{
    convert::TryInto,
    rc::Rc,
    cell::RefCell,
    sync::{
        Arc, Mutex, atomic::{AtomicU8, Ordering}, MutexGuard
    },
};
use crate::utils::{
    meminterface::{MemInterface8, MemInterface16},
    bits::u8
};
use crate::common::wram::WRAM;
use crate::common::videomem::{
    VideoMemory, VideoRegisters
};
use vram::{ARM9VRAM, ARM7VRAMSlots, EngineAVRAM, EngineBVRAM};
pub use vram::ARM7VRAM;
use control::*;

/// The NDS video memory.
/// Contains all of the registers, palette, OAM, and VRAM.
/// 
/// Acts as the interface between ARM9 and PPU/GPU.
pub struct DSVideoMemory {
    vram:           ARM9VRAM,

    // Mem control
    a_cnt:  VRAMControl,
    b_cnt:  VRAMControl,
    c_cnt:  VRAMControl,
    d_cnt:  VRAMControl,
    e_cnt:  VRAMControl,
    f_cnt:  VRAMControl,
    g_cnt:  VRAMControl,
    h_cnt:  VRAMControl,
    i_cnt:  VRAMControl,

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

            a_cnt:          VRAMControl::default(),
            b_cnt:          VRAMControl::default(),
            c_cnt:          VRAMControl::default(),
            d_cnt:          VRAMControl::default(),
            e_cnt:          VRAMControl::default(),
            f_cnt:          VRAMControl::default(),
            g_cnt:          VRAMControl::default(),
            h_cnt:          VRAMControl::default(),
            i_cnt:          VRAMControl::default(),

            arm7_mem:       arm7_vram.mem.clone(),
            engine_a_mem:   Arc::new(Mutex::new(VideoMemory::new(eng_a_vram))),
            engine_b_mem:   Arc::new(Mutex::new(VideoMemory::new(eng_b_vram)))
        }, arm7_vram)
    }
}

// Registers
impl MemInterface8 for DSVideoMemory {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0 => self.a_cnt.bits(),
            1 => self.b_cnt.bits(),
            2 => self.c_cnt.bits(),
            3 => self.d_cnt.bits(),
            4 => self.e_cnt.bits(),
            5 => self.f_cnt.bits(),
            6 => self.g_cnt.bits(),

            8 => self.h_cnt.bits(),
            9 => self.i_cnt.bits(),
            _ => 0,
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0 => self.set_a_cnt(data),
            1 => self.set_b_cnt(data),
            2 => self.set_c_cnt(data),
            3 => self.set_d_cnt(data),
            4 => self.set_e_cnt(data),
            5 => self.set_f_cnt(data),
            6 => self.set_g_cnt(data),

            8 => self.set_h_cnt(data),
            9 => self.set_i_cnt(data),
            _ => {},
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

    pub fn read_halfword_palette(&mut self, addr: u32) -> u16 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.read_halfword(addr)
        } else {
            self.engine_b_mem.lock().unwrap().palette.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_palette(&mut self, addr: u32, data: u16) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.write_halfword(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().palette.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_palette(&mut self, addr: u32) -> u32 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.read_word(addr)
        } else {
            self.engine_b_mem.lock().unwrap().palette.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_palette(&mut self, addr: u32, data: u32) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().palette.write_word(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().palette.write_word(addr & 0x3FF, data);
        }
    }

    pub fn read_halfword_oam(&mut self, addr: u32) -> u16 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.read_halfword(addr)
        } else {
            self.engine_b_mem.lock().unwrap().oam.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_oam(&mut self, addr: u32, data: u16) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.write_halfword(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().oam.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_oam(&mut self, addr: u32) -> u32 {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.read_word(addr)
        } else {
            self.engine_b_mem.lock().unwrap().oam.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_oam(&mut self, addr: u32, data: u32) {
        let mod_addr = addr & 0x7FF;
        if mod_addr < 0x400 {
            self.engine_a_mem.lock().unwrap().oam.write_word(addr, data);
        } else {
            self.engine_b_mem.lock().unwrap().oam.write_word(addr & 0x3FF, data);
        }
    }
}

impl DSVideoMemory {
    fn get_mem(&mut self, from_slot: Slot) -> Box<WRAM> {
        match from_slot {
            Slot::LCDC(lcdc) => match lcdc {
                LCDC::A => self.vram.a.take().unwrap(),
                LCDC::B => self.vram.b.take().unwrap(),
                LCDC::C => self.vram.c.take().unwrap(),
                LCDC::D => self.vram.d.take().unwrap(),
                LCDC::E => self.vram.e.take().unwrap(),
                LCDC::F => self.vram.f.take().unwrap(),
                LCDC::G => self.vram.g.take().unwrap(),
                LCDC::H => self.vram.h.take().unwrap(),
                LCDC::I => self.vram.i.take().unwrap(),
            },
            Slot::ARM7(arm7) => match arm7 {
                ARM7::LO => self.arm7_mem.lock().unwrap().c.take().unwrap(),
                ARM7::HI => self.arm7_mem.lock().unwrap().d.take().unwrap(),
            },
            Slot::EngineA(slot) => {
                use EngineA::*;
                let engine_a = self.engine_a_mem.lock().unwrap();
                match slot {
                    BG_0    => engine_a.vram.bg_slot_0.take().unwrap(),
                    BG_01   => engine_a.vram.bg_slot_01.take().unwrap(),
                    BG_02   => engine_a.vram.bg_slot_02.take().unwrap(),
                    BG_03   => engine_a.vram.bg_slot_03.take().unwrap(),
                    BG_1    => engine_a.vram.bg_slot_1.take().unwrap(),
                    BG_2    => engine_a.vram.bg_slot_2.take().unwrap(),
                    BG_3    => engine_a.vram.bg_slot_3.take().unwrap(),
                
                    OBJ_0   => engine_a.vram.obj_slot_0.take().unwrap(),
                    OBJ_01  => engine_a.vram.obj_slot_01.take().unwrap(),
                    OBJ_02  => engine_a.vram.obj_slot_02.take().unwrap(),
                    OBJ_03  => engine_a.vram.obj_slot_03.take().unwrap(),
                    OBJ_1   => engine_a.vram.obj_slot_1.take().unwrap(),
                
                    BG_EXT_PALETTE_0 => engine_a.vram.ext_bg_palette_0.take().unwrap(),
                    BG_EXT_PALETTE_2 => engine_a.vram.ext_bg_palette_2.take().unwrap(),
                
                    OBJ_EXT_PALETTE => engine_a.vram.ext_obj_palette.take().unwrap()
                }
            },
            Slot::EngineB(slot) => {
                use EngineB::*;
                let engine_b = self.engine_b_mem.lock().unwrap();
                match slot {
                    BG_0    => engine_b.vram.bg_slot_0.take().unwrap(),
                    BG_01   => engine_b.vram.bg_slot_01.take().unwrap(),
                
                    OBJ   => engine_b.vram.obj_slot.take().unwrap(),
                
                    BG_EXT_PALETTE  => engine_b.vram.ext_bg_palette.take().unwrap(),
                    OBJ_EXT_PALETTE => engine_b.vram.ext_obj_palette.take().unwrap()
                }
            },
            Slot::Texture(_) => panic!("TEX unsupported right now"),
        }
    }

    fn set_mem(&mut self, from_slot: Slot, mem: Box<WRAM>) {
        match from_slot {
            Slot::LCDC(lcdc) => match lcdc {
                LCDC::A => self.vram.a = Some(mem),
                LCDC::B => self.vram.b = Some(mem),
                LCDC::C => self.vram.c = Some(mem),
                LCDC::D => self.vram.d = Some(mem),
                LCDC::E => self.vram.e = Some(mem),
                LCDC::F => self.vram.f = Some(mem),
                LCDC::G => self.vram.g = Some(mem),
                LCDC::H => self.vram.h = Some(mem),
                LCDC::I => self.vram.i = Some(mem),
            },
            Slot::ARM7(arm7) => match arm7 {
                ARM7::LO => self.arm7_mem.lock().unwrap().c = Some(mem),
                ARM7::HI => self.arm7_mem.lock().unwrap().d = Some(mem),
            },
            Slot::EngineA(slot) => {
                use EngineA::*;
                let engine_a = self.engine_a_mem.lock().unwrap();
                match slot {
                    BG_0    => engine_a.vram.bg_slot_0 = Some(mem),
                    BG_01   => engine_a.vram.bg_slot_01 = Some(mem),
                    BG_02   => engine_a.vram.bg_slot_02 = Some(mem),
                    BG_03   => engine_a.vram.bg_slot_03 = Some(mem),
                    BG_1    => engine_a.vram.bg_slot_1 = Some(mem),
                    BG_2    => engine_a.vram.bg_slot_2 = Some(mem),
                    BG_3    => engine_a.vram.bg_slot_3 = Some(mem),
                
                    OBJ_0   => engine_a.vram.obj_slot_0 = Some(mem),
                    OBJ_01  => engine_a.vram.obj_slot_01 = Some(mem),
                    OBJ_02  => engine_a.vram.obj_slot_02 = Some(mem),
                    OBJ_03  => engine_a.vram.obj_slot_03 = Some(mem),
                    OBJ_1   => engine_a.vram.obj_slot_1 = Some(mem),
                
                    BG_EXT_PALETTE_0 => engine_a.vram.ext_bg_palette_0 = Some(mem),
                    BG_EXT_PALETTE_2 => engine_a.vram.ext_bg_palette_2 = Some(mem),
                
                    OBJ_EXT_PALETTE => engine_a.vram.ext_obj_palette = Some(mem)
                }
            },
            Slot::EngineB(slot) => {
                use EngineB::*;
                let engine_b = self.engine_b_mem.lock().unwrap();
                match slot {
                    BG_0    => engine_b.vram.bg_slot_0 = Some(mem),
                    BG_01   => engine_b.vram.bg_slot_01 = Some(mem),
                
                    OBJ   => engine_b.vram.obj_slot = Some(mem),
                
                    BG_EXT_PALETTE  => engine_b.vram.ext_bg_palette = Some(mem),
                    OBJ_EXT_PALETTE => engine_b.vram.ext_obj_palette = Some(mem)
                }
            },
            Slot::Texture(_) => panic!("TEX unsupported right now"),
        }
    }

    fn set_a_cnt(&mut self, data: u8) {
        let from_slot = self.a_cnt.slot_ab();
        let mem = self.get_mem(from_slot);
        self.a_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.a_cnt.slot_ab();
        self.set_mem(to_slot, mem);
    }

    fn set_b_cnt(&mut self, data: u8) {
        let from_slot = self.b_cnt.slot_ab();
        let mem = self.get_mem(from_slot);
        self.b_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.b_cnt.slot_ab();
        self.set_mem(to_slot, mem);
    }

    fn set_c_cnt(&mut self, data: u8) {
        let from_slot = self.c_cnt.slot_cd();
        let mem = self.get_mem(from_slot);
        self.c_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.c_cnt.slot_cd();
        self.set_mem(to_slot, mem);
    }

    fn set_d_cnt(&mut self, data: u8) {
        let from_slot = self.d_cnt.slot_cd();
        let mem = self.get_mem(from_slot);
        self.d_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.d_cnt.slot_cd();
        self.set_mem(to_slot, mem);
    }

    fn set_e_cnt(&mut self, data: u8) {
        let from_slot = self.e_cnt.slot_e();
        let mem = self.get_mem(from_slot);
        self.e_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.e_cnt.slot_e();
        self.set_mem(to_slot, mem);
    }

    fn set_f_cnt(&mut self, data: u8) {
        let from_slot = self.f_cnt.slot_fg();
        let mem = self.get_mem(from_slot);
        self.f_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.f_cnt.slot_fg();
        self.set_mem(to_slot, mem);
    }

    fn set_g_cnt(&mut self, data: u8) {
        let from_slot = self.g_cnt.slot_fg();
        let mem = self.get_mem(from_slot);
        self.g_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.g_cnt.slot_fg();
        self.set_mem(to_slot, mem);
    }

    fn set_h_cnt(&mut self, data: u8) {
        let from_slot = self.h_cnt.slot_h();
        let mem = self.get_mem(from_slot);
        self.h_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.h_cnt.slot_h();
        self.set_mem(to_slot, mem);
    }

    fn set_i_cnt(&mut self, data: u8) {
        let from_slot = self.i_cnt.slot_i();
        let mem = self.get_mem(from_slot);
        self.i_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.i_cnt.slot_i();
        self.set_mem(to_slot, mem);
    }
}
