mod vram;
mod control;

use bitflags::bitflags;
use parking_lot::{Mutex, MutexGuard};
use std::sync::{
    atomic::{AtomicU16, Ordering},
    Arc
};
use crate::utils::{
    meminterface::MemInterface16,
    bits::{u8, u16}
};
use crate::common::mem::ram::RAM;
use crate::common::video::mem::VideoMemory;
use crate::ds::video::video3d::RenderingEngine;
use vram::ARM7VRAMSlots;
pub use vram::{ARM9VRAM, ARM7VRAM, EngineAVRAM, EngineBVRAM, Engine3DVRAM};
use control::*;

bitflags! {
    #[derive(Default)]
    pub struct GraphicsPowerControl: u16 {
        const DISPLAY_SWAP  = u16::bit(15);
        const ENABLE_B      = u16::bit(9);

        const GEOM_3D       = u16::bit(3);
        const RENDER_3D     = u16::bit(2);
        const ENABLE_A      = u16::bit(1);
        const ENABLE_LCD    = u16::bit(0);
    }
}

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

impl TryFrom<usize> for VRAMRegion {
    type Error = &'static str;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        use VRAMRegion::*;
        match value {
            0 => Ok(A),
            1 => Ok(B),
            2 => Ok(C),
            3 => Ok(D),
            4 => Ok(E),
            5 => Ok(F),
            6 => Ok(G),
            7 => Ok(H),
            8 => Ok(I),
            _ => Err("invalid")
        }
    }
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
    pub lcdc_vram:  Arc<Mutex<ARM9VRAM>>,

    mem_control:    [VRAMControlModule; 9],
    pub power_cnt:  Arc<AtomicU16>, // GraphicsPowerControl

    arm7_mem:           Arc<Mutex<ARM7VRAMSlots>>,
    pub engine_a_mem:   Arc<Mutex<VideoMemory<EngineAVRAM>>>,
    pub engine_b_mem:   Arc<Mutex<VideoMemory<EngineBVRAM>>>,
    engine_3d_mem:      Arc<Mutex<Engine3DVRAM>>,
}

impl DSVideoMemory {
    pub fn new(render_engine: Arc<Mutex<RenderingEngine>>) -> (Self, ARM7VRAM, RendererVRAM) {
        let lcdc_vram = Arc::new(Mutex::new(ARM9VRAM::new()));
        let arm7_vram = ARM7VRAM::default();
        let engine_a_mem = Arc::new(Mutex::new(VideoMemory::new(EngineAVRAM::default())));
        let engine_b_mem = Arc::new(Mutex::new(VideoMemory::new(EngineBVRAM::default())));
        let engine_3d_vram = Arc::new(Mutex::new(Engine3DVRAM::default()));

        let power_cnt = Arc::new(AtomicU16::new(0));

        (Self {
            lcdc_vram: lcdc_vram.clone(),

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
            power_cnt:  power_cnt.clone(),

            arm7_mem:       arm7_vram.mem.clone(),
            engine_a_mem:   engine_a_mem.clone(),
            engine_b_mem:   engine_b_mem.clone(),
            engine_3d_mem:  engine_3d_vram.clone()
        }, arm7_vram, RendererVRAM {
            lcdc_vram, power_cnt, engine_a_mem, engine_b_mem, engine_3d_vram, render_engine
        })
    }
}

impl DSVideoMemory {
    pub fn get_cnt(&self, region: VRAMRegion) -> u8 {
        self.mem_control[region as usize].cnt.bits()
    }

    pub fn set_cnt(&mut self, region: VRAMRegion, data: u8) {
        if self.mem_control[region as usize].cnt.bits() == data {
            return;
        }
        //println!("Set region {:?} => {:X}", region, data);
        // Get mem from current slot.
        let mem = self.swap_mem(self.mem_control[region as usize].slot, None);
        //println!("found {}", mem.is_some());
        let cnt = VRAMControl::from_bits_truncate(data);
        // Set mem in new slot.
        let to_slot = cnt.get_slot(region);
        //println!("move {:?} | {:?} => {:?}", region, self.mem_control[region as usize].slot, to_slot);
        self.mem_control[region as usize].cnt = cnt;
        self.mem_control[region as usize].slot = to_slot;
        let prev_mem = self.swap_mem(to_slot, mem);
        if prev_mem.is_some() {
            // There was already something in the slot.
            let old = self.lookup_at_slot(to_slot, region).unwrap();
            self.lcdc_vram.lock().lcdc[old] = prev_mem;
            self.mem_control[old].slot = Slot::LCDC(old.try_into().unwrap());
            //println!("writeback {:?} | => {:?}", old, self.mem_control[old].slot);
        }
    }
}

// Mem interface: VRAM
impl DSVideoMemory {

    fn lookup_vram<T, F: Fn(u32, &Box<RAM>) -> T>(&mut self, addr: u32, read_fn: F) -> Option<T> {
        match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let engine_a = self.engine_a_mem.lock();
                engine_a.vram.lookup_bg(addr)
                    .map(|(mask, vram)| read_fn(addr & mask, vram))
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let engine_b = self.engine_b_mem.lock();
                engine_b.vram.lookup_bg(addr)
                    .map(|(mask, vram)| read_fn(addr & mask, vram))
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let engine_a = self.engine_a_mem.lock();
                engine_a.vram.lookup_obj(addr)
                    .map(|(mask, vram)| read_fn(addr & mask, vram))
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let engine_b = self.engine_b_mem.lock();
                engine_b.vram.obj_slot.as_ref().map(|v| read_fn(addr & v.mask(), v))
            },
            _ => {
                let mut lcdc = self.lcdc_vram.lock();
                let (vram, offset) = lcdc.mut_lcdc(addr);
                vram.map(|v| read_fn(addr - offset, v))
            }
        }
    }

    fn lookup_vram_mut<F: Fn(u32, &mut Box<RAM>)>(&mut self, addr: u32, write_fn: F) {
        match addr {
            0x0600_0000..=0x061F_FFFF => {
                let addr = addr & 0x7_FFFF;
                let mut engine_a = self.engine_a_mem.lock();
                engine_a.vram.lookup_bg_mut(addr)
                    .map(|(mask, vram)| write_fn(addr & mask, vram));
            },
            0x0620_0000..=0x063F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock();
                engine_b.vram.lookup_bg_mut(addr)
                    .map(|(mask, vram)| write_fn(addr & mask, vram));
            },
            0x0640_0000..=0x065F_FFFF => {
                let addr = addr & 0x3_FFFF;
                let mut engine_a = self.engine_a_mem.lock();
                engine_a.vram.lookup_obj_mut(addr)
                    .map(|(mask, vram)| write_fn(addr & mask, vram));
            },
            0x0660_0000..=0x067F_FFFF => {
                let addr = addr & 0x1_FFFF;
                let mut engine_b = self.engine_b_mem.lock();
                engine_b.vram.obj_slot.as_mut().map(|v| write_fn(addr & v.mask(), v));
            },
            _ => {
                let mut lcdc = self.lcdc_vram.lock();
                let (vram, offset) = lcdc.mut_lcdc(addr);
                vram.map(|v| write_fn(addr - offset, v));
            }
        }
    }

    pub fn read_byte_vram(&mut self, addr: u32) -> u8 {
        self.lookup_vram(addr, |addr, vram| vram.read_byte(addr)).unwrap_or(0)
    }
    pub fn write_byte_vram(&mut self, addr: u32, data: u8) {
        self.lookup_vram_mut(addr, |addr, vram| vram.write_byte(addr, data));
    }

    pub fn read_halfword_vram(&mut self, addr: u32) -> u16 {
        self.lookup_vram(addr, |addr, vram| vram.read_halfword(addr)).unwrap_or(0)
    }
    pub fn write_halfword_vram(&mut self, addr: u32, data: u16) {
        self.lookup_vram_mut(addr, |addr, vram| vram.write_halfword(addr, data));
    }

    pub fn read_word_vram(&mut self, addr: u32) -> u32 {
        self.lookup_vram(addr, |addr, vram| vram.read_word(addr)).unwrap_or(0)
    }
    pub fn write_word_vram(&mut self, addr: u32, data: u32) {
        self.lookup_vram_mut(addr, |addr, vram| vram.write_word(addr, data));
    }
}

// Mem interface: engine memory
impl DSVideoMemory {
    pub fn mut_engine_a<'a>(&'a mut self) -> MutexGuard<'a, VideoMemory<EngineAVRAM>> {
        self.engine_a_mem.lock()
    }
    pub fn mut_engine_b<'a>(&'a mut self) -> MutexGuard<'a, VideoMemory<EngineBVRAM>> {
        self.engine_b_mem.lock()
    }

    pub fn read_byte_palette(&mut self, addr: u32) -> u8 {
        if addr < 0x400 {
            self.engine_a_mem.lock().palette.read_byte(addr)
        } else {
            self.engine_b_mem.lock().palette.read_byte(addr & 0x3FF)
        }
    }
    pub fn write_byte_palette(&mut self, addr: u32, data: u8) {
        if addr < 0x400 {
            self.engine_a_mem.lock().palette.write_byte(addr, data);
        } else {
            self.engine_b_mem.lock().palette.write_byte(addr & 0x3FF, data);
        }
    }

    pub fn read_halfword_palette(&mut self, addr: u32) -> u16 {
        if addr < 0x400 {
            self.engine_a_mem.lock().palette.read_halfword(addr)
        } else {
            self.engine_b_mem.lock().palette.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_palette(&mut self, addr: u32, data: u16) {
        if addr < 0x400 {
            self.engine_a_mem.lock().palette.write_halfword(addr, data);
        } else {
            self.engine_b_mem.lock().palette.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_palette(&mut self, addr: u32) -> u32 {
        if addr < 0x400 {
            self.engine_a_mem.lock().palette.read_word(addr)
        } else {
            self.engine_b_mem.lock().palette.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_palette(&mut self, addr: u32, data: u32) {
        if addr < 0x400 {
            self.engine_a_mem.lock().palette.write_word(addr, data);
        } else {
            self.engine_b_mem.lock().palette.write_word(addr & 0x3FF, data);
        }
    }

    pub fn read_byte_oam(&mut self, addr: u32) -> u8 {
        if addr < 0x400 {
            self.engine_a_mem.lock().oam.read_byte(addr)
        } else {
            self.engine_b_mem.lock().oam.read_byte(addr & 0x3FF)
        }
    }
    pub fn write_byte_oam(&mut self, addr: u32, data: u8) {
        if addr < 0x400 {
            self.engine_a_mem.lock().oam.write_byte(addr, data);
        } else {
            self.engine_b_mem.lock().oam.write_byte(addr & 0x3FF, data);
        }
    }

    pub fn read_halfword_oam(&mut self, addr: u32) -> u16 {
        if addr < 0x400 {
            self.engine_a_mem.lock().oam.read_halfword(addr)
        } else {
            self.engine_b_mem.lock().oam.read_halfword(addr & 0x3FF)
        }
    }
    pub fn write_halfword_oam(&mut self, addr: u32, data: u16) {
        if addr < 0x400 {
            self.engine_a_mem.lock().oam.write_halfword(addr, data);
        } else {
            self.engine_b_mem.lock().oam.write_halfword(addr & 0x3FF, data);
        }
    }

    pub fn read_word_oam(&mut self, addr: u32) -> u32 {
        if addr < 0x400 {
            self.engine_a_mem.lock().oam.read_word(addr)
        } else {
            self.engine_b_mem.lock().oam.read_word(addr & 0x3FF)
        }
    }
    pub fn write_word_oam(&mut self, addr: u32, data: u32) {
        if addr < 0x400 {
            self.engine_a_mem.lock().oam.write_word(addr, data);
        } else {
            self.engine_b_mem.lock().oam.write_word(addr & 0x3FF, data);
        }
    }
}

impl DSVideoMemory {
    fn swap_mem(&mut self, from_slot: Slot, new: Option<Box<RAM>>) -> Option<Box<RAM>> {
        match from_slot {
            Slot::LCDC(lcdc) => {
                let mut vram = self.lcdc_vram.lock();
                std::mem::replace(&mut vram.lcdc[lcdc as usize], new)
            },
            Slot::ARM7(arm7) => match arm7 {
                ARM7::Lo => std::mem::replace(&mut self.arm7_mem.lock().c, new),
                ARM7::Hi => std::mem::replace(&mut self.arm7_mem.lock().d, new),
            },
            Slot::EngineA(slot) => {
                use EngineA::*;
                let mut engine_a = self.engine_a_mem.lock();
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
                let mut engine_b = self.engine_b_mem.lock();
                match slot {
                    Bg0    => std::mem::replace(&mut engine_b.vram.bg_slot_0, new),
                    Bg01   => std::mem::replace(&mut engine_b.vram.bg_slot_01, new),
                
                    Obj   => std::mem::replace(&mut engine_b.vram.obj_slot, new),
                
                    BgExtPalette  => {
                        engine_b.vram.ext_bg_palette_dirty = true;
                        std::mem::replace(&mut engine_b.vram.ext_bg_palette, new)
                    },
                    ObjExtPalette => {
                        engine_b.vram.ext_obj_palette_dirty = true;
                        std::mem::replace(&mut engine_b.vram.ext_obj_palette, new)
                    }
                }
            },
            Slot::Texture(slot) => {
                use Texture::*;
                let mut engine_3d = self.engine_3d_mem.lock();
                match slot {
                    Tex0 => std::mem::replace(&mut engine_3d.tex_0, new),
                    Tex1 => std::mem::replace(&mut engine_3d.tex_1, new),
                    Tex2 => std::mem::replace(&mut engine_3d.tex_2, new),
                    Tex3 => std::mem::replace(&mut engine_3d.tex_3, new),
                
                    Palette0 => {
                        engine_3d.tex_palette_dirty = true;
                        std::mem::replace(&mut engine_3d.tex_palette_0, new)
                    },
                    Palette1 => {
                        engine_3d.tex_palette_dirty = true;
                        std::mem::replace(&mut engine_3d.tex_palette_1, new)
                    },
                    Palette4 => {
                        engine_3d.tex_palette_dirty = true;
                        std::mem::replace(&mut engine_3d.tex_palette_4, new)
                    },
                    Palette5 => {
                        engine_3d.tex_palette_dirty = true;
                        std::mem::replace(&mut engine_3d.tex_palette_5, new)
                    },
                }
            }
        }
    }

    /// Find which VRAM region is at slot
    fn lookup_at_slot(&mut self, slot: Slot, except: VRAMRegion) -> Option<usize> {
        for (n, region) in self.mem_control.iter().enumerate()
            .filter(|(n, _)| *n != (except as usize))
        {
            if region.slot == slot {
                return Some(n);
            }
        }
        None
    }
}

/// Renderer-side access to VRAM.
pub struct RendererVRAM {
    pub lcdc_vram:      Arc<Mutex<ARM9VRAM>>,
    pub power_cnt:      Arc<AtomicU16>, // GraphicsPowerControl
    pub engine_a_mem:   Arc<Mutex<VideoMemory<EngineAVRAM>>>,
    pub engine_b_mem:   Arc<Mutex<VideoMemory<EngineBVRAM>>>,
    pub engine_3d_vram: Arc<Mutex<Engine3DVRAM>>,
    pub render_engine:  Arc<Mutex<RenderingEngine>>
}

impl RendererVRAM {
    pub fn read_power_cnt(&self) -> GraphicsPowerControl {
        GraphicsPowerControl::from_bits_truncate(self.power_cnt.load(Ordering::Acquire))
    }
}