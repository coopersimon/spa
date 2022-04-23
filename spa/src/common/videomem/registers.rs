/// Video registers

use bitflags::bitflags;
use fixed::types::I24F8;
use crate::utils::{
    bits::{
        u8, u16
    },
    bytes,
    meminterface::MemInterface16
};
use crate::common::drawing::background::*;

bitflags! {
    #[derive(Default)]
    struct LCDControl: u16 {
        const DISPLAY_OBJ_WIN   = u16::bit(15);
        const DISPLAY_WIN1      = u16::bit(14);
        const DISPLAY_WIN0      = u16::bit(13);
        const DISPLAY_OBJ       = u16::bit(12);
        const DISPLAY_BG3       = u16::bit(11);
        const DISPLAY_BG2       = u16::bit(10);
        const DISPLAY_BG1       = u16::bit(9);
        const DISPLAY_BG0       = u16::bit(8);
        const FORCED_BLANK      = u16::bit(7);
        const OBJ_TILE_MAP      = u16::bit(6);
        const HBLANK_INTERVAL   = u16::bit(5);
        const FRAME_DISPLAY     = u16::bit(4);
        const CGB_MODE          = u16::bit(3);
        const MODE              = u16::bits(0, 2);
    }
}

bitflags! {
    #[derive(Default)]
    struct LCDStatus: u16 {
        const VCOUNT        = u16::bits(8, 15);
        const VCOUNT_IRQ    = u16::bit(5);
        const HBLANK_IRQ    = u16::bit(4);
        const VBLANK_IRQ    = u16::bit(3);
        const VCOUNT_FLAG   = u16::bit(2);
        const HBLANK_FLAG   = u16::bit(1);
        const VBLANK_FLAG   = u16::bit(0);
    }
}

impl LCDStatus {
    fn get_flags(self) -> LCDStatus {
        self & (LCDStatus::VBLANK_FLAG | LCDStatus::HBLANK_FLAG | LCDStatus::VCOUNT_FLAG)
    }
}

bitflags! {
    #[derive(Default)]
    struct BGControl: u16 {
        const SCREEN_SIZE   = u16::bits(14, 15);
        const OVERFLOW      = u16::bit(13);
        const MAP_BASE      = u16::bits(8, 12);
        const USE_8_BPP     = u16::bit(7);
        const MOSAIC        = u16::bit(6);
        const TILE_BASE     = u16::bits(2, 3);
        const PRIORITY      = u16::bits(0, 1);
    }
}

impl BGControl {
    fn priority(self) -> u8 {
        (self & BGControl::PRIORITY).bits() as u8
    }

    fn tile_data_block(self) -> u32 {
        ((self & BGControl::TILE_BASE).bits() >> 2) as u32
    }

    fn is_mosaic(self) -> bool {
        self.contains(BGControl::MOSAIC)
    }

    fn use_8_bpp(self) -> bool {
        self.contains(BGControl::USE_8_BPP)
    }

    fn tile_map_block(self) -> u32 {
        ((self & BGControl::MAP_BASE).bits() >> 8) as u32
    }

    fn affine_wraparound(self) -> bool {
        self.contains(BGControl::OVERFLOW)
    }

    fn layout(self) -> BackgroundMapLayout {
        const SMALL: u16 = 0 << 14;
        const WIDE: u16 = 1 << 14;
        const TALL: u16 = 2 << 14;
        const LARGE: u16 = 3 << 14;
        match (self & BGControl::SCREEN_SIZE).bits() {
            SMALL => BackgroundMapLayout::Small,
            WIDE => BackgroundMapLayout::Wide,
            TALL => BackgroundMapLayout::Tall,
            LARGE => BackgroundMapLayout::Large,
            _ => unreachable!()
        }
    }

    fn affine_size(self) -> u32 {
        const SMALL: u16 = 0 << 14;
        const MID: u16 = 1 << 14;
        const LARGE: u16 = 2 << 14;
        const XLARGE: u16 = 3 << 14;
        match (self & BGControl::SCREEN_SIZE).bits() {
            SMALL => 128,
            MID => 256,
            LARGE => 512,
            XLARGE => 1024,
            _ => unreachable!()
        }
    }
}

bitflags! {
    #[derive(Default)]
    struct WindowControl: u8 {
        const COLOUR_SPECIAL    = u8::bit(5);
        const OBJ_ENABLE        = u8::bit(4);
        const BG3_ENABLE        = u8::bit(3);
        const BG2_ENABLE        = u8::bit(2);
        const BG1_ENABLE        = u8::bit(1);
        const BG0_ENABLE        = u8::bit(0);
    }
}

bitflags! {
    #[derive(Default)]
    struct ColourSpecialControl: u16 {
        const BD_TARGET_2   = u16::bit(13);
        const OBJ_TARGET_2  = u16::bit(12);
        const BG3_TARGET_2  = u16::bit(11);
        const BG2_TARGET_2  = u16::bit(10);
        const BG1_TARGET_2  = u16::bit(9);
        const BG0_TARGET_2  = u16::bit(8);
        const EFFECT        = u16::bits(6, 7);
        const BD_TARGET_1   = u16::bit(5);
        const OBJ_TARGET_1  = u16::bit(4);
        const BG3_TARGET_1  = u16::bit(3);
        const BG2_TARGET_1  = u16::bit(2);
        const BG1_TARGET_1  = u16::bit(1);
        const BG0_TARGET_1  = u16::bit(0);
    }
}

impl Into<ColourEffect> for ColourSpecialControl {
    fn into(self) -> ColourEffect {
        const NO_EFFECT: u16 = 0 << 6;
        const BLEND: u16 = 1 << 6;
        const BRIGHTEN: u16 = 2 << 6;
        const DARKEN: u16 = 3 << 6;
        match (self & ColourSpecialControl::EFFECT).bits() {
            NO_EFFECT => ColourEffect::None,
            BLEND => ColourEffect::AlphaBlend,
            BRIGHTEN => ColourEffect::Brighten,
            DARKEN => ColourEffect::Darken,
            _ => unreachable!()
        }
    }
}

#[derive(Default)]
pub struct VideoRegisters {
    lcd_control:    LCDControl,
    lcd_status:     LCDStatus,
    vcount:         u8,

    bg0_control:    BGControl,
    bg1_control:    BGControl,
    bg2_control:    BGControl,
    bg3_control:    BGControl,

    bg0_x_offset:   u16,
    bg0_y_offset:   u16,
    bg1_x_offset:   u16,
    bg1_y_offset:   u16,
    bg2_x_offset:   u16,
    bg2_y_offset:   u16,
    bg3_x_offset:   u16,
    bg3_y_offset:   u16,
    
    // Public affine regs
    bg2_matrix_a:   u16,
    bg2_matrix_b:   u16,
    bg2_matrix_c:   u16,
    bg2_matrix_d:   u16,
    bg2_ref_x:      u32,
    bg2_ref_y:      u32,
    // Internal affine regs
    bg2_internal_a: I24F8,
    bg2_internal_b: I24F8,
    bg2_internal_c: I24F8,
    bg2_internal_d: I24F8,
    bg2_internal_x: I24F8,
    bg2_internal_y: I24F8,

    // Public affine regs
    bg3_matrix_a:   u16,
    bg3_matrix_b:   u16,
    bg3_matrix_c:   u16,
    bg3_matrix_d:   u16,
    bg3_ref_x:      u32,
    bg3_ref_y:      u32,
    // Internal affine regs
    bg3_internal_a: I24F8,
    bg3_internal_b: I24F8,
    bg3_internal_c: I24F8,
    bg3_internal_d: I24F8,
    bg3_internal_x: I24F8,
    bg3_internal_y: I24F8,

    win0_x_right:   u8,
    win0_x_left:    u8,
    win1_x_right:   u8,
    win1_x_left:    u8,

    win0_y_bottom:  u8,
    win0_y_top:     u8,
    win1_y_bottom:  u8,
    win1_y_top:     u8,

    win0_inside:    WindowControl,
    win1_inside:    WindowControl,
    win_outside:    WindowControl,
    win_obj_inside: WindowControl,

    mosaic:         u16,
    colour_special: ColourSpecialControl,
    alpha_coeffs:   u16,
    brightness:     u8,
}

impl VideoRegisters {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set V-blank.
    pub fn set_v_blank(&mut self, enable: bool) {
        self.lcd_status.set(LCDStatus::VBLANK_FLAG, enable);
    }

    /// Set H-blank.
    pub fn set_h_blank(&mut self, enable: bool) {
        self.lcd_status.set(LCDStatus::HBLANK_FLAG, enable);
    }

    /// Increment v-count by one.
    pub fn inc_v_count(&mut self) {
        self.vcount += 1;
        self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.vcount == bytes::u16::hi(self.lcd_status.bits()));
        // Increment internal x & y affine transform offset points.
        // This is important for HDMA-based "mode 7" effects with affine backgrounds.
        self.bg2_internal_x += self.bg2_internal_b;
        self.bg2_internal_y += self.bg2_internal_d;
        self.bg3_internal_x += self.bg3_internal_b;
        self.bg3_internal_y += self.bg3_internal_d;
    }

    /// Reset v-count to zero.
    pub fn reset_v_count(&mut self) {
        self.vcount = 0;
        self.lcd_status.set(LCDStatus::VCOUNT_FLAG, self.vcount == bytes::u16::hi(self.lcd_status.bits()));
        self.bg2_internal_x = I24F8::from_bits(self.bg2_ref_x as i32);
        self.bg2_internal_y = I24F8::from_bits(self.bg2_ref_y as i32);
        self.bg3_internal_x = I24F8::from_bits(self.bg3_ref_x as i32);
        self.bg3_internal_y = I24F8::from_bits(self.bg3_ref_y as i32);
    }

    pub fn v_blank_irq(&self) -> bool {
        self.lcd_status.contains(LCDStatus::VBLANK_IRQ | LCDStatus::VBLANK_FLAG)
    }

    pub fn h_blank_irq(&self) -> bool {
        self.lcd_status.contains(LCDStatus::HBLANK_IRQ | LCDStatus::HBLANK_FLAG)
    }

    pub fn v_count_irq(&self) -> bool {
        self.lcd_status.contains(LCDStatus::VCOUNT_IRQ | LCDStatus::VCOUNT_FLAG)
    }
}

// Render-side interface.
impl VideoRegisters {
    pub fn in_fblank(&self) -> bool {
        self.lcd_control.contains(LCDControl::FORCED_BLANK)
    }

    pub fn mode(&self) -> u16 {
        (self.lcd_control & LCDControl::MODE).bits()
    }

    pub fn is_obj_enabled(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_OBJ)
    }

    fn bitmap_frame(&self) -> bool {
        self.lcd_control.contains(LCDControl::FRAME_DISPLAY)
    }

    /// Get background data for the current mode.
    /// 
    /// Will return data for each enabled background in the current mode,
    /// in priority order (high-low)
    pub fn bg_data_for_mode(&self) -> Vec<BackgroundData> {
        let mut backgrounds = Vec::<BackgroundData>::new();
        let mut insert = |bg: Option<BackgroundData>| {
            if let Some(bg_data) = bg {
                for i in 0..backgrounds.len() {
                    if bg_data.priority < backgrounds[i].priority {
                        backgrounds.insert(i, bg_data);
                        return;
                    }
                }
                backgrounds.push(bg_data);
            }
        };
        match self.mode() {
            0 => {
                insert(self.get_tiled_bg0());
                insert(self.get_tiled_bg1());
                insert(self.get_tiled_bg2());
                insert(self.get_tiled_bg3());
            },
            1 => {
                insert(self.get_tiled_bg0());
                insert(self.get_tiled_bg1());
                insert(self.get_affine_bg2());
            },
            2 => {
                insert(self.get_affine_bg2());
                insert(self.get_affine_bg3());
            },
            3 => if let Some(bg_data) = self.get_bitmap_bg(0, true, false) {
                backgrounds.push(bg_data);
            },
            4 => if let Some(bg_data) = self.get_bitmap_bg(if self.bitmap_frame() {0xA000} else {0}, false, false) {
                backgrounds.push(bg_data);
            },
            5 => if let Some(bg_data) = self.get_bitmap_bg(if self.bitmap_frame() {0xA000} else {0}, true, true) {
                backgrounds.push(bg_data);
            },
            _ => unreachable!()
        }
        backgrounds
    }

    fn get_tiled_bg0(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG0) {
            let tiled_data = TiledBackgroundData {
                tile_map_addr:  self.bg0_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg0_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg0_control.use_8_bpp(),
                scroll_x:       self.bg0_x_offset,
                scroll_y:       self.bg0_y_offset,
                layout:         self.bg0_control.layout(),
            };
            Some(BackgroundData {
                priority:       self.bg0_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG0_ENABLE),
                    self.win1_inside.contains(WindowControl::BG0_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG0_ENABLE),
                    self.win_outside.contains(WindowControl::BG0_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG0_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG0_TARGET_2)
                ),
                mosaic:     self.bg0_control.is_mosaic(),
                type_data:  BackgroundTypeData::Tiled(tiled_data)
            })
        } else {
            None
        }
    }

    fn get_tiled_bg1(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG1) {
            let tiled_data = TiledBackgroundData {
                tile_map_addr:  self.bg1_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg1_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg1_control.use_8_bpp(),
                scroll_x:       self.bg1_x_offset,
                scroll_y:       self.bg1_y_offset,
                layout:         self.bg1_control.layout(),
            };
            Some(BackgroundData {
                priority:       self.bg1_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG1_ENABLE),
                    self.win1_inside.contains(WindowControl::BG1_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG1_ENABLE),
                    self.win_outside.contains(WindowControl::BG1_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG1_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG1_TARGET_2)
                ),
                mosaic:     self.bg1_control.is_mosaic(),
                type_data:  BackgroundTypeData::Tiled(tiled_data)
            })
        } else {
            None
        }
    }

    fn get_tiled_bg2(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG2) {
            let tiled_data = TiledBackgroundData {
                tile_map_addr:  self.bg2_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg2_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg2_control.use_8_bpp(),
                scroll_x:       self.bg2_x_offset,
                scroll_y:       self.bg2_y_offset,
                layout:         self.bg2_control.layout(),
            };
            Some(BackgroundData {
                priority:       self.bg2_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG2_ENABLE),
                    self.win1_inside.contains(WindowControl::BG2_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG2_ENABLE),
                    self.win_outside.contains(WindowControl::BG2_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG2_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG2_TARGET_2)
                ),
                mosaic:     self.bg2_control.is_mosaic(),
                type_data:  BackgroundTypeData::Tiled(tiled_data)
            })
        } else {
            None
        }
    }

    fn get_tiled_bg3(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG3) {
            let tiled_data = TiledBackgroundData {
                tile_map_addr:  self.bg3_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg3_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg3_control.use_8_bpp(),
                scroll_x:       self.bg3_x_offset,
                scroll_y:       self.bg3_y_offset,
                layout:         self.bg3_control.layout(),
            };
            Some(BackgroundData {
                priority:       self.bg3_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG3_ENABLE),
                    self.win1_inside.contains(WindowControl::BG3_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG3_ENABLE),
                    self.win_outside.contains(WindowControl::BG3_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG3_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG3_TARGET_2)
                ),
                mosaic:     self.bg3_control.is_mosaic(),
                type_data:  BackgroundTypeData::Tiled(tiled_data)
            })
        } else {
            None
        }
    }

    fn get_affine_bg2(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG2) {
            let affine_data = AffineBackgroundData {
                tile_map_addr:  self.bg2_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg2_control.tile_data_block() * 16 * 1024,
                bg_ref_point_x: self.bg2_internal_x,
                bg_ref_point_y: self.bg2_internal_y,
                matrix_a:       self.bg2_internal_a,
                matrix_b:       self.bg2_internal_b,
                matrix_c:       self.bg2_internal_c,
                matrix_d:       self.bg2_internal_d,
                wrap:           self.bg2_control.affine_wraparound(),
                size:           self.bg2_control.affine_size(),
            };
            Some(BackgroundData {
                priority:       self.bg2_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG2_ENABLE),
                    self.win1_inside.contains(WindowControl::BG2_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG2_ENABLE),
                    self.win_outside.contains(WindowControl::BG2_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG2_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG2_TARGET_2)
                ),
                mosaic:     self.bg2_control.is_mosaic(),
                type_data:  BackgroundTypeData::Affine(affine_data)
            })
        } else {
            None
        }
    }

    fn get_affine_bg3(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG3) {
            let affine_data = AffineBackgroundData {
                tile_map_addr:  self.bg3_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg3_control.tile_data_block() * 16 * 1024,
                bg_ref_point_x: self.bg3_internal_x,
                bg_ref_point_y: self.bg3_internal_y,
                matrix_a:       self.bg3_internal_a,
                matrix_b:       self.bg3_internal_b,
                matrix_c:       self.bg3_internal_c,
                matrix_d:       self.bg3_internal_d,
                wrap:           self.bg3_control.affine_wraparound(),
                size:           self.bg3_control.affine_size(),
            };
            Some(BackgroundData {
                priority:       self.bg3_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG3_ENABLE),
                    self.win1_inside.contains(WindowControl::BG3_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG3_ENABLE),
                    self.win_outside.contains(WindowControl::BG3_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG3_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG3_TARGET_2)
                ),
                mosaic:     self.bg3_control.is_mosaic(),
                type_data:  BackgroundTypeData::Affine(affine_data)
            })
        } else {
            None
        }
    }

    fn get_bitmap_bg(&self, offset: u32, use_15bpp: bool, small: bool) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG2) {
            let bitmap_data = BitmapBackgroundData {
                data_addr:  offset,
                use_15bpp:  use_15bpp,
                small:      small,
            };
            Some(BackgroundData {
                priority:       self.bg2_control.priority(),
                window_mask:    WindowMask::make(
                    self.win0_inside.contains(WindowControl::BG2_ENABLE),
                    self.win1_inside.contains(WindowControl::BG2_ENABLE),
                    self.win_obj_inside.contains(WindowControl::BG2_ENABLE),
                    self.win_outside.contains(WindowControl::BG2_ENABLE)
                ),
                blend_mask:     BlendMask::make(
                    self.colour_special.contains(ColourSpecialControl::BG2_TARGET_1) && self.use_blend_layer_1(),
                    self.colour_special.contains(ColourSpecialControl::BG2_TARGET_2)
                ),
                mosaic:     self.bg2_control.is_mosaic(),
                type_data:  BackgroundTypeData::Bitmap(bitmap_data)
            })
        } else {
            None
        }
    }

    /// Returns true if tiles should map in 1D.
    /// Returns false if tiles should map in 2D.
    pub fn obj_1d_tile_mapping(&self) -> bool {
        self.lcd_control.contains(LCDControl::OBJ_TILE_MAP)
    }

    // Windows
    pub fn windows_enabled(&self) -> bool {
        self.lcd_control.intersects(LCDControl::DISPLAY_WIN0 | LCDControl::DISPLAY_WIN1 | LCDControl::DISPLAY_OBJ_WIN)
    }
    pub fn window_0_enabled(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_WIN0)
    }
    pub fn window_1_enabled(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_WIN1)
    }
    pub fn window_obj_enabled(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_OBJ_WIN)
    }

    pub fn obj_window_mask(&self) -> WindowMask {
        WindowMask::make(
            self.win0_inside.contains(WindowControl::OBJ_ENABLE),
            self.win1_inside.contains(WindowControl::OBJ_ENABLE),
            self.win_obj_inside.contains(WindowControl::OBJ_ENABLE),
            self.win_outside.contains(WindowControl::OBJ_ENABLE)
        )
    }

    pub fn colour_window_mask(&self) -> WindowMask {
        WindowMask::make(
            self.win0_inside.contains(WindowControl::COLOUR_SPECIAL),
            self.win1_inside.contains(WindowControl::COLOUR_SPECIAL),
            self.win_obj_inside.contains(WindowControl::COLOUR_SPECIAL),
            self.win_outside.contains(WindowControl::COLOUR_SPECIAL)
        )
    }

    /// Check if window 0 should be used for this line.
    pub fn y_inside_window_0(&self, y: u8) -> bool {
        y >= self.win0_y_top && y < self.win0_y_bottom
    }
    pub fn x_inside_window_0(&self, x: u8) -> bool {
        x >= self.win0_x_left && x < self.win0_x_right
    }
    /// Check if window 0 should be used for this line.
    pub fn y_inside_window_1(&self, y: u8) -> bool {
        y >= self.win1_y_top && y < self.win1_y_bottom
    }
    pub fn x_inside_window_1(&self, x: u8) -> bool {
        x >= self.win1_x_left && x < self.win1_x_right
    }

    // Colour special effects
    fn use_blend_layer_1(&self) -> bool {
        self.colour_special.intersects(ColourSpecialControl::EFFECT)
    }
    pub fn colour_effect(&self) -> ColourEffect {
        self.colour_special.into()
    }
    pub fn obj_blend_mask(&self) -> BlendMask {
        BlendMask::make(
            self.colour_special.contains(ColourSpecialControl::OBJ_TARGET_1),
            self.colour_special.contains(ColourSpecialControl::OBJ_TARGET_2)
        )
    }
    pub fn backdrop_blend_mask(&self) -> BlendMask {
        BlendMask::make(
            self.colour_special.contains(ColourSpecialControl::BD_TARGET_1),
            self.colour_special.contains(ColourSpecialControl::BD_TARGET_2)
        )
    }
    
    pub fn get_alpha_coeffs(&self) -> (u16, u16) {
        let eva = self.alpha_coeffs & 0x1F;
        let evb = (self.alpha_coeffs >> 8) & 0x1F;
        (std::cmp::min(0x10, eva), std::cmp::min(0x10, evb))
    }
    pub fn get_brightness_coeff(&self) -> u16 {
        let evy = (self.brightness & 0x1F) as u16;
        std::cmp::min(0x10, evy)
    }

    // Mosaic
    pub fn bg_mosaic_x(&self) -> u8 {
        let mosaic = (self.mosaic & 0xF) as u8;
        mosaic + 1
    }
    pub fn bg_mosaic_y(&self) -> u8 {
        let mosaic = ((self.mosaic >> 4) & 0xF) as u8;
        mosaic + 1
    }

    pub fn obj_mosaic_x(&self) -> u8 {
        let mosaic = ((self.mosaic >> 8) & 0xF) as u8;
        mosaic + 1
    }
    pub fn obj_mosaic_y(&self) -> u8 {
        let mosaic = ((self.mosaic >> 12) & 0xF) as u8;
        mosaic + 1
    }
}

impl MemInterface16 for VideoRegisters {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0 => self.lcd_control.bits(),
            0x2 => 0, // TODO: green swap
            0x4 => self.lcd_status.bits(),
            0x6 => self.vcount as u16,
            0x8 => self.bg0_control.bits(),
            0xA => self.bg1_control.bits(),
            0xC => self.bg2_control.bits(),
            0xE => self.bg3_control.bits(),
            0x10 => self.bg0_x_offset,
            0x12 => self.bg0_y_offset,
            0x14 => self.bg1_x_offset,
            0x16 => self.bg1_y_offset,
            0x18 => self.bg2_x_offset,
            0x1A => self.bg2_y_offset,
            0x1C => self.bg3_x_offset,
            0x1E => self.bg3_y_offset,
            0x20 => self.bg2_matrix_a,
            0x22 => self.bg2_matrix_b,
            0x24 => self.bg2_matrix_c,
            0x26 => self.bg2_matrix_d,
            0x28 => bytes::u32::lo(self.bg2_ref_x),
            0x2A => bytes::u32::hi(self.bg2_ref_x),
            0x2C => bytes::u32::lo(self.bg2_ref_y),
            0x2E => bytes::u32::hi(self.bg2_ref_y),
            0x30 => self.bg3_matrix_a,
            0x32 => self.bg3_matrix_b,
            0x34 => self.bg3_matrix_c,
            0x36 => self.bg3_matrix_d,
            0x38 => bytes::u32::lo(self.bg3_ref_x),
            0x3A => bytes::u32::hi(self.bg3_ref_x),
            0x3C => bytes::u32::lo(self.bg3_ref_y),
            0x3E => bytes::u32::hi(self.bg3_ref_y),
            0x40 => bytes::u16::make(self.win0_x_left, self.win0_x_right),
            0x42 => bytes::u16::make(self.win1_x_left, self.win1_x_right),
            0x44 => bytes::u16::make(self.win0_y_top, self.win0_y_bottom),
            0x46 => bytes::u16::make(self.win1_y_top, self.win1_y_bottom),
            0x48 => bytes::u16::make(self.win1_inside.bits(), self.win0_inside.bits()),
            0x4A => bytes::u16::make(self.win_obj_inside.bits(), self.win_outside.bits()),
            0x4C => self.mosaic,
            0x4E => 0,
            0x50 => self.colour_special.bits(),
            0x52 => self.alpha_coeffs,
            0x54 => self.brightness as u16,
            0x56 => 0,
            _ => panic!("reading from invalid video register address {:X}", addr)
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0 => self.lcd_control = LCDControl::from_bits_truncate(data),
            0x2 => {}, // TODO: green swap
            0x4 => self.set_lcd_status(data),
            0x6 => {},
            0x8 => self.bg0_control = BGControl::from_bits_truncate(data),
            0xA => self.bg1_control = BGControl::from_bits_truncate(data),
            0xC => self.bg2_control = BGControl::from_bits_truncate(data),
            0xE => self.bg3_control = BGControl::from_bits_truncate(data),
            0x10 => self.bg0_x_offset = data,
            0x12 => self.bg0_y_offset = data,
            0x14 => self.bg1_x_offset = data,
            0x16 => self.bg1_y_offset = data,
            0x18 => self.bg2_x_offset = data,
            0x1A => self.bg2_y_offset = data,
            0x1C => self.bg3_x_offset = data,
            0x1E => self.bg3_y_offset = data,
            0x20 => {
                self.bg2_matrix_a = data;
                self.bg2_internal_a = I24F8::from_bits((self.bg2_matrix_a as i16) as i32);
            },
            0x22 => {
                self.bg2_matrix_b = data;
                self.bg2_internal_b = I24F8::from_bits((self.bg2_matrix_b as i16) as i32);
            },
            0x24 => {
                self.bg2_matrix_c = data;
                self.bg2_internal_c = I24F8::from_bits((self.bg2_matrix_c as i16) as i32);
            },
            0x26 => {
                self.bg2_matrix_d = data;
                self.bg2_internal_d = I24F8::from_bits((self.bg2_matrix_d as i16) as i32);
            },
            0x28 => {
                self.bg2_ref_x = bytes::u32::set_lo(self.bg2_ref_x, data);
                self.bg2_internal_x = I24F8::from_bits(self.bg2_ref_x as i32);
            },
            0x2A => {
                self.bg2_ref_x = bytes::u32::set_hi(self.bg2_ref_x, sign_extend_12bit(data & 0xFFF));
                self.bg2_internal_x = I24F8::from_bits(self.bg2_ref_x as i32);
            },
            0x2C => {
                self.bg2_ref_y = bytes::u32::set_lo(self.bg2_ref_y, data);
                self.bg2_internal_y = I24F8::from_bits(self.bg2_ref_y as i32);
            },
            0x2E => {
                self.bg2_ref_y = bytes::u32::set_hi(self.bg2_ref_y, sign_extend_12bit(data & 0xFFF));
                self.bg2_internal_y = I24F8::from_bits(self.bg2_ref_y as i32);
            },
            0x30 => {
                self.bg3_matrix_a = data;
                self.bg3_internal_a = I24F8::from_bits((self.bg3_matrix_a as i16) as i32);
            },
            0x32 => {
                self.bg3_matrix_b = data;
                self.bg3_internal_b = I24F8::from_bits((self.bg3_matrix_b as i16) as i32);
            },
            0x34 => {
                self.bg3_matrix_c = data;
                self.bg3_internal_c = I24F8::from_bits((self.bg3_matrix_c as i16) as i32);
            },
            0x36 => {
                self.bg3_matrix_d = data;
                self.bg3_internal_d = I24F8::from_bits((self.bg3_matrix_d as i16) as i32);
            },
            0x38 => {
                self.bg3_ref_x = bytes::u32::set_lo(self.bg3_ref_x, data);
                self.bg3_internal_x = I24F8::from_bits(self.bg3_ref_x as i32);
            },
            0x3A => {
                self.bg3_ref_x = bytes::u32::set_hi(self.bg3_ref_x, sign_extend_12bit(data & 0xFFF));
                self.bg3_internal_x = I24F8::from_bits(self.bg3_ref_x as i32);
            },
            0x3C => {
                self.bg3_ref_y = bytes::u32::set_lo(self.bg3_ref_y, data);
                self.bg3_internal_y = I24F8::from_bits(self.bg3_ref_y as i32);
            },
            0x3E => {
                self.bg3_ref_y = bytes::u32::set_hi(self.bg3_ref_y, sign_extend_12bit(data & 0xFFF));
                self.bg3_internal_y = I24F8::from_bits(self.bg3_ref_y as i32);
            },
            0x40 => {
                self.win0_x_right = bytes::u16::lo(data);
                self.win0_x_left = bytes::u16::hi(data);
            },
            0x42 => {
                self.win1_x_right = bytes::u16::lo(data);
                self.win1_x_left = bytes::u16::hi(data);
            },
            0x44 => {
                self.win0_y_bottom = bytes::u16::lo(data);
                self.win0_y_top = bytes::u16::hi(data);
            },
            0x46 => {
                self.win1_y_bottom = bytes::u16::lo(data);
                self.win1_y_top = bytes::u16::hi(data);
            },
            0x48 => {
                self.win0_inside = WindowControl::from_bits_truncate(bytes::u16::lo(data));
                self.win1_inside = WindowControl::from_bits_truncate(bytes::u16::hi(data));
            },
            0x4A => {
                self.win_outside = WindowControl::from_bits_truncate(bytes::u16::lo(data));
                self.win_obj_inside = WindowControl::from_bits_truncate(bytes::u16::hi(data));
            },
            0x4C => self.mosaic = data,
            0x4E => {},
            0x50 => self.colour_special = ColourSpecialControl::from_bits_truncate(data),
            0x52 => self.alpha_coeffs = data,
            0x54 => self.brightness = bytes::u16::lo(data),
            0x56 => {},
            _ => panic!("writing to invalid video register address {:X}", addr)
        }
    }
}

// Internal
impl VideoRegisters {
    fn set_lcd_status(&mut self, data: u16) {
        let old_flags = self.lcd_status.get_flags();
        let lcd_status = LCDStatus::from_bits_truncate(data & 0xFFF8);
        self.lcd_status = lcd_status | old_flags;
    }
}

#[inline]
const fn sign_extend_12bit(val: u16) -> u16 {
    if u16::test_bit(val, 11) {
        val | 0xF000
    } else {
        val
    }
}