/// Object attribute memory (for sprites)

use bitflags::bitflags;
use crate::common::{
    bits::u16,
    meminterface::MemInterface16
};

pub struct OAM {
    objects:    Vec<ObjAttrs>
}

impl OAM {
    pub fn new() -> Self {
        Self {
            objects:    vec![ObjAttrs::new(); 128]
        }
    }
}

impl MemInterface16 for OAM {
    fn read_halfword(&self, addr: u32) -> u16 {
        let obj = (addr >> 2) as usize;
        let index = addr & 0x3;
        match index {
            0 => self.objects[obj].attrs_0.bits(),
            1 => self.objects[obj].attrs_1.bits(),
            2 => self.objects[obj].attrs_2.bits(),
            3 => self.objects[obj].rot_scale_param,
            _ => unreachable!()
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let obj = (addr >> 2) as usize;
        let index = addr & 0x3;
        match index {
            0 => self.objects[obj].attrs_0 = ObjAttr0::from_bits_truncate(data),
            1 => self.objects[obj].attrs_1 = ObjAttr1::from_bits_truncate(data),
            2 => self.objects[obj].attrs_2 = ObjAttr2::from_bits_truncate(data),
            3 => self.objects[obj].rot_scale_param = data,
            _ => unreachable!()
        }
    }
}


bitflags!{
    #[derive(Default)]
    pub struct ObjAttr0: u16 {
        const SHAPE     = u16::bits(14, 15);
        const BPP       = u16::bit(13);
        const MOSAIC    = u16::bit(12);
        const OBJ_MODE  = u16::bits(10, 11);
        const ENABLE    = u16::bit(9);
        const DBL_SIZE  = u16::bit(9);
        const ROT_SCALE = u16::bit(8);
        const Y_COORD   = u16::bits(0, 7);
    }
}

bitflags!{
    #[derive(Default)]
    pub struct ObjAttr1: u16 {
        const SIZE      = u16::bits(14, 15);
        const V_FLIP    = u16::bit(13);
        const H_FLIP    = u16::bit(12);
        const PARAMS    = u16::bits(9, 13);
        const X_COORD   = u16::bits(0, 8);
    }
}

bitflags!{
    #[derive(Default)]
    pub struct ObjAttr2: u16 {
        const PALETTE   = u16::bits(12, 15);
        const PRIORITY  = u16::bits(10, 11);
        const TILE_NUM  = u16::bits(0, 9);
    }
}

/// A single obj attribute, + one OAM parameter
#[derive(Clone)]
pub struct ObjAttrs {
    attrs_0:            ObjAttr0,
    attrs_1:            ObjAttr1,
    attrs_2:            ObjAttr2,
    rot_scale_param:    u16,
}

impl ObjAttrs {
    pub fn new() -> Self {
        Self {
            attrs_0:            ObjAttr0::default(),
            attrs_1:            ObjAttr1::default(),
            attrs_2:            ObjAttr2::default(),
            rot_scale_param:    0,
        }
    }
}