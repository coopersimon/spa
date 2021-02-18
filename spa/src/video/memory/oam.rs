/// Object attribute memory (for sprites)

use bitflags::bitflags;
use fixed::types::I24F8;
use crate::common::{
    bits::u16,
    meminterface::MemInterface16
};

/// Parameters used for affine coords.
pub struct ObjAffineParams {
    pub pa: I24F8,
    pub pb: I24F8,
    pub pc: I24F8,
    pub pd: I24F8,
}

/// Object attribute memory.
pub struct OAM {
    objects:    Vec<ObjAttrs>
}

impl OAM {
    pub fn new() -> Self {
        Self {
            objects:    vec![ObjAttrs::new(); 128]
        }
    }

    pub fn ref_objects<'a>(&'a self) -> &'a [ObjAttrs] {
        &self.objects
    }

    pub fn affine_params(&self, param_num: u16) -> ObjAffineParams {
        let offset = (param_num as usize) * 4;
        ObjAffineParams {
            pa: I24F8::from_bits(self.objects[offset].affine_param as i16 as i32),
            pb: I24F8::from_bits(self.objects[offset + 1].affine_param as i16 as i32),
            pc: I24F8::from_bits(self.objects[offset + 2].affine_param as i16 as i32),
            pd: I24F8::from_bits(self.objects[offset + 3].affine_param as i16 as i32),
        }
    }
}

impl MemInterface16 for OAM {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let obj = (addr >> 3) as usize;
        let index = (addr >> 1) & 0x3;
        match index {
            0 => self.objects[obj].attrs_0.bits(),
            1 => self.objects[obj].attrs_1.bits(),
            2 => self.objects[obj].attrs_2.bits(),
            3 => self.objects[obj].affine_param,
            _ => unreachable!()
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        let obj = (addr >> 3) as usize;
        let index = (addr >> 1) & 0x3;
        match index {
            0 => self.objects[obj].attrs_0 = ObjAttr0::from_bits_truncate(data),
            1 => self.objects[obj].attrs_1 = ObjAttr1::from_bits_truncate(data),
            2 => self.objects[obj].attrs_2 = ObjAttr2::from_bits_truncate(data),
            3 => self.objects[obj].affine_param = data,
            _ => unreachable!()
        }
    }
}


bitflags!{
    #[derive(Default)]
    struct ObjAttr0: u16 {
        const SHAPE         = u16::bits(14, 15);
        const USE_8_BPP     = u16::bit(13);
        const MOSAIC        = u16::bit(12);
        const OBJ_MODE      = u16::bits(10, 11);
        const DISABLE       = u16::bit(9);
        const DOUBLE_CLIP   = u16::bit(9);
        const AFFINE        = u16::bit(8);
        const Y_COORD       = u16::bits(0, 7);
    }
}

bitflags!{
    #[derive(Default)]
    struct ObjAttr1: u16 {
        const SIZE          = u16::bits(14, 15);
        const V_FLIP        = u16::bit(13);
        const H_FLIP        = u16::bit(12);
        const AFFINE_PARAMS = u16::bits(9, 13);
        const X_COORD       = u16::bits(0, 8);
    }
}

bitflags!{
    #[derive(Default)]
    struct ObjAttr2: u16 {
        const PALETTE   = u16::bits(12, 15);
        const PRIORITY  = u16::bits(10, 11);
        const TILE_NUM  = u16::bits(0, 9);
    }
}

/// A single obj attribute, + one OAM parameter
#[derive(Clone)]
pub struct ObjAttrs {
    attrs_0:        ObjAttr0,
    attrs_1:        ObjAttr1,
    attrs_2:        ObjAttr2,
    affine_param:   u16,
}

impl ObjAttrs {
    pub fn new() -> Self {
        Self {
            attrs_0:        ObjAttr0::default(),
            attrs_1:        ObjAttr1::default(),
            attrs_2:        ObjAttr2::default(),
            affine_param:   0,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.attrs_0.contains(ObjAttr0::AFFINE) || !self.attrs_0.contains(ObjAttr0::DISABLE)
    }

    /// Get the affine param num if the object is affine transformed.
    /// 
    /// If it is not, None will be returned.
    pub fn affine_param_num(&self) -> Option<u16> {
        if self.attrs_0.contains(ObjAttr0::AFFINE) {
            Some((self.attrs_1 & ObjAttr1::AFFINE_PARAMS).bits() >> 9)
        } else {
            None
        }
    }

    pub fn h_flip(&self) -> bool {
        self.attrs_1.contains(ObjAttr1::H_FLIP)
    }

    pub fn v_flip(&self) -> bool {
        self.attrs_1.contains(ObjAttr1::V_FLIP)
    }

    pub fn priority(&self) -> u8 {
        ((self.attrs_2 & ObjAttr2::PRIORITY).bits() >> 10) as u8
    }

    /// Get top-left corner of object.
    pub fn coords(&self) -> (i16, i16) {
        let y = ((self.attrs_0 & ObjAttr0::Y_COORD).bits() as i8) as i16;
        let mut x = (self.attrs_1 & ObjAttr1::X_COORD).bits();
        if u16::test_bit(x, 8) {
            x |= 0xFE00;
        }
        (x as i16, y)
    }

    pub fn source_size(&self) -> (u8, u8) {
        const SHAPE_SQ: u16     = 0 << 14;
        const SHAPE_HOR: u16    = 1 << 14;
        const SHAPE_VER: u16    = 2 << 14;
        const SHAPE_NONE: u16   = 3 << 14;
        const SIZE_SMALL: u16   = 0 << 14;
        const SIZE_MID: u16     = 1 << 14;
        const SIZE_LARGE: u16   = 2 << 14;
        const SIZE_XLARGE: u16  = 3 << 14;

        match (self.attrs_0 & ObjAttr0::SHAPE).bits() {
            SHAPE_SQ => match (self.attrs_1 & ObjAttr1::SIZE).bits() {
                SIZE_SMALL  => (8, 8),
                SIZE_MID    => (16, 16),
                SIZE_LARGE  => (32, 32),
                SIZE_XLARGE => (64, 64),
                _ => unreachable!(),
            },
            SHAPE_HOR => match (self.attrs_1 & ObjAttr1::SIZE).bits() {
                SIZE_SMALL  => (16, 8),
                SIZE_MID    => (32, 8),
                SIZE_LARGE  => (32, 16),
                SIZE_XLARGE => (64, 32),
                _ => unreachable!(),
            },
            SHAPE_VER => match (self.attrs_1 & ObjAttr1::SIZE).bits() {
                SIZE_SMALL  => (8, 16),
                SIZE_MID    => (8, 32),
                SIZE_LARGE  => (16, 32),
                SIZE_XLARGE => (32, 64),
                _ => unreachable!(),
            },
            SHAPE_NONE => (0, 0),
            _ => unreachable!(),
        }
    }

    /// Get size of object clipping window.
    pub fn size(&self) -> (i16, i16) {
        let double_size = self.attrs_0.contains(ObjAttr0::AFFINE | ObjAttr0::DOUBLE_CLIP);
        let shift = if double_size {1} else {0};
        let base_size = self.source_size();
        ((base_size.0 as i16) << shift, (base_size.1 as i16) << shift)
    }

    /// Get the palette bank for the object.
    /// If "None" is returned, the object uses 8bpp tiles.
    pub fn palette_bank(&self) -> Option<u8> {
        if self.attrs_0.contains(ObjAttr0::USE_8_BPP) {
            None
        } else {
            let palette = (self.attrs_2 & ObjAttr2::PALETTE).bits() >> 12;
            Some(palette as u8)
        }
    }

    /// Get the sprite tile num to use.
    pub fn tile_num(&self) -> u32 {
        (self.attrs_2 & ObjAttr2::TILE_NUM).bits() as u32
    }

    pub fn is_semi_transparent(&self) -> bool {
        const SEMI_TRANS: u16 = 1 << 10;
        (self.attrs_0 & ObjAttr0::OBJ_MODE).bits() == SEMI_TRANS
    }

    pub fn is_obj_window(&self) -> bool {
        const OBJ_WINDOW: u16 = 2 << 10;
        (self.attrs_0 & ObjAttr0::OBJ_MODE).bits() == OBJ_WINDOW
    }
}