
use bitflags::bitflags;
use parking_lot::Mutex;
use std::sync::Arc;
use crate::utils::{
    meminterface::MemInterface16,
    bits::u8
};
use crate::common::{
    videomem::VRAM2D,
    wram::WRAM
};
use super::VRAMRegion;

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
    pub lcdc:   [VRAMSlot; 9]
}

type VRAMSlot = Option<Box<WRAM>>; // TODO: does this need to be boxed even?

impl ARM9VRAM {
    pub fn new() -> Self {
        Self {
            lcdc: [
                Some(Box::new(WRAM::new(128 * 1024))),
                Some(Box::new(WRAM::new(128 * 1024))),
                Some(Box::new(WRAM::new(128 * 1024))),
                Some(Box::new(WRAM::new(128 * 1024))),
                Some(Box::new(WRAM::new(64 * 1024))),
                Some(Box::new(WRAM::new(16 * 1024))),
                Some(Box::new(WRAM::new(16 * 1024))),
                Some(Box::new(WRAM::new(32 * 1024))),
                Some(Box::new(WRAM::new(16 * 1024))),
            ]

            //arm7_status:    Arc::new(AtomicU8::new(VRAMStatus::default().bits()))
        }
    }

    /// Get a mutable reference to the relevant lcdc memory region.
    pub fn mut_lcdc<'a>(&'a mut self, addr: u32) -> (Option<&'a mut Box<WRAM>>, u32) {
        match addr {
            0x0680_0000..=0x0681_FFFF => (self.lcdc[VRAMRegion::A as usize].as_mut(), 0x0680_0000),
            0x0682_0000..=0x0683_FFFF => (self.lcdc[VRAMRegion::B as usize].as_mut(), 0x0682_0000),
            0x0684_0000..=0x0685_FFFF => (self.lcdc[VRAMRegion::C as usize].as_mut(), 0x0684_0000),
            0x0686_0000..=0x0687_FFFF => (self.lcdc[VRAMRegion::D as usize].as_mut(), 0x0686_0000),
            0x0688_0000..=0x0688_FFFF => (self.lcdc[VRAMRegion::E as usize].as_mut(), 0x0688_0000),
            0x0689_0000..=0x0689_3FFF => (self.lcdc[VRAMRegion::F as usize].as_mut(), 0x0689_0000),
            0x0689_4000..=0x0689_7FFF => (self.lcdc[VRAMRegion::G as usize].as_mut(), 0x0689_4000),
            0x0689_8000..=0x0689_FFFF => (self.lcdc[VRAMRegion::H as usize].as_mut(), 0x0689_8000),
            0x068A_0000..=0x068A_3FFF => (self.lcdc[VRAMRegion::I as usize].as_mut(), 0x068A_0000),
            _ => panic!("accessing LCDC image {:X}", addr),
        }
    }

    /// Immutably reference a region of VRAM mapped to LCDC.
    /// Supports A-D.
    pub fn ref_region<'a>(&'a self, region: u16) -> Option<&'a Box<WRAM>> {
        self.lcdc[region as usize].as_ref()
    }

    /// Immutably reference a region of VRAM mapped to LCDC.
    /// Supports A-D.
    pub fn mut_region<'a>(&'a mut self, region: u16) -> Option<&'a mut Box<WRAM>> {
        self.lcdc[region as usize].as_mut()
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
        let slots = self.mem.lock();
        let mut status = VRAMStatus::empty();
        status.set(VRAMStatus::VRAM_C, slots.c.is_some());
        status.set(VRAMStatus::VRAM_D, slots.d.is_some());
        status.bits()
    }
}

impl MemInterface16 for ARM7VRAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let slots = self.mem.lock();
        match addr {
            0x0600_0000..=0x0601_FFFF => match slots.c.as_ref() {
                Some(vram) => vram.read_halfword(addr - 0x0600_0000),
                None => 0,
            },
            0x0602_0000..=0x0603_FFFF => match slots.d.as_ref() {
                Some(vram) => vram.read_halfword(addr - 0x0602_0000),
                None => 0,
            },
            _ => panic!("reading from weird VRAM addr (ARM7: {:X})", addr),
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let mut slots = self.mem.lock();
        match addr {
            0x0600_0000..=0x0601_FFFF => match slots.c.as_mut() {
                Some(vram) => vram.write_halfword(addr - 0x0600_0000, data),
                None => {},
            },
            0x0602_0000..=0x0603_FFFF => match slots.d.as_mut() {
                Some(vram) => vram.write_halfword(addr - 0x0602_0000, data),
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

    pub ext_bg_palette_dirty:   bool,
    pub ext_obj_palette_dirty:  bool,
}

impl EngineAVRAM {
    /// Get the VRAM slot for the BG addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_bg<'a>(&'a self, addr: u32) -> Option<(u32, &'a Box<WRAM>)> {
        match addr {
            0x4000..=0x7FFF => match self.bg_slot_01.as_ref() {
                None => self.bg_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x1_0000..=0x1_3FFF => match self.bg_slot_02.as_ref() {
                None => self.bg_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x1_4000..=0x1_7FFF => match self.bg_slot_03.as_ref() {
                None => self.bg_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x0..=0x1_FFFF => self.bg_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
            0x2_0000..=0x3_FFFF => self.bg_slot_1.as_ref().map(|vram| (0x1_FFFF, vram)),
            0x4_0000..=0x5_FFFF => self.bg_slot_2.as_ref().map(|vram| (0x1_FFFF, vram)),
            0x6_0000..=0x7_FFFF => self.bg_slot_3.as_ref().map(|vram| (0x1_FFFF, vram)),
            _ => None
        }
    }

    /// Get the VRAM slot for the BG addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_bg_mut<'a>(&'a mut self, addr: u32) -> Option<(u32, &'a mut Box<WRAM>)> {
        match addr {
            0x4000..=0x7FFF if self.bg_slot_01.is_some() => self.bg_slot_01.as_mut().map(|vram| (0x3FFF, vram)),
            0x1_0000..=0x1_3FFF if self.bg_slot_02.is_some() => self.bg_slot_02.as_mut().map(|vram| (0x3FFF, vram)),
            0x1_4000..=0x1_7FFF if self.bg_slot_03.is_some() => self.bg_slot_03.as_mut().map(|vram| (0x3FFF, vram)),
            0x0..=0x1_FFFF => self.bg_slot_0.as_mut().map(|vram| (vram.mask(), vram)),
            0x2_0000..=0x3_FFFF => self.bg_slot_1.as_mut().map(|vram| (0x1_FFFF, vram)),
            0x4_0000..=0x5_FFFF => self.bg_slot_2.as_mut().map(|vram| (0x1_FFFF, vram)),
            0x6_0000..=0x7_FFFF => self.bg_slot_3.as_mut().map(|vram| (0x1_FFFF, vram)),
            _ => None
        }
    }

    /// Get the VRAM slot for the OBJ addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_obj<'a>(&'a self, addr: u32) -> Option<(u32, &'a Box<WRAM>)> {
        match addr {
            0x4000..=0x7FFF => match self.obj_slot_01.as_ref() {
                None => self.obj_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x1_0000..=0x1_3FFF => match self.obj_slot_02.as_ref() {
                None => self.obj_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x1_4000..=0x1_7FFF => match self.obj_slot_03.as_ref() {
                None => self.obj_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x0..=0x1_FFFF => self.obj_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
            0x2_0000..=0x3_FFFF => self.obj_slot_1.as_ref().map(|vram| (0x1_FFFF, vram)),
            _ => None
        }
    }

    /// Get the VRAM slot for the OBJ addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_obj_mut<'a>(&'a mut self, addr: u32) -> Option<(u32, &'a mut Box<WRAM>)> {
        match addr {
            0x4000..=0x7FFF if self.obj_slot_01.is_some() => self.obj_slot_01.as_mut().map(|vram| (0x3FFF, vram)),
            0x1_0000..=0x1_3FFF if self.obj_slot_02.is_some() => self.obj_slot_02.as_mut().map(|vram| (0x3FFF, vram)),
            0x1_4000..=0x1_7FFF if self.obj_slot_03.is_some() => self.obj_slot_03.as_mut().map(|vram| (0x3FFF, vram)),
            0x0..=0x1_FFFF => self.obj_slot_0.as_mut().map(|vram| (vram.mask(), vram)),
            0x2_0000..=0x3_FFFF => self.obj_slot_1.as_mut().map(|vram| (0x1_FFFF, vram)),
            _ => None
        }
    }
}

impl VRAM2D for EngineAVRAM {
    fn get_bg_byte(&self, addr: u32) -> u8 {
        if let Some((mask, vram)) = self.lookup_bg(addr) {
            vram.read_byte(addr & mask)
        } else {
            //panic!("reading from strange addr (ENG_A_BG: {:X})", addr)
            0
        }
    }

    fn get_bg_halfword(&self, addr: u32) -> u16 {
        if let Some((mask, vram)) = self.lookup_bg(addr) {
            vram.read_halfword(addr & mask)
        } else {
            //panic!("reading from strange addr (ENG_A_BG: {:X})", addr)
            0
        }
    }

    fn get_obj_byte(&self, addr: u32) -> u8 {
        if let Some((mask, vram)) = self.lookup_obj(addr) {
            vram.read_byte(addr & mask)
        } else {
            //panic!("reading from strange addr (ENG_A_OBJ: {:X})", addr)
            0
        }
    }

    fn get_obj_halfword(&self, addr: u32) -> u16 {
        if let Some((mask, vram)) = self.lookup_obj(addr) {
            vram.read_halfword(addr & mask)
        } else {
            //panic!("reading from strange addr (ENG_A_OBJ: {:X})", addr)
            0
        }
    }

    fn ref_ext_bg_palette<'a>(&'a mut self) -> [Option<&'a [u8]>; 4] {
        if self.ext_bg_palette_dirty {
            self.ext_bg_palette_dirty = false;
            [
                self.ext_bg_palette_0.as_ref().map(|v| &v.ref_mem()[0..0x2000]),
                self.ext_bg_palette_0.as_ref().map(|v| &v.ref_mem()[0x2000..0x4000]),
                if self.ext_bg_palette_2.is_some() {
                    self.ext_bg_palette_2.as_ref().map(|v| &v.ref_mem()[0..0x2000])
                } else {
                    self.ext_bg_palette_0.as_ref().map(|v| &v.ref_mem()[0x4000..0x6000])
                },
                if self.ext_bg_palette_2.is_some() {
                    self.ext_bg_palette_2.as_ref().map(|v| &v.ref_mem()[0x2000..0x4000])
                } else {
                    self.ext_bg_palette_0.as_ref().map(|v| &v.ref_mem()[0x6000..0x8000])
                }
            ]
        } else {
            [None; 4]
        }
    }

    fn ref_ext_obj_palette<'a>(&'a mut self) -> Option<&'a [u8]> {
        if self.ext_obj_palette_dirty {
            self.ext_obj_palette_dirty = false;
            self.ext_obj_palette.as_ref().map(|v| &v.ref_mem()[0..0x2000])
        } else {
            None
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

    pub ext_bg_palette_dirty:   bool,
    pub ext_obj_palette_dirty:  bool,
}

impl EngineBVRAM {
    /// Get the VRAM slot for the BG addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_bg<'a>(&'a self, addr: u32) -> Option<(u32, &'a Box<WRAM>)> {
        match addr {
            0x8000..=0xBFFF => match self.bg_slot_01.as_ref() {
                None => self.bg_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
                Some(vram) => Some((0x3FFF, vram))
            },
            0x0..=0x1_FFFF => self.bg_slot_0.as_ref().map(|vram| (vram.mask(), vram)),
            _ => None
        }
    }

    /// Get the VRAM slot for the BG addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_bg_mut<'a>(&'a mut self, addr: u32) -> Option<(u32, &'a mut Box<WRAM>)> {
        match addr {
            0x8000..=0xBFFF if self.bg_slot_01.is_some() => self.bg_slot_01.as_mut().map(|vram| (0x3FFF, vram)),
            0x0..=0x1_FFFF => self.bg_slot_0.as_mut().map(|vram| (vram.mask(), vram)),
            _ => None
        }
    }
}

impl VRAM2D for EngineBVRAM {
    fn get_bg_byte(&self, addr: u32) -> u8 {
        if let Some((mask, vram)) = self.lookup_bg(addr) {
            vram.read_byte(addr & mask)
        } else {
            //panic!("reading from strange addr (ENG_B_BG: {:X})", addr)
            0
        }
    }

    fn get_bg_halfword(&self, addr: u32) -> u16 {
        if let Some((mask, vram)) = self.lookup_bg(addr) {
            vram.read_halfword(addr & mask)
        } else {
            //panic!("reading from strange addr (ENG_B_BG: {:X})", addr)
            0
        }
    }

    fn get_obj_byte(&self, addr: u32) -> u8 {
        self.obj_slot.as_ref().map(|v| v.read_byte(addr & v.mask())).unwrap_or(0)
    }

    fn get_obj_halfword(&self, addr: u32) -> u16 {
        self.obj_slot.as_ref().map(|v| v.read_halfword(addr & v.mask())).unwrap_or(0)
    }

    fn ref_ext_bg_palette<'a>(&'a mut self) -> [Option<&'a [u8]>; 4] {
        if self.ext_bg_palette_dirty {
            self.ext_bg_palette_dirty = false;
            [
                self.ext_bg_palette.as_ref().map(|v| &v.ref_mem()[0..0x2000]),
                self.ext_bg_palette.as_ref().map(|v| &v.ref_mem()[0x2000..0x4000]),
                self.ext_bg_palette.as_ref().map(|v| &v.ref_mem()[0x4000..0x6000]),
                self.ext_bg_palette.as_ref().map(|v| &v.ref_mem()[0x6000..0x8000])
            ]
        } else {
            [None; 4]
        }
    }

    fn ref_ext_obj_palette<'a>(&'a mut self) -> Option<&'a [u8]> {
        if self.ext_obj_palette_dirty {
            self.ext_obj_palette_dirty = false;
            self.ext_obj_palette.as_ref().map(|v| &v.ref_mem()[0..0x2000])
        } else {
            None
        }
    }
}


#[derive(Default)]
pub struct Engine3DVRAM {
    pub tex_0:  VRAMSlot,
    pub tex_1:  VRAMSlot,
    /// This slot is used for clear images (+alpha).
    pub tex_2:  VRAMSlot,
    /// This slot is used for clear depth (+fog).
    pub tex_3:  VRAMSlot,

    pub tex_palette_0:  VRAMSlot,
    pub tex_palette_1:  VRAMSlot,
    pub tex_palette_4:  VRAMSlot,
    pub tex_palette_5:  VRAMSlot,
}

impl Engine3DVRAM {
    /// Get the VRAM slot for the texture addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_tex<'a>(&'a self, addr: u32) -> Option<(u32, &'a Box<WRAM>)> {
        match addr {
            0x0_0000..=0x1_FFFF => self.tex_0.as_ref().map(|vram| (0x1_FFFF, vram)),
            0x2_0000..=0x3_FFFF => self.tex_1.as_ref().map(|vram| (0x1_FFFF, vram)),
            0x4_0000..=0x5_FFFF => self.tex_2.as_ref().map(|vram| (0x1_FFFF, vram)),
            0x6_0000..=0x7_FFFF => self.tex_3.as_ref().map(|vram| (0x1_FFFF, vram)),
            _ => None
        }
    }

    /// Get the VRAM slot for the palette addr provided.
    /// 
    /// Returns the ram and a mask to use for the address.
    pub fn lookup_tex_palette<'a>(&'a self, addr: u32) -> Option<(u32, &'a Box<WRAM>)> {
        match addr {
            0x0_4000..=0x0_7FFF => match self.tex_palette_1.as_ref() {
                None => self.tex_palette_0.as_ref().map(|vram| (vram.mask(), vram)),    // TODO: what if a 16kB VRAM block is mapped to 0, and nothing to 1?
                Some(vram) => Some((0x3FFF, vram))
            },
            0x1_0000..=0x1_3FFF => self.tex_palette_4.as_ref().map(|vram| (0x3FFF, vram)),
            0x1_4000..=0x1_7FFF => self.tex_palette_5.as_ref().map(|vram| (0x3FFF, vram)),
            0x0_0000..=0x0_FFFF => self.tex_palette_0.as_ref().map(|vram| (vram.mask(), vram)),
            _ => None
        }
    }
}

