
use bytemuck::{
    try_from_bytes, try_from_bytes_mut
};
use bitflags::bitflags;
use crate::common::bits::{u8, u16};

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

/// VRAM. Contains tile data, background maps, and bitmaps.
pub struct VRAM {
    data: Vec<u8>
}

impl VRAM {
    pub fn new() -> Self {
        Self {
            data: vec![0; 96 * 1024]
        }
    }

    /// Get a set of tile map attributes for a regular background.
    pub fn tile_map_attrs(&self, addr: u32) -> TileMapAttrs {
        let start = addr as usize;
        let end = start + 2;
        let data = *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read tile map attrs at 0x{:X}", addr));
        TileMapAttrs::from_bits_truncate(data)
    }

    /// Get the tile number for an affine background.
    pub fn affine_map_tile_num(&self, addr: u32) -> u32 {
        self.data[addr as usize] as u32
    }

    /// Get a texel for a particular tile, using 16-colour palette.
    pub fn tile_texel_4bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (4 * y) as u32;
        let x_offset = (x / 2) as u32;
        let data = self.data[(addr + y_offset + x_offset) as usize];
        if u8::test_bit(x, 0) {
            data >> 4
        } else {
            data & 0xF
        }
    }

    /// Get a texel for a particular tile, using 256-colour palette.
    pub fn tile_texel_8bpp(&self, addr: u32, x: u8, y: u8) -> u8 {
        let y_offset = (8 * y) as u32;
        let x_offset = x as u32;
        self.data[(addr + y_offset + x_offset) as usize]
    }

    /// Get a bitmap texel, using 256-colour palette.
    pub fn bitmap_texel_8bpp(&self, addr: u32, x: u32, y: u32) -> u8 {
        let y_offset = y * 240;
        self.data[(addr + y_offset + x) as usize]
    }

    /// Get a bitmap texel, using direct colour.
    /// Bitmap size is 240x160.
    pub fn bitmap_texel_15bpp(&self, addr: u32, x: u32, y: u32) -> u16 {
        let y_offset = y * 480;
        let x_offset = x * 2;
        let texel_addr = (addr + y_offset + x_offset) as usize;

        let start = texel_addr;
        let end = start + 2;
        *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read bitmap texel at 0x{:X}", start))
    }

    /// Get a bitmap texel, using direct colour.
    /// Bitmap size is 160x128.
    pub fn small_bitmap_texel_15bpp(&self, addr: u32, x: u32, y: u32) -> u16 {
        let y_offset = y * 320;
        let x_offset = x * 2;
        let texel_addr = (addr + y_offset + x_offset) as usize;

        let start = texel_addr;
        let end = start + 2;
        *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read bitmap texel at 0x{:X}", start))
    }
}

// Memory interface
impl VRAM {
    pub fn read_halfword(&self, addr: u32) -> u16 {
        let start = addr as usize;
        let end = (addr + 2) as usize;
        *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read vram halfword at 0x{:X}", addr))
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        let start = addr as usize;
        let end = (addr + 2) as usize;
        let dest = try_from_bytes_mut(&mut self.data[start..end]).expect(&format!("cannot write vram halfword at 0x{:X}", addr));
        *dest = data;
    }

    /*pub fn read_word(&self, addr: u32) -> u32 {
        let start = addr as usize;
        let end = (addr + 4) as usize;
        *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read vram word at 0x{:X}", addr))
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        let start = addr as usize;
        let end = (addr + 4) as usize;
        let dest = try_from_bytes_mut(&mut self.data[start..end]).expect(&format!("cannot write vram word at 0x{:X}", addr));
        *dest = data;
    }*/
}
