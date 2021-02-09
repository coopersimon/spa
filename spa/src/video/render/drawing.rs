/// Software rendering.

use crate::constants::gba;
use super::{
    colour::*,
    super::memory::*
};

const VRAM_TILE_BLOCK: usize = 16 * 1024;
const TILE_SIZE: usize = 8;
/// 4bpp tile size in bytes.
const TILE_BYTES: usize = 32;
const TILE_MAP_SIZE: usize = 32;
const VRAM_MAP_BLOCK: usize = TILE_MAP_SIZE * TILE_MAP_SIZE * 2;

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
            match mem.registers.mode() {
                0 => self.draw_mode_0(mem, target, line),
                1 => {},
                2 => {},
                3 => {},
                4 => {},
                5 => {},
                _ => panic!("unknown mode!"),
            }
        }
    }
}

// Internal: draw layers
impl SoftwareRenderer {
    /// Draw object pixels to a target line.
    fn draw_obj_line(&self, mem: &VideoMemory, target: &mut [Option<ObjectPixel>], obj_window: &mut [bool], y: i16) {
        const OBJECT_VRAM_BASE: usize = VRAM_TILE_BLOCK * 4;
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
            let semi_transparent = object.is_semi_transparent();
            let priority = object.priority();
            let palette_bank = object.palette_bank();
            let palette_offset = palette_bank.unwrap_or(0) * 16;
            let use_8bpp = palette_bank.is_none();
            let base_tile_num = object.tile_num();
            let affine = object.affine_param_num();
            let object_y = y - top;

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
                // ALSO TODO: write to obj window

                // Find the pixel
                let (index_x, index_y) = if let Some(affine_param_num) = affine {
                    // TODO: affine shit
                    (0, 0)
                } else {
                    let index_x = if object.h_flip() {width - object_x - 1} else {object_x} as u8;
                    let index_y = if object.v_flip() {height - object_y - 1} else {object_y} as u8;
                    (index_x, index_y)
                };
                let tile_num = if use_1d_tile_mapping {
                    tile_num_for_coord_1d(base_tile_num, index_x, index_y, width as u8)
                } else {
                    tile_num_for_coord_2d(base_tile_num, index_x, index_y)
                };
                let tile_addr = OBJECT_VRAM_BASE + (tile_num * TILE_BYTES);
                let texel = if use_8bpp {
                    mem.vram.tile_texel_8bpp(tile_addr, index_x, index_y)
                } else {
                    mem.vram.tile_texel_4bpp(tile_addr, index_x, index_y)
                };
                // Transparent.
                if texel == 0 {
                    continue;
                }
                // Palette lookup.
                target[x as usize] = Some(ObjectPixel{
                    colour: palette_offset + texel, priority, semi_transparent
                });
            }
        }
    }

    /// Get the palette number of a background pixel.
    /// The x and y values provided should be scrolled & mosaiced already (i.e., background values and not screen values).
    /// 
    /// If 0 is returned, the pixel is transparent.
    fn tile_bg_pixel(&self, bg: &BackgroundData, vram: &VRAM, bg_x: usize, bg_y: usize) -> u8 {
        // TODO: Check if pixel is visible through window

        // Find tile attrs in bg map
        let map_x = bg_x / TILE_SIZE;
        let map_y = bg_y / TILE_SIZE;
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
        
        let mut tile_x = (bg_x % 8) as u8;
        let mut tile_y = (bg_y % 8) as u8;
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

    // TODO: draw affine bg layer

    // TODO: draw bitmap bg layer
}

// Internal: draw modes
impl SoftwareRenderer {
    fn draw_mode_0(&self, mem: &VideoMemory, target: &mut [u8], line: u16) {
        // Gather the backgrounds.
        let mut bg_prio_data = Vec::<BackgroundData>::new();
        for bg in (0..4).map(|bg_num| mem.registers.tile_bg_data(bg_num)) {
            if let Some(bg_data) = bg.as_ref() {
                for i in 0..bg_prio_data.len() {
                    if bg_data.priority < bg_prio_data[i].priority {
                        bg_prio_data.insert(i, bg_data.clone());
                    }
                }
                bg_prio_data.push(bg_data.clone());
            }
        }

        let mut obj_line = [None; gba::H_RES];
        let mut obj_window = [false; gba::H_RES];
        self.draw_obj_line(mem, &mut obj_line, &mut obj_window, line as i16);
        for x in 0..gba::H_RES {
            let dest = x * 4;
            // Prio 0
            let colour = self.eval_mode_0(mem, obj_line[x], &bg_prio_data, x, line as usize);
            target[dest] = colour.r;
            target[dest + 1] = colour.g;
            target[dest + 2] = colour.b;
        }
    }

    fn eval_mode_0(&self, mem: &VideoMemory, obj_pixel: Option<ObjectPixel>, bg_prio_data: &[BackgroundData], x: usize, y: usize) -> Colour {
        if let Some(obj) = obj_pixel {
            for priority in 0..4 {
                if obj.priority == priority {
                    return self.palette_cache.get_obj(obj.colour);
                } else {
                    for bg in bg_prio_data {
                        if bg.priority == priority {
                            // TODO: check window...
                            let scrolled_x = x.wrapping_add(bg.scroll_x as usize);
                            let scrolled_y = y.wrapping_add(bg.scroll_y as usize);
                            let texel = self.tile_bg_pixel(bg, &mem.vram, scrolled_x, scrolled_y);
                            if texel != 0 {
                                return self.palette_cache.get_bg(texel);
                            }
                        }
                    }
                }
            }
        } else {
            for priority in 0..4 {
                for bg in bg_prio_data {
                    if bg.priority == priority {
                        // TODO: check window...
                        let scrolled_x = x.wrapping_add(bg.scroll_x as usize);
                        let scrolled_y = y.wrapping_add(bg.scroll_y as usize);
                        let texel = self.tile_bg_pixel(bg, &mem.vram, scrolled_x, scrolled_y);
                        if texel != 0 {
                            return self.palette_cache.get_bg(texel);
                        }
                    }
                }
            }
        }
        self.palette_cache.get_backdrop()
    }
}

// Helpers: addr calculation
// TODO: maybe these should be moved?

/// Provided the coordinates into an object, get the offset from the base tile.
/// 
/// The returned value is the offset in units of tiles. Multiply this by 2 when using 8bpp.
fn tile_num_for_coord_2d(base_tile_num: usize, obj_x: u8, obj_y: u8) -> usize {
    const TILE_GRID_WIDTH: usize = 0x20;
    const TILE_GRID_HEIGHT: usize = 0x20;
    let base_tile_x = base_tile_num % TILE_GRID_WIDTH;
    let base_tile_y = base_tile_num / TILE_GRID_WIDTH;
    let tile_x = (obj_x / 8) as usize;
    let tile_y = (obj_y / 8) as usize;
    let target_tile_x = base_tile_x.wrapping_add(tile_x) % TILE_GRID_WIDTH;
    let target_tile_y = base_tile_y.wrapping_add(tile_y) % TILE_GRID_HEIGHT;
    target_tile_x + (target_tile_y * TILE_GRID_WIDTH)
}

/// Provided the coordinates into an object, and the base tile of the sprite,
/// get the tile number for the coords provided.
fn tile_num_for_coord_1d(base_tile_num: usize, obj_x: u8, obj_y: u8, width_x: u8) -> usize {
    let tile_width = (width_x / 8) as usize;
    let tile_x = (obj_x / 8) as usize;
    let tile_y = (obj_y / 8) as usize;
    base_tile_num + tile_x + (tile_y * tile_width)
}
