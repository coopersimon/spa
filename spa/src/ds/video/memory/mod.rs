mod vram;
mod control;

use std::{
    sync::{
        Arc, Mutex, MutexGuard
    },
};
use crate::utils::{
    meminterface::{MemInterface8, MemInterface16},
    bits::u8
};
use crate::common::wram::WRAM;
use crate::common::videomem::VideoMemory;
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

// Mem interface: VRAM
impl DSVideoMemory {
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
                ARM7::Lo => self.arm7_mem.lock().unwrap().c.take().unwrap(),
                ARM7::Hi => self.arm7_mem.lock().unwrap().d.take().unwrap(),
            },
            Slot::EngineA(slot) => {
                use EngineA::*;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                match slot {
                    Bg0    => engine_a.vram.bg_slot_0.take().unwrap(),
                    Bg01   => engine_a.vram.bg_slot_01.take().unwrap(),
                    Bg02   => engine_a.vram.bg_slot_02.take().unwrap(),
                    Bg03   => engine_a.vram.bg_slot_03.take().unwrap(),
                    Bg1    => engine_a.vram.bg_slot_1.take().unwrap(),
                    Bg2    => engine_a.vram.bg_slot_2.take().unwrap(),
                    Bg3    => engine_a.vram.bg_slot_3.take().unwrap(),
                
                    Obj0   => engine_a.vram.obj_slot_0.take().unwrap(),
                    Obj01  => engine_a.vram.obj_slot_01.take().unwrap(),
                    Obj02  => engine_a.vram.obj_slot_02.take().unwrap(),
                    Obj03  => engine_a.vram.obj_slot_03.take().unwrap(),
                    Obj1   => engine_a.vram.obj_slot_1.take().unwrap(),
                
                    BgExtPalette0 => engine_a.vram.ext_bg_palette_0.take().unwrap(),
                    BgExtPalette2 => engine_a.vram.ext_bg_palette_2.take().unwrap(),
                
                    ObjExtPalette => engine_a.vram.ext_obj_palette.take().unwrap()
                }
            },
            Slot::EngineB(slot) => {
                use EngineB::*;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                match slot {
                    Bg0    => engine_b.vram.bg_slot_0.take().unwrap(),
                    Bg01   => engine_b.vram.bg_slot_01.take().unwrap(),
                
                    Obj   => engine_b.vram.obj_slot.take().unwrap(),
                
                    BgExtPalette  => engine_b.vram.ext_bg_palette.take().unwrap(),
                    ObjExtPalette => engine_b.vram.ext_obj_palette.take().unwrap()
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
                ARM7::Lo => self.arm7_mem.lock().unwrap().c = Some(mem),
                ARM7::Hi => self.arm7_mem.lock().unwrap().d = Some(mem),
            },
            Slot::EngineA(slot) => {
                use EngineA::*;
                let mut engine_a = self.engine_a_mem.lock().unwrap();
                match slot {
                    Bg0    => engine_a.vram.bg_slot_0 = Some(mem),
                    Bg01   => engine_a.vram.bg_slot_01 = Some(mem),
                    Bg02   => engine_a.vram.bg_slot_02 = Some(mem),
                    Bg03   => engine_a.vram.bg_slot_03 = Some(mem),
                    Bg1    => engine_a.vram.bg_slot_1 = Some(mem),
                    Bg2    => engine_a.vram.bg_slot_2 = Some(mem),
                    Bg3    => engine_a.vram.bg_slot_3 = Some(mem),
                
                    Obj0   => engine_a.vram.obj_slot_0 = Some(mem),
                    Obj01  => engine_a.vram.obj_slot_01 = Some(mem),
                    Obj02  => engine_a.vram.obj_slot_02 = Some(mem),
                    Obj03  => engine_a.vram.obj_slot_03 = Some(mem),
                    Obj1   => engine_a.vram.obj_slot_1 = Some(mem),
                
                    BgExtPalette0 => engine_a.vram.ext_bg_palette_0 = Some(mem),
                    BgExtPalette2 => engine_a.vram.ext_bg_palette_2 = Some(mem),
                
                    ObjExtPalette => engine_a.vram.ext_obj_palette = Some(mem)
                }
            },
            Slot::EngineB(slot) => {
                use EngineB::*;
                let mut engine_b = self.engine_b_mem.lock().unwrap();
                match slot {
                    Bg0    => engine_b.vram.bg_slot_0 = Some(mem),
                    Bg01   => engine_b.vram.bg_slot_01 = Some(mem),
                
                    Obj   => engine_b.vram.obj_slot = Some(mem),
                
                    BgExtPalette  => engine_b.vram.ext_bg_palette = Some(mem),
                    ObjExtPalette => engine_b.vram.ext_obj_palette = Some(mem)
                }
            },
            Slot::Texture(_) => panic!("TEX unsupported right now"),
        }
    }

    fn set_a_cnt(&mut self, data: u8) {
        let from_slot = self.a_cnt.slot_ab(LCDC::A);
        let mem = self.get_mem(from_slot);
        self.a_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.a_cnt.slot_ab(LCDC::A);
        self.set_mem(to_slot, mem);
    }

    fn set_b_cnt(&mut self, data: u8) {
        let from_slot = self.b_cnt.slot_ab(LCDC::B);
        let mem = self.get_mem(from_slot);
        self.b_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.b_cnt.slot_ab(LCDC::B);
        self.set_mem(to_slot, mem);
    }

    fn set_c_cnt(&mut self, data: u8) {
        let from_slot = self.c_cnt.slot_c();
        let mem = self.get_mem(from_slot);
        self.c_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.c_cnt.slot_c();
        self.set_mem(to_slot, mem);
    }

    fn set_d_cnt(&mut self, data: u8) {
        let from_slot = self.d_cnt.slot_d();
        let mem = self.get_mem(from_slot);
        self.d_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.d_cnt.slot_d();
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
        let from_slot = self.f_cnt.slot_fg(LCDC::F);
        let mem = self.get_mem(from_slot);
        self.f_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.f_cnt.slot_fg(LCDC::F);
        self.set_mem(to_slot, mem);
    }

    fn set_g_cnt(&mut self, data: u8) {
        let from_slot = self.g_cnt.slot_fg(LCDC::G);
        let mem = self.get_mem(from_slot);
        self.g_cnt = VRAMControl::from_bits_truncate(data);
        let to_slot = self.g_cnt.slot_fg(LCDC::G);
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

    /// Get a reference to the relevant lcdc memory region.
    fn ref_lcdc_vram<'a>(&'a mut self, addr: u32) -> (Option<&'a mut Box<WRAM>>, u32) {
        match addr {
            0x0680_0000..=0x0681_FFFF => (self.vram.a.as_mut(), 0x0680_0000),
            0x0682_0000..=0x0683_FFFF => (self.vram.b.as_mut(), 0x0682_0000),
            0x0684_0000..=0x0685_FFFF => (self.vram.c.as_mut(), 0x0684_0000),
            0x0686_0000..=0x0687_FFFF => (self.vram.d.as_mut(), 0x0686_0000),
            0x0688_0000..=0x0688_FFFF => (self.vram.e.as_mut(), 0x0688_0000),
            0x0689_0000..=0x0689_3FFF => (self.vram.f.as_mut(), 0x0689_0000),
            0x0689_4000..=0x0689_7FFF => (self.vram.g.as_mut(), 0x0689_4000),
            0x0689_8000..=0x0689_FFFF => (self.vram.h.as_mut(), 0x0689_8000),
            0x068A_0000..=0x068A_3FFF => (self.vram.i.as_mut(), 0x068A_0000),
            _ => panic!("accessing LCDC image"),
        }
    }
}
