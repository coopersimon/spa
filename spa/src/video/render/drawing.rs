/// Software rendering.

use fixed::types::I24F8;
use crate::constants::gba;
use super::{
    colour::*,
    super::memory::*
};

const VRAM_TILE_BLOCK: u32 = 16 * 1024;
const TILE_SIZE: u32 = 8;
/// 4bpp tile size in bytes.
const TILE_BYTES: u32 = 32;
const TILE_MAP_SIZE: u32 = 32;
const VRAM_MAP_BLOCK: u32 = TILE_MAP_SIZE * TILE_MAP_SIZE * 2;

pub struct SoftwareRenderer {
    palette_cache:  PaletteCache
}

impl SoftwareRenderer {
    pub fn new() -> Self {
        Self {
            palette_cache:  PaletteCache::new()
        }
    }

    /// Create caches from dirty memory.
    pub fn setup_caches(&mut self, mem: &mut VideoMemory) {
        // Refresh palette cache
        if let Some(bg_palette_mem) = mem.palette.ref_bg_palette() {
            self.palette_cache.update_bg(bg_palette_mem);
        }
        if let Some(obj_palette_mem) = mem.palette.ref_obj_palette() {
            self.palette_cache.update_obj(obj_palette_mem);
        }
    }

    pub fn draw_line(&self, mem: &VideoMemory, target: &mut [u8], line: u16) {
        if mem.registers.in_fblank() {
            for p in target {
                *p = 0;
            }
        } else {
            self.draw(mem, target, line);
        }
    }
}

// Internal: draw layers
impl SoftwareRenderer {
    /// Draw object pixels to a target line.
    fn draw_obj_line(&self, mem: &VideoMemory, target: &mut [Option<ObjectPixel>], obj_window: &mut [bool], y: i16) {
        const OBJECT_VRAM_BASE: u32 = VRAM_TILE_BLOCK * 4;
        let use_1d_tile_mapping = mem.registers.obj_1d_tile_mapping();
        /*let check_windows = regs.windows_enabled();
        let render_objects = if check_windows {
            let check_win0 = regs.obj_window_0() && regs.y_inside_window_0(y as u8);
            let check_win1 = regs.obj_window_1() && regs.y_inside_window_1(y as u8);
            let check_outside = !check_win0 && !check_win1
        };*/

        // TODO: check windows for line
        for object in mem.oam.ref_objects() {
            if !object.is_enabled() {
                continue;
            }
            let (left, top) = object.coords();
            let (width, height) = object.size();
            if y < top || y >= (top + height) {
                continue;
            }

            // Lots of stuff we need for the object...
            let in_obj_window = object.is_obj_window();
            let semi_transparent = object.is_semi_transparent();
            let priority = object.priority();
            let palette_bank = object.palette_bank();
            let palette_offset = palette_bank.unwrap_or(0) * 16;
            let use_8bpp = palette_bank.is_none();
            let tile_shift = if use_8bpp {1} else {0};
            let base_tile_num = object.tile_num();
            let affine = object.affine_param_num();
            let object_y = y - top;

            let x_0 = I24F8::from_num((width / 2) as i32);
            let y_0 = I24F8::from_num((height / 2) as i32);
            let y_i = I24F8::from_num(object_y as i32) - y_0;

            let source_size = object.source_size();
            let source_x_0 = I24F8::from_num((source_size.0 / 2) as i32);
            let source_y_0 = I24F8::from_num((source_size.1 / 2) as i32);

            for object_x in 0..width {
                let x = left + object_x;
                if x < 0 || x >= (gba::H_RES as i16) {
                    continue;
                }
                if let Some(existing_pixel) = &target[x as usize] {
                    if existing_pixel.priority <= priority {
                        continue;
                    }
                }
                // TODO: check if inside window.

                // Find the pixel
                let (index_x, index_y) = if let Some(affine_params_num) = affine {
                    let params = mem.oam.affine_params(affine_params_num);
                    let x_i = I24F8::from_num(object_x as i32) - x_0;
                    let p_x = (params.pa * x_i) + (params.pb * y_i) + source_x_0;
                    let p_y = (params.pc * x_i) + (params.pd * y_i) + source_y_0;
                    let index_x = p_x.to_num::<i32>() as u8;
                    let index_y = p_y.to_num::<i32>() as u8;
                    if index_x >= source_size.0 || index_y >= source_size.1 {
                        continue;
                    }
                    (index_x, index_y)
                } else {
                    let index_x = if object.h_flip() {width - object_x - 1} else {object_x} as u8;
                    let index_y = if object.v_flip() {height - object_y - 1} else {object_y} as u8;
                    (index_x, index_y)
                };
                let tile_x = (index_x / 8) as u32;
                let tile_y = (index_y / 8) as u32;
                let tile_num = if use_1d_tile_mapping {
                    let tile_width = (width / 8) as u32;
                    let offset = (tile_x + (tile_y * tile_width)) << tile_shift;
                    base_tile_num + offset
                } else {
                    const TILE_GRID_WIDTH: u32 = 0x20;
                    const TILE_GRID_HEIGHT: u32 = 0x20;
                    let base_tile_x = base_tile_num % TILE_GRID_WIDTH;
                    let base_tile_y = base_tile_num / TILE_GRID_WIDTH;
                    let target_tile_x = base_tile_x.wrapping_add(tile_x << tile_shift) % TILE_GRID_WIDTH;
                    let target_tile_y = base_tile_y.wrapping_add(tile_y) % TILE_GRID_HEIGHT;
                    target_tile_x + (target_tile_y * TILE_GRID_WIDTH)
                };
                
                let tile_addr = OBJECT_VRAM_BASE + (tile_num * TILE_BYTES);
                let texel = if use_8bpp {
                    mem.vram.tile_texel_8bpp(tile_addr, index_x % 8, index_y % 8)
                } else {
                    mem.vram.tile_texel_4bpp(tile_addr, index_x % 8, index_y % 8)
                };
                // Transparent.
                if texel == 0 {
                    continue;
                }
                if in_obj_window {
                    obj_window[x as usize] = true;
                } else {
                    // Palette lookup.
                    target[x as usize] = Some(ObjectPixel{
                        colour: palette_offset + texel, priority, semi_transparent
                    });
                }
            }
        }
    }

    /// Get the palette number of a background pixel.
    /// The x and y values provided should be scrolled & mosaiced already (i.e., background values and not screen values).
    /// 
    /// If 0 is returned, the pixel is transparent.
    fn tile_bg_pixel(&self, bg: &TiledBackgroundData, vram: &VRAM, bg_x: u32, bg_y: u32) -> u8 {
        // TODO: Check if pixel is visible through window

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
        let tile_addr = bg.tile_data_addr + (attrs.tile_num() * TILE_BYTES);
        let texel = if bg.use_8bpp {
            vram.tile_texel_8bpp(tile_addr, tile_x, tile_y)
        } else {
            vram.tile_texel_4bpp(tile_addr, tile_x, tile_y)
        };
        if texel == 0 {
            0
        } else {
            (attrs.palette_num() * 16) + texel
        }
    }

    /// Get the palette number of a background pixel.
    /// The x and y values provided should be mosaiced already.
    /// 
    /// If 0 is returned, the pixel is transparent.
    fn affine_bg_pixel(&self, bg: &AffineBackgroundData, vram: &VRAM, screen_x: u32, screen_y: u32) -> u8 {
        // TODO: Check if pixel is visible through window

        // Transform from screen space to BG space.
        let x_0 = bg.bg_ref_point_x;
        let y_0 = bg.bg_ref_point_y;
        let x_i = I24F8::from_bits(screen_x as i32) - x_0;
        let y_i = I24F8::from_bits(screen_y as i32) - y_0;
        let x_out = (bg.matrix_a * x_i) + (bg.matrix_b * y_i) + x_0;
        let y_out = (bg.matrix_c * x_i) + (bg.matrix_d * y_i) + y_0;

        let bg_x = if bg.wrap {
            (x_out.to_bits() as u32) & (bg.size - 1)
        } else {
            let bg_x = x_out.to_bits() as u32;
            if bg_x >= bg.size {
                return 0;
            }
            bg_x
        };
        let bg_y = if bg.wrap {
            (y_out.to_bits() as u32) & (bg.size - 1)
        } else {
            let bg_y = y_out.to_bits() as u32;
            if bg_y >= bg.size {
                return 0;
            }
            bg_y
        };

        // Find tile attrs in bg map
        let map_x = bg_x / TILE_SIZE;
        let map_y = bg_y / TILE_SIZE;

        // The address of the tile attributes.
        let tile_map_addr = bg.tile_map_addr + map_x + map_y * bg.size;
        let tile_num = vram.affine_map_tile_num(tile_map_addr);
        
        let tile_x = (bg_x % TILE_SIZE) as u8;
        let tile_y = (bg_y % TILE_SIZE) as u8;
        let tile_addr = bg.tile_data_addr + (tile_num * TILE_BYTES * 2);
        vram.tile_texel_8bpp(tile_addr, tile_x, tile_y)
    }

    /// Draw a bitmap pixel.
    fn bitmap_bg_pixel(&self, bg: &BitmapBackgroundData, vram: &VRAM, palette: &PaletteCache, bg_x: u32, bg_y: u32) -> Option<Colour> {
        // TODO: Check if pixel is visible through window

        if bg.small {
            let bitmap_x = (bg_x as usize).wrapping_sub(gba::SMALL_BITMAP_LEFT);
            let bitmap_y = (bg_y as usize).wrapping_sub(gba::SMALL_BITMAP_TOP);
            if bitmap_x >= gba::SMALL_BITMAP_WIDTH || bitmap_y >= gba::SMALL_BITMAP_HEIGHT {
                return None;
            }
            let colour = vram.small_bitmap_texel_15bpp(bg.data_addr, bitmap_x as u32, bitmap_y as u32);
            Some(Colour::from_555(colour))
        } else if bg.use_15bpp {
            let colour = vram.bitmap_texel_15bpp(0, bg_x, bg_y);
            Some(Colour::from_555(colour))
        } else {
            let texel = vram.bitmap_texel_8bpp(bg.data_addr, bg_x, bg_y);
            if texel == 0 {
                None
            } else {
                Some(palette.get_bg(texel))
            }
        }
    }
}

// Internal: draw modes
impl SoftwareRenderer {
    fn draw(&self, mem: &VideoMemory, target: &mut [u8], line: u16) {
        // Gather the backgrounds.
        /*let mut bg_prio_data = Vec::<BackgroundData>::new();
        for bg in (0..4).map(|bg_num| mem.registers.tile_bg_data(bg_num)) {
            if let Some(bg_data) = bg.as_ref() {
                for i in 0..bg_prio_data.len() {
                    if bg_data.priority < bg_prio_data[i].priority {
                        bg_prio_data.insert(i, bg_data.clone());
                    }
                }
                bg_prio_data.push(bg_data.clone());
            }
        }*/
        let bg_data = mem.registers.bg_data_for_mode();

        let mut obj_line = [None; gba::H_RES];
        let mut obj_window = [false; gba::H_RES];
        self.draw_obj_line(mem, &mut obj_line, &mut obj_window, line as i16);
        for x in 0..gba::H_RES {
            let dest = x * 4;
            // Prio 0
            let colour = self.eval_pixel(mem, obj_line[x], &bg_data, x as u32, line as u32);
            target[dest] = colour.r;
            target[dest + 1] = colour.g;
            target[dest + 2] = colour.b;
        }
    }

    fn eval_pixel(&self, mem: &VideoMemory, obj_pixel: Option<ObjectPixel>, bg_data: &[BackgroundData], x: u32, y: u32) -> Colour {
        if let Some(obj) = obj_pixel {
            for priority in 0..4 {
                if obj.priority == priority {
                    return self.palette_cache.get_obj(obj.colour);
                } else {
                    for bg in bg_data {
                        if bg.priority() == priority {
                            if let Some(col) = self.bg_pixel(mem, bg, x, y) {
                                return col;
                            }
                        }
                    }
                }
            }
        } else {
            for priority in 0..4 {
                for bg in bg_data {
                    if bg.priority() == priority {
                        if let Some(col) = self.bg_pixel(mem, bg, x, y) {
                            return col;
                        }
                    }
                }
            }
        }
        self.palette_cache.get_backdrop()
    }

    /// Find a pixel value for a particular background.
    fn bg_pixel(&self, mem: &VideoMemory, bg: &BackgroundData, x: u32, y: u32) -> Option<Colour> {
        match bg {
            BackgroundData::Tiled(t) => {
                // TODO: check window...
                // TODO: mosaic..?
                let scrolled_x = x.wrapping_add(t.scroll_x as u32);
                let scrolled_y = y.wrapping_add(t.scroll_y as u32);
                let texel = self.tile_bg_pixel(t, &mem.vram, scrolled_x, scrolled_y);
                if texel != 0 {
                    Some(self.palette_cache.get_bg(texel))
                } else {
                    None
                }
            },
            BackgroundData::Affine(a) => {
                /*let texel = self.affine_bg_pixel(a, &mem.vram, x, y);
                if texel != 0 {
                    Some(self.palette_cache.get_bg(texel))
                } else {
                    None
                }*/
                None
            },
            BackgroundData::Bitmap(b) => self.bitmap_bg_pixel(b, &mem.vram, &self.palette_cache, x, y)
        }
    }
}

// Debug
impl SoftwareRenderer {
    /// Debug: Draws the current VRAM in 8bpp format.
    pub fn draw_8bpp_tiles(&self, mem: &VideoMemory, target: &mut [u8]) {
        for y in 0..(48 * 8) {
            // First 48KB.
            let tile_row = y / 8;
            let tile_y = y % 8;
            for x in 0..(16 * 8) {
                let tile_col = x / 8;
                let tile_x = x % 8;
                // Rows of 16 tiles.
                let texel = mem.vram.tile_texel_8bpp((tile_row * 1024) + (tile_col * 64), tile_x as u8, tile_y as u8);
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
                let texel = mem.vram.tile_texel_8bpp((tile_row * 1024) + (tile_col * 64), tile_x as u8, tile_y as u8);
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
}

// Helpers: addr calculation
// TODO: maybe these should be moved?

/// Provided the coordinates into an object, get the offset from the base tile.
/// 
/// The returned value is the offset in units of tiles. Multiply this by 2 when using 8bpp.
fn tile_num_for_coord_2d(base_tile_num: u32, obj_x: u8, obj_y: u8) -> u32 {
    const TILE_GRID_WIDTH: u32 = 0x20;
    const TILE_GRID_HEIGHT: u32 = 0x20;
    let base_tile_x = base_tile_num % TILE_GRID_WIDTH;
    let base_tile_y = base_tile_num / TILE_GRID_WIDTH;
    let tile_x = (obj_x / 8) as u32;
    let tile_y = (obj_y / 8) as u32;
    let target_tile_x = base_tile_x.wrapping_add(tile_x) % TILE_GRID_WIDTH;
    let target_tile_y = base_tile_y.wrapping_add(tile_y) % TILE_GRID_HEIGHT;
    target_tile_x + (target_tile_y * TILE_GRID_WIDTH)
}

/// Provided the coordinates into an object, and the base tile of the sprite,
/// get the tile number for the coords provided.
fn tile_offset_for_coord_1d(obj_x: u8, obj_y: u8, width_x: u8) -> u32 {
    let tile_width = (width_x / 8) as u32;
    let tile_x = (obj_x / 8) as u32;
    let tile_y = (obj_y / 8) as u32;
    tile_x + (tile_y * tile_width)
}
