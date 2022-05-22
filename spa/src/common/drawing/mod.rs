/// Software rendering.

pub mod colour;
pub mod background;

use fixed::types::I24F8;
use crate::utils::bits::u16;
use crate::common::videomem::{
    VideoMemory, VideoRegisters, VRAM2D
};
use colour::*;
use background::*;

const TILE_SIZE: u32 = 8;
const TILE_SHIFT_4BPP: usize = 5;
const TILE_BYTES_4BPP: u32 = 1 << TILE_SHIFT_4BPP;
const TILE_BYTES_8BPP: u32 = TILE_BYTES_4BPP * 2;
const BMP_SHIFT: usize = 7;
const BMP_MAP_SIZE: u32 = 16;
const TILE_MAP_SIZE: u32 = 32;
const VRAM_MAP_BLOCK: u32 = TILE_MAP_SIZE * TILE_MAP_SIZE * 2;

/// Width of bitmap in GBA mode 5.
const SMALL_BITMAP_WIDTH: u32 = 160;
/// Height of bitmap in GBA mode 5.
const SMALL_BITMAP_HEIGHT: u32 = 128;
const SMALL_BITMAP_LEFT: u32 = (240 - SMALL_BITMAP_WIDTH) / 2;
const SMALL_BITMAP_TOP: u32 = (160 - SMALL_BITMAP_HEIGHT) / 2;
/// Width of bitmap in GBA mode 4, 5.
const LARGE_BITMAP_WIDTH: u32 = 240;

pub enum RendererMode {
    GBA,
    NDSA,
    NDSB
}

pub struct SoftwareRenderer {
    mode:   RendererMode,
    h_res:  usize,
    palette_cache:  PaletteCache
}

impl SoftwareRenderer {
    pub fn new(mode: RendererMode) -> Self {
        // TODO: source constants from constants files
        let h_res = match mode {
            RendererMode::GBA   => 240,
            /* NDS */_          => 256,
        };
        Self {
            mode:   mode,
            h_res:  h_res,
            palette_cache:  PaletteCache::new()
        }
    }

    /// Create caches from dirty memory.
    pub fn setup_caches<V: VRAM2D>(&mut self, mem: &mut VideoMemory<V>) {
        // Refresh palette cache
        match self.mode {
            RendererMode::GBA => {
                if let Some(bg_palette_mem) = mem.palette.ref_bg_palette() {
                    self.palette_cache.update_bg_555(bg_palette_mem);
                }
                if let Some(obj_palette_mem) = mem.palette.ref_obj_palette() {
                    self.palette_cache.update_obj_555(obj_palette_mem);
                }
            },
            _ => {
                if let Some(bg_palette_mem) = mem.palette.ref_bg_palette() {
                    self.palette_cache.update_bg_565(bg_palette_mem);
                }
                if let Some(obj_palette_mem) = mem.palette.ref_obj_palette() {
                    self.palette_cache.update_obj_565(obj_palette_mem);
                }
                for (n, palette) in mem.vram.ref_ext_bg_palette().iter().enumerate() {
                    if let Some(bg_ext_palette) = palette {
                        self.palette_cache.update_ext_bg(n, bg_ext_palette);
                    }
                }
                if let Some(obj_ext_palette) = mem.vram.ref_ext_obj_palette() {
                    self.palette_cache.update_ext_obj(obj_ext_palette);
                }
            }
        }
    }

    /// Draw a regular 2D line, with BG and OBJ, and applied effects.
    /// 
    /// The only mode for GBA, also used in NDS for 2D engines A and B
    pub fn draw_line<V: VRAM2D>(&self, mem: &VideoMemory<V>, target: &mut [u8], line: u8) {
        if mem.registers.in_fblank() {
            for p in target {
                *p = 0;
            }
        } else {
            self.draw(mem, target, line);
        }
    }
}

// Internal: GBA / NDS
impl SoftwareRenderer {
    /// Check if tiled objects should use 2D or 1D mapping.
    /// 
    /// 2D mapping: grid of 32x32 tiles. An object that
    /// is larger than 1 tile will expand into x and y
    /// dimensions appropriately.
    /// 
    /// 1D mapping: List of 1024 tiles.
    fn obj_1d_tile_mapping(&self, regs: &VideoRegisters) -> bool {
        match self.mode {
            RendererMode::GBA   => regs.gba_obj_1d_tile_mapping(),
            _                   => regs.nds_obj_1d_tile_mapping(),
        }
    }

    /// The shift needed to convert tile number into
    /// VRAM address, for 1D objects.
    fn obj_1d_tile_shift(&self, regs: &VideoRegisters) -> usize {
        match self.mode {
            RendererMode::NDSA => match regs.nds_obj_1d_tile_boundary() {
                0 => TILE_SHIFT_4BPP,
                1 => TILE_SHIFT_4BPP + 1,
                2 => TILE_SHIFT_4BPP + 2,
                _ => TILE_SHIFT_4BPP + 3,
            },
            RendererMode::NDSB => match regs.nds_obj_1d_tile_boundary() {
                0 => TILE_SHIFT_4BPP,
                1 => TILE_SHIFT_4BPP + 1,
                _ => TILE_SHIFT_4BPP + 2,
            }
            _ => TILE_SHIFT_4BPP,
        }
    }

    /// The shift needed to convert tile number into
    /// VRAM address for bitmaps.
    fn obj_bmp_shift(&self, regs: &VideoRegisters) -> usize {
        match self.mode {
            RendererMode::NDSA if regs.obj_1d_bmp_large_boundary() => BMP_SHIFT + 1,
            _ => BMP_SHIFT,
        }
    }
}

// Internal: draw layers
impl SoftwareRenderer {
    /// Draw object pixels to a target line.
    fn draw_obj_line<V: VRAM2D>(&self, mem: &VideoMemory<V>, target: &mut [Option<ObjectPixel>], obj_window: &mut [bool], y: u8) {
        // Global settings
        let use_1d_tile_mapping = self.obj_1d_tile_mapping(&mem.registers);
        let tile_1d_start_shift = self.obj_1d_tile_shift(&mem.registers);

        let use_1d_bmp_mapping = mem.registers.obj_1d_bmp_mapping();
        let bmp_1d_addr_shift = self.obj_bmp_shift(&mem.registers);
        let bmp_2d_width = if mem.registers.obj_2d_wide_bmp() {BMP_MAP_SIZE * 2} else {BMP_MAP_SIZE};
        let bmp_2d_mask = bmp_2d_width - 1;

        let mosaic_x = mem.registers.obj_mosaic_x();
        let mosaic_y = mem.registers.obj_mosaic_y();

        for object in mem.oam.ref_objects() {
            if !object.is_enabled() {
                continue;
            }
            let (left, top) = object.coords();
            let (width, height) = object.size();
            let object_y = y.wrapping_sub(top);
            if object_y >= height {
                continue;
            }
            // Lots of stuff we need for the object...
            let in_obj_window = object.is_obj_window();
            let semi_transparent = object.is_semi_transparent();
            let bitmap = object.is_bitmap();
            let priority = object.priority();
            let palette_offset = object.palette_bank() * 16;
            let use_8bpp = object.use_8bpp();
            let tile_shift = if use_8bpp {1} else {0};
            let base_tile_num = object.tile_num();
            let affine = object.affine_param_num();

            let x_0 = I24F8::from_num((width / 2) as i32);
            let y_0 = I24F8::from_num((height / 2) as i32);
            let y_i = I24F8::from_num(object_y as i32) - y_0;

            let source_size = object.source_size();
            let source_x_0 = I24F8::from_num((source_size.0 / 2) as i32);
            let source_y_0 = I24F8::from_num((source_size.1 / 2) as i32);

            for object_x in 0..width {
                let x = left.wrapping_add(object_x);
                if x >= (self.h_res as u16) {
                    continue;
                }
                if !in_obj_window {
                    if let Some(existing_pixel) = &target[x as usize] {
                        if existing_pixel.priority <= priority {
                            continue;
                        }
                    }
                }

                // Find the pixel
                let (index_x, index_y) = if let Some(affine_params_num) = affine {
                    let params = mem.oam.affine_params(affine_params_num);
                    let x_i = I24F8::from_num(object_x as i32) - x_0;
                    let p_x = (params.pa * x_i) + (params.pb * y_i) + source_x_0;
                    let p_y = (params.pc * x_i) + (params.pd * y_i) + source_y_0;
                    let index_x = p_x.to_num::<i32>() as u16;
                    let index_y = p_y.to_num::<i32>() as u16;
                    if index_x >= (source_size.0 as u16) || index_y >= (source_size.1 as u16) {
                        continue;
                    }
                    (index_x as u8, index_y as u8)
                } else {
                    let index_x = if object.h_flip() {width - object_x - 1} else {object_x} as u8;
                    let index_y = if object.v_flip() {height - object_y - 1} else {object_y} as u8;
                    (index_x, index_y)
                };
                let (index_x, index_y) = if object.is_mosaic() {
                    (index_x - (index_x % mosaic_x), index_y - (index_y % mosaic_y))
                } else {
                    (index_x, index_y)
                };

                let colour = if bitmap {
                    let addr = if use_1d_bmp_mapping {
                        let base = base_tile_num << bmp_1d_addr_shift;
                        let offset_x = index_x as u32;
                        let offset_y = index_y as u32 * source_size.0 as u32;
                        base + ((offset_x + offset_y) * 2)
                    } else {
                        let base_tile_x = base_tile_num & bmp_2d_mask;
                        let base_tile_y = base_tile_num & (!bmp_2d_mask);
                        let target_tile_x = base_tile_x * BMP_MAP_SIZE;                 // In pixels
                        let target_tile_y = base_tile_y * (BMP_MAP_SIZE * TILE_SIZE);   // In pixels
                        let base = target_tile_x + target_tile_y;
                        let offset_x = index_x as u32;
                        let offset_y = (index_y as u32) * (bmp_2d_width * TILE_SIZE);
                        (base + offset_x + offset_y) * 2
                    };
                    let colour = mem.vram.get_obj_halfword(addr);
                    // Transparent.
                    if !u16::test_bit(colour, 15) {
                        continue;
                    }
                    ColType::Direct(colour)
                } else {
                    let tile_x = (index_x / 8) as u32;
                    let tile_y = (index_y / 8) as u32;
                    let tile_addr = if use_1d_tile_mapping {
                        let start = base_tile_num << tile_1d_start_shift;
                        let tile_width = (source_size.0 / 8) as u32;    // Width of object in tiles.
                        let offset = (tile_x + (tile_y * tile_width)) << tile_shift;
                        start + (offset << TILE_SHIFT_4BPP)
                    } else {
                        const TILE_GRID_WIDTH: u32 = 0x20;
                        const TILE_GRID_HEIGHT: u32 = 0x20;
                        let base_tile_x = base_tile_num % TILE_GRID_WIDTH;
                        let base_tile_y = base_tile_num / TILE_GRID_WIDTH;
                        let target_tile_x = base_tile_x.wrapping_add(tile_x << tile_shift) % TILE_GRID_WIDTH;
                        let target_tile_y = base_tile_y.wrapping_add(tile_y) % TILE_GRID_HEIGHT;
                        (target_tile_x + (target_tile_y * TILE_GRID_WIDTH)) << TILE_SHIFT_4BPP
                    };
                    
                    let texel = if use_8bpp {
                        mem.vram.obj_tile_texel_8bpp(tile_addr, index_x % 8, index_y % 8)
                    } else {
                        mem.vram.obj_tile_texel_4bpp(tile_addr, index_x % 8, index_y % 8)
                    };
                    // Transparent.
                    if texel == 0 {
                        continue;
                    }
                    if use_8bpp {
                        if mem.registers.obj_ext_palette() {
                            let offset = (palette_offset as u16) * 16;
                            ColType::Extended(offset + (texel as u16))
                        } else {
                            ColType::Palette(texel)
                        }
                    } else {
                        ColType::Palette(palette_offset + texel)
                    }
                };

                if in_obj_window {
                    obj_window[x as usize] = true;
                } else {
                    let obj_type = if bitmap {
                        ObjType::Bitmap(object.palette_bank() as u16)
                    } else if semi_transparent {
                        ObjType::SemiTransparent
                    } else {
                        ObjType::None
                    };
                    target[x as usize] = Some(ObjectPixel{
                        colour, priority, obj_type
                    });
                }
            }
        }
    }

    /// Get the colour of a background pixel.
    /// The x and y values provided should be scrolled & mosaiced already (i.e., background values and not screen values).
    /// 
    /// If None is returned, the pixel is transparent.
    fn tile_bg_pixel(&self, bg: &TiledBackgroundData, vram: &impl VRAM2D, bg_x: u32, bg_y: u32) -> Option<Colour> {
        let (x, y) = match bg.layout {
            BackgroundMapLayout::Small => (bg_x % 256, bg_y % 256),
            BackgroundMapLayout::Wide => (bg_x % 512, bg_y % 256),
            BackgroundMapLayout::Tall => (bg_x % 256, bg_y % 512),
            BackgroundMapLayout::Large => (bg_x % 512, bg_y % 512),
        };

        // Find tile attrs in bg map
        let map_x = x / TILE_SIZE;
        let map_y = y / TILE_SIZE;
        let tile_map_offset = match bg.layout {
            BackgroundMapLayout::Small => 0,
            BackgroundMapLayout::Wide => if map_x >= TILE_MAP_SIZE {
                VRAM_MAP_BLOCK
            } else {
                0
            },
            BackgroundMapLayout::Tall => if map_y >= TILE_MAP_SIZE {
                VRAM_MAP_BLOCK
            } else {
                0
            },
            BackgroundMapLayout::Large => match (map_x >= TILE_MAP_SIZE, map_y >= TILE_MAP_SIZE) {
                (false, false) => 0,
                (true, false) => VRAM_MAP_BLOCK,
                (false, true) => VRAM_MAP_BLOCK * 2,
                (true, true) => VRAM_MAP_BLOCK * 3
            }
        };
        let submap_x = map_x % TILE_MAP_SIZE;
        let submap_y = map_y % TILE_MAP_SIZE;
        // The address of the tile attributes.
        let tile_map_addr = bg.tile_map_addr + tile_map_offset + (submap_x + submap_y * TILE_MAP_SIZE) * 2;
        let attrs = vram.tile_map_attrs(tile_map_addr);
        
        let mut tile_x = (x % TILE_SIZE) as u8;
        let mut tile_y = (y % TILE_SIZE) as u8;
        if attrs.h_flip() {
            tile_x = 7 - tile_x;
        }
        if attrs.v_flip() {
            tile_y = 7 - tile_y;
        }
        let texel = if bg.use_8bpp {
            let tile_addr = bg.tile_data_addr + (attrs.tile_num() * TILE_BYTES_8BPP);
            vram.bg_tile_texel_8bpp(tile_addr, tile_x, tile_y)
        } else {
            let tile_addr = bg.tile_data_addr + (attrs.tile_num() * TILE_BYTES_4BPP);
            vram.bg_tile_texel_4bpp(tile_addr, tile_x, tile_y)
        };
        if texel == 0 {
            None
        } else {
            if bg.use_8bpp {
                if let Some(slot) = bg.ext_palette {
                    let palette_offset = (attrs.palette_num() as u16) * 256;
                    Some(self.palette_cache.get_ext_bg(slot, palette_offset + (texel as u16)))
                } else {
                    Some(self.palette_cache.get_bg(texel))
                }
            } else {
                Some(self.palette_cache.get_bg((attrs.palette_num() * 16) + texel))
            }
        }
    }

    /// Get the palette number of a background pixel.
    /// The x and y values provided should be mosaiced already.
    /// 
    /// If 0 is returned, the pixel is transparent.
    fn affine_bg_pixel(&self, bg: &AffineBackgroundData, vram: &impl VRAM2D, screen_x: u8, _screen_y: u8) -> Option<Colour> {
        // Transform from screen space to BG space.
        // Displacement points x0 and y0 are incremented by matrix points B and D respectively
        // after each scanline, simulating (B * y_i) + x_0 and (D * y_i) + x_0
        let x_0 = bg.bg_ref_point_x;
        let y_0 = bg.bg_ref_point_y;
        let x_i = I24F8::from_num(screen_x as i32);
        //let y_i = I24F8::from_num(screen_y as i32);
        let x_out = (bg.matrix_a * x_i) + x_0;
        let y_out = (bg.matrix_c * x_i) + y_0;

        let bg_x = if bg.wrap {
            (x_out.to_num::<i32>() as u32) & (bg.size - 1)
        } else {
            let bg_x = x_out.to_num::<i32>() as u32;
            if bg_x >= bg.size {
                return None;
            }
            bg_x
        };
        let bg_y = if bg.wrap {
            (y_out.to_num::<i32>() as u32) & (bg.size - 1)
        } else {
            let bg_y = y_out.to_num::<i32>() as u32;
            if bg_y >= bg.size {
                return None;
            }
            bg_y
        };

        // Find tile attrs in bg map
        let map_x = bg_x / TILE_SIZE;
        let map_y = bg_y / TILE_SIZE;
        let map_width = bg.size / TILE_SIZE;

        // The address of the tile attributes.
        let tile_map_addr = bg.tile_map_addr + map_x + (map_y * map_width);
        let tile_num = vram.affine_map_tile_num(tile_map_addr);
        
        let tile_x = (bg_x % TILE_SIZE) as u8;
        let tile_y = (bg_y % TILE_SIZE) as u8;
        let tile_addr = bg.tile_data_addr + (tile_num * TILE_BYTES_8BPP);
        let texel = vram.bg_tile_texel_8bpp(tile_addr, tile_x, tile_y);
        if texel == 0 {
            None
        } else {
            Some(self.palette_cache.get_bg(texel))
        }
    }

    /// Get the colour of a tile affine BG pixel (NDS only).
    /// The x and y values provided should be mosaiced already.
    /// 
    /// If None is returned, the pixel is transparent.
    fn tile_affine_bg_pixel(&self, bg: &AffineBackgroundData, vram: &impl VRAM2D, screen_x: u8, _screen_y: u8) -> Option<Colour> {
        // Transform from screen space to BG space.
        // Displacement points x0 and y0 are incremented by matrix points B and D respectively
        // after each scanline, simulating (B * y_i) + x_0 and (D * y_i) + x_0
        let x_0 = bg.bg_ref_point_x;
        let y_0 = bg.bg_ref_point_y;
        let x_i = I24F8::from_num(screen_x as i32);
        //let y_i = I24F8::from_num(screen_y as i32);
        let x_out = (bg.matrix_a * x_i) + x_0;
        let y_out = (bg.matrix_c * x_i) + y_0;

        let bg_x = if bg.wrap {
            (x_out.to_num::<i32>() as u32) & (bg.size - 1)
        } else {
            let bg_x = x_out.to_num::<i32>() as u32;
            if bg_x >= bg.size {
                return None;
            }
            bg_x
        };
        let bg_y = if bg.wrap {
            (y_out.to_num::<i32>() as u32) & (bg.size - 1)
        } else {
            let bg_y = y_out.to_num::<i32>() as u32;
            if bg_y >= bg.size {
                return None;
            }
            bg_y
        };

        // Find tile attrs in bg map
        let map_x = bg_x / TILE_SIZE;
        let map_y = bg_y / TILE_SIZE;
        let map_width = bg.size / TILE_SIZE;
        // The address of the tile attributes.
        let tile_map_addr = bg.tile_map_addr + (map_x + (map_y * map_width)) * 2;
        let attrs = vram.tile_map_attrs(tile_map_addr);
        
        let mut tile_x = (bg_x % TILE_SIZE) as u8;
        let mut tile_y = (bg_y % TILE_SIZE) as u8;
        if attrs.h_flip() {
            tile_x = 7 - tile_x;
        }
        if attrs.v_flip() {
            tile_y = 7 - tile_y;
        }
        let tile_addr = bg.tile_data_addr + (attrs.tile_num() * TILE_BYTES_8BPP);
        let texel = vram.bg_tile_texel_8bpp(tile_addr, tile_x, tile_y);
        if texel == 0 {
            None
        } else {
            if let Some(slot) = bg.ext_palette {
                let palette_offset = (attrs.palette_num() as u16) * 256;
                Some(self.palette_cache.get_ext_bg(slot, palette_offset + (texel as u16)))
            } else {
                Some(self.palette_cache.get_bg(texel))
            }
        }
    }

    /// Draw a bitmap pixel.
    fn bitmap_bg_pixel(&self, bg: &BitmapBackgroundData, vram: &impl VRAM2D, bg_x: u32, bg_y: u32) -> Option<Colour> {
        if bg.small {
            let bitmap_x = bg_x.wrapping_sub(SMALL_BITMAP_LEFT);
            let bitmap_y = bg_y.wrapping_sub(SMALL_BITMAP_TOP);
            if bitmap_x >= SMALL_BITMAP_WIDTH || bitmap_y >= SMALL_BITMAP_HEIGHT {
                return None;
            }
            let colour = vram.bg_bitmap_texel_15bpp(bg.data_addr, bitmap_x, bitmap_y, SMALL_BITMAP_WIDTH);
            Some(Colour::from_555(colour))
        } else if bg.use_15bpp {
            let colour = vram.bg_bitmap_texel_15bpp(0, bg_x, bg_y, LARGE_BITMAP_WIDTH);
            Some(Colour::from_555(colour))
        } else {
            let texel = vram.bg_bitmap_texel_8bpp(bg.data_addr, bg_x, bg_y, LARGE_BITMAP_WIDTH);
            if texel == 0 {
                None
            } else {
                Some(self.palette_cache.get_bg(texel))
            }
        }
    }

    /// Draw an affine bitmap pixel (NDS only).
    fn bitmap_affine_bg_pixel(&self, bg: &BitmapAffineBackgroundData, vram: &impl VRAM2D, screen_x: u8, _screen_y: u8) -> Option<Colour> {
        // Transform from screen space to BG space.
        // Displacement points x0 and y0 are incremented by matrix points B and D respectively
        // after each scanline, simulating (B * y_i) + x_0 and (D * y_i) + x_0
        let x_0 = bg.bg_ref_point_x;
        let y_0 = bg.bg_ref_point_y;
        let x_i = I24F8::from_num(screen_x as i32);
        //let y_i = I24F8::from_num(screen_y as i32);
        let x_out = (bg.matrix_a * x_i) + x_0;
        let y_out = (bg.matrix_c * x_i) + y_0;

        let bg_x = if bg.wrap {
            (x_out.to_num::<i32>() as u32) & (bg.size.0 - 1)
        } else {
            let bg_x = x_out.to_num::<i32>() as u32;
            if bg_x >= bg.size.0 {
                return None;
            }
            bg_x
        };
        let bg_y = if bg.wrap {
            (y_out.to_num::<i32>() as u32) & (bg.size.1 - 1)
        } else {
            let bg_y = y_out.to_num::<i32>() as u32;
            if bg_y >= bg.size.1 {
                return None;
            }
            bg_y
        };

        if bg.use_15bpp {
            let colour = vram.bg_bitmap_texel_15bpp(bg.data_addr, bg_x, bg_y, bg.size.0);
            if !u16::test_bit(colour, 15) {
                None
            } else {
                Some(Colour::from_555(colour))
            }
        } else {
            let texel = vram.bg_bitmap_texel_8bpp(bg.data_addr, bg_x, bg_y, bg.size.0);
            if texel == 0 {
                None
            } else {
                Some(self.palette_cache.get_bg(texel))
            }
        }
    }
}

// Internal: draw modes
impl SoftwareRenderer {
    fn draw<V: VRAM2D>(&self, mem: &VideoMemory<V>, target: &mut [u8], line: u8) {
        // Gather the backgrounds.
        let bg_data = match self.mode {
            RendererMode::GBA => mem.registers.gba_bg_data_for_mode(),
            RendererMode::NDSA => mem.registers.nds_bg_data_for_mode(),
            RendererMode::NDSB => mem.registers.nds_bg_data_for_mode(), // TODO: disallow mode 6
        };

        // TODO: don't alloc these every time
        let mut obj_line = vec![None; self.h_res];
        let mut obj_window = vec![false; self.h_res];
        if mem.registers.is_obj_enabled() {
            self.draw_obj_line(mem, &mut obj_line, &mut obj_window, line);
        }
        for x in 0..self.h_res {
            let dest = x * 4;
            // Prio 0
            let colour = self.eval_pixel(mem, obj_line[x], obj_window[x], &bg_data, x as u8, line);
            target[dest] = colour.r;
            target[dest + 1] = colour.g;
            target[dest + 2] = colour.b;
        }
    }

    fn eval_pixel<V: VRAM2D>(&self, mem: &VideoMemory<V>, obj_pixel: Option<ObjectPixel>, obj_window: bool, bg_data: &[BackgroundData], x: u8, y: u8) -> Colour {
        let colour_window = || {
            self.window_pixel(&mem.registers, mem.registers.colour_window_mask(), obj_window, x, y)
        };
        let mut target_1: Option<BlendTarget1> = None;
        for priority in 0..4 {
            if let Some(obj) = obj_pixel {
                if obj.priority == priority {
                    if self.window_pixel(&mem.registers, mem.registers.obj_window_mask(), obj_window, x, y) {
                        let col = match obj.colour {
                            ColType::Palette(c) => self.palette_cache.get_obj(c),
                            ColType::Extended(c) => self.palette_cache.get_ext_obj(c),
                            ColType::Direct(c) => Colour::from_555(c),
                        };
                        if colour_window() {
                            match self.colour_effect(&mem.registers, mem.registers.obj_blend_mask(), col, target_1, obj.obj_type) {
                                Blended::Colour(c) => return c,
                                Blended::AlphaTarget1(a) => target_1 = Some(a),
                            }
                        } else {
                            return col;
                        }
                    }
                }
            }
            for bg in bg_data {
                if bg.priority == priority {
                    if let Some(col) = self.bg_pixel(mem, bg, obj_window, x, y) {
                        if colour_window() {
                            match self.colour_effect(&mem.registers, bg.blend_mask, col, target_1, ObjType::None) {
                                Blended::Colour(c) => return c,
                                Blended::AlphaTarget1(a) => target_1 = Some(a),
                            }
                        } else {
                            return col;
                        }
                    }
                }
            }
        }
        let col = self.palette_cache.get_backdrop();
        if colour_window() {
            match self.colour_effect(&mem.registers, mem.registers.backdrop_blend_mask(), col, target_1, ObjType::None) {
                Blended::Colour(c) => c,
                Blended::AlphaTarget1(a) => a.colour,
            }
        } else {
            col
        }
    }

    /// Find a pixel value for a particular background.
    fn bg_pixel<V: VRAM2D>(&self, mem: &VideoMemory<V>, bg: &BackgroundData, obj_window: bool, x: u8, y: u8) -> Option<Colour> {
        if !self.window_pixel(&mem.registers, bg.window_mask, obj_window, x, y) {
            return None;
        }
        let (x, y) = if bg.mosaic {
            (x - x % mem.registers.bg_mosaic_x(), y - y % mem.registers.bg_mosaic_y())
        } else {
            (x, y)
        };
        use BackgroundTypeData::*;
        match &bg.type_data {
            Tiled(t) => {
                let scrolled_x = (x as u32).wrapping_add(t.scroll_x as u32);
                let scrolled_y = (y as u32).wrapping_add(t.scroll_y as u32);
                self.tile_bg_pixel(t, &mem.vram, scrolled_x, scrolled_y)
            },
            Affine(a) => self.affine_bg_pixel(a, &mem.vram, x, y),
            Bitmap(b) => self.bitmap_bg_pixel(b, &mem.vram, x as u32, y as u32),
            ExtTiledAffine(a) => self.tile_affine_bg_pixel(a, &mem.vram, x, y),
            ExtBitmapAffine(ba) => self.bitmap_affine_bg_pixel(ba, &mem.vram, x, y),
        }
    }

    /// Check if a background pixel should appear through windows.
    fn window_pixel(&self, regs: &VideoRegisters, mask: WindowMask, obj_window: bool, x: u8, y: u8) -> bool {
        if !regs.windows_enabled() {
            return true;
        }
        if regs.window_0_enabled() {
            if regs.x_inside_window_0(x) && regs.y_inside_window_0(y) {
                return mask.contains(WindowMask::WINDOW_0);
            }
        }
        if regs.window_1_enabled() {
            if regs.x_inside_window_1(x) && regs.y_inside_window_1(y) {
                return mask.contains(WindowMask::WINDOW_1);
            }
        }
        if regs.window_obj_enabled() {
            if obj_window {
                return mask.contains(WindowMask::OBJ_WIN);
            }
        }
        mask.contains(WindowMask::OUT_WIN)
    }

    /// Apply colour effects.
    fn colour_effect(&self, regs: &VideoRegisters, mask: BlendMask, colour: Colour, target_1: Option<BlendTarget1>, obj_type: ObjType) -> Blended {
        use Blended::*;
        if let Some(target_1) = target_1 {
            if mask.contains(BlendMask::LAYER_2) {
                Colour(apply_alpha_blend(target_1.alpha, regs.get_alpha_coeff_b(), target_1.colour, colour))
            } else {
                Colour(target_1.colour)
            }
        } else {
            match obj_type {
                ObjType::SemiTransparent => AlphaTarget1(BlendTarget1 {colour, alpha: regs.get_alpha_coeff_a()}),
                ObjType::Bitmap(alpha)  => AlphaTarget1(BlendTarget1 {colour, alpha}),
                ObjType::None           => match regs.colour_effect() {
                    ColourEffect::AlphaBlend => if mask.contains(BlendMask::LAYER_1) {
                        AlphaTarget1(BlendTarget1 {colour, alpha: regs.get_alpha_coeff_a()})
                    } else {
                        Colour(colour)
                    },
                    ColourEffect::Brighten => if mask.contains(BlendMask::LAYER_1) {
                        Colour(apply_brighten(regs.get_brightness_coeff(), colour))
                    } else {
                        Colour(colour)
                    },
                    ColourEffect::Darken => if mask.contains(BlendMask::LAYER_1) {
                        Colour(apply_darken(regs.get_brightness_coeff(), colour))
                    } else {
                        Colour(colour)
                    },
                    _ => Colour(colour)
                }
            }
        }
    }
}

/// The components of alpha blend "target 1".
/// (The top layer of a blend.)
struct BlendTarget1 {
    /// The colour to blend.
    colour: Colour,
    /// Alpha of this colour.
    alpha:  u16,
}

enum Blended {
    AlphaTarget1(BlendTarget1),
    Colour(Colour)
}

// Debug
impl SoftwareRenderer {
    /// Debug: Draws the current VRAM in 8bpp format.
    pub fn draw_8bpp_tiles<V: VRAM2D>(&self, mem: &VideoMemory<V>, target: &mut [u8]) {
        for y in 0..(48 * 8) {
            // First 48KB.
            let tile_row = y / 8;
            let tile_y = y % 8;
            for x in 0..(16 * 8) {
                let tile_col = x / 8;
                let tile_x = x % 8;
                // Rows of 16 tiles.
                let texel = mem.vram.bg_tile_texel_8bpp((tile_row * 1024) + (tile_col * 64), tile_x as u8, tile_y as u8);
                let colour = self.palette_cache.get_bg(texel);
                let pixel_num = (((y * 256) + x) * 4) as usize;
                target[pixel_num] = colour.r;
                target[pixel_num + 1] = colour.g;
                target[pixel_num + 2] = colour.b;
            }
            // Second 48KB.
            let tile_row = (y / 8) + 48;
            for x in (16 * 8)..(32 * 8) {
                let tile_col = (x / 8) - 16;
                let tile_x = x % 8;
                // Rows of 16 tiles.
                let texel = mem.vram.bg_tile_texel_8bpp((tile_row * 1024) + (tile_col * 64), tile_x as u8, tile_y as u8);
                let colour = if tile_row >= 64 {
                    self.palette_cache.get_obj(texel)
                } else {
                    self.palette_cache.get_bg(texel)
                };
                let pixel_num = (((y * 256) + x) * 4) as usize;
                target[pixel_num] = colour.r;
                target[pixel_num + 1] = colour.g;
                target[pixel_num + 2] = colour.b;
            }
        }
    }

    /// Debug: Draws the current VRAM in 4bpp format.
    pub fn draw_4bpp_tiles<V: VRAM2D>(&self, mem: &VideoMemory<V>, target: &mut [u8]) {
        for tile_num in 0..1024 {
            for y in 0..8 {
                for x in 0..8 {
                    let offset = 0 * 1024;
                    let texel = mem.vram.bg_tile_texel_4bpp(offset + (tile_num * TILE_BYTES_4BPP), x as u8, y as u8);
                    let colour = self.palette_cache.get_bg(texel);

                    let tile_x = tile_num % 32;
                    let tile_y = tile_num / 32;
                    let pixel_x = (tile_x * 8) + x;
                    let pixel_y = (tile_y * 8) + y;
                    let pixel_num = (((pixel_y * 256) + pixel_x) * 4) as usize;
                    target[pixel_num] = colour.r;
                    target[pixel_num + 1] = colour.g;
                    target[pixel_num + 2] = colour.b;
                }
            }
        }
    }
}

fn apply_alpha_blend(eva: u16, evb: u16, target_1: Colour, target_2: Colour) -> Colour {
    let r_mid = (target_1.r as u16) * eva + (target_2.r as u16) * evb;
    let g_mid = (target_1.g as u16) * eva + (target_2.g as u16) * evb;
    let b_mid = (target_1.b as u16) * eva + (target_2.b as u16) * evb;
    Colour {
        r: std::cmp::min(0xFF, r_mid >> 4) as u8,
        g: std::cmp::min(0xFF, g_mid >> 4) as u8,
        b: std::cmp::min(0xFF, b_mid >> 4) as u8,
    }
}

fn apply_brighten(evy: u16, target: Colour) -> Colour {
    let r_var = (((0xFF - target.r) as u16) * evy) >> 4;
    let g_var = (((0xFF - target.g) as u16) * evy) >> 4;
    let b_var = (((0xFF - target.b) as u16) * evy) >> 4;
    Colour {
        r: target.r.saturating_add(r_var as u8),
        g: target.g.saturating_add(g_var as u8),
        b: target.b.saturating_add(b_var as u8),
    }
}

fn apply_darken(evy: u16, target: Colour) -> Colour {
    let r_var = ((target.r as u16) * evy) >> 4;
    let g_var = ((target.g as u16) * evy) >> 4;
    let b_var = ((target.b as u16) * evy) >> 4;
    Colour {
        r: target.r.saturating_sub(r_var as u8),
        g: target.g.saturating_sub(g_var as u8),
        b: target.b.saturating_sub(b_var as u8),
    }
}
