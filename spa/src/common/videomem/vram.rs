
use bitflags::bitflags;
use crate::utils::bits::{u8, u16};

bitflags! {
    #[derive(Default)]
    /// Tile map attributes.
    pub struct TileMapAttrs: u16 {
        const PALETTE   = u16::bits(12, 15);
        const V_FLIP    = u16::bit(11);
        const H_FLIP    = u16::bit(10);
        const TILE_NUM  = u16::bits(0, 9);
    }
}

impl TileMapAttrs {
    pub fn tile_num(self) -> u32 {
        (self & TileMapAttrs::TILE_NUM).bits() as u32
    }

    pub fn h_flip(self) -> bool {
        self.contains(TileMapAttrs::H_FLIP)
    }

    pub fn v_flip(self) -> bool {
        self.contains(TileMapAttrs::V_FLIP)
    }

    pub fn palette_num(self) -> u8 {
        ((self & TileMapAttrs::PALETTE).bits() >> 12) as u8
    }
}

/// VRAM for 2D rendering.
/// Contains tile data, background maps, and bitmaps.
pub trait VRAM2D {

    /// Read a byte from background VRAM.
    fn get_bg_byte(&self, addr: u32) -> u8;

    /// Read a byte from object VRAM.
    fn get_obj_byte(&self, addr: u32) -> u8;

    /// Read a halfword from background VRAM.
    fn get_bg_halfword(&self, addr: u32) -> u16;

    /// Get a set of tile map attributes for a regular background.
    fn tile_map_attrs(&self, addr: u32) -> TileMapAttrs {
        TileMapAttrs::from_bits_truncate(self.get_bg_halfword(addr))
    }

    /// Get the tile number for an affine background.
    fn affine_map_tile_num(&self, addr: u32) -> u32 {
        self.get_bg_byte(addr) as u32
    }

    /// Get a texel for a particular background tile, using 16-colour palette.
    fn bg_tile_texel_4bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (4 * y) as u32;
        let x_offset = (x / 2) as u32;
        let data = self.get_bg_byte(addr + y_offset + x_offset);
        if u8::test_bit(x, 0) {
            data >> 4
        } else {
            data & 0xF
        }
    }

    /// Get a texel for a particular background tile, using 256-colour palette.
    fn bg_tile_texel_8bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (8 * y) as u32;
        let x_offset = x as u32;
        self.get_bg_byte(addr + y_offset + x_offset)
    }

    /// Get a texel for a particular object tile, using 16-colour palette.
    fn obj_tile_texel_4bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (4 * y) as u32;
        let x_offset = (x / 2) as u32;
        let data = self.get_obj_byte(addr + y_offset + x_offset);
        if u8::test_bit(x, 0) {
            data >> 4
        } else {
            data & 0xF
        }
    }

    /// Get a texel for a particular object tile, using 256-colour palette.
    fn obj_tile_texel_8bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (8 * y) as u32;
        let x_offset = x as u32;
        self.get_obj_byte(addr + y_offset + x_offset)
    }

    /// Get a bitmap texel, using 256-colour palette.
    fn bitmap_texel_8bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (y as u32) * 240;
        let x_offset = x as u32;
        self.get_bg_byte(addr + y_offset + x_offset)
    }

    /// Get a bitmap texel, using direct colour.
    /// Bitmap size is 240x160.
    fn bitmap_texel_15bpp(&self, addr: u32, x: u8, y: u8) -> u16 {
        let y_offset = (y as u32) * 480;
        let x_offset = (x as u32) * 2;
        let texel_addr = addr + y_offset + x_offset;
        self.get_bg_halfword(texel_addr)
    }

    /// Get a bitmap texel, using direct colour.
    /// Bitmap size is 160x128.
    fn small_bitmap_texel_15bpp(&self, addr: u32, x: u8, y: u8) -> u16 {
        let y_offset = (y as u32) * 320;
        let x_offset = (x as u32) * 2;
        let texel_addr = addr + y_offset + x_offset;
        self.get_bg_halfword(texel_addr)
    }
}
