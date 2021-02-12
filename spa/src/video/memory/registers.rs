/// Video registers

use bitflags::bitflags;
use fixed::types::I24F8;
use crate::common::{
    bits::{
        u8, u16
    },
    bytes,
    meminterface::MemInterface16
};
use crate::interrupt::Interrupts;

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

/************* BG DATA ***************/

#[derive(Clone)]
pub enum BackgroundData {
    Tiled(TiledBackgroundData),
    Affine(AffineBackgroundData),
    Bitmap(BitmapBackgroundData)
}

impl BackgroundData {
    pub fn priority(&self) -> u8 {
        use BackgroundData::*;
        match self {
            Tiled(t) => t.priority,
            Affine(a) => a.priority,
            Bitmap(b) => b.priority
        }
    }
}

#[derive(Clone)]
pub enum BackgroundMapLayout {
    Small,  // 1x1 map
    Wide,   // 2x1 map
    Tall,   // 1x2 map
    Large   // 2x2 map
}

/// Data for a tiled background.
#[derive(Clone)]
pub struct TiledBackgroundData {
    pub priority:       u8,
    pub tile_map_addr:  u32,
    pub tile_data_addr: u32,
    pub use_8bpp:       bool,
    pub mosaic:         bool,

    pub scroll_x:   u16,
    pub scroll_y:   u16,
    pub layout:     BackgroundMapLayout,
}

/// Data for a tiled background.
#[derive(Clone)]
pub struct AffineBackgroundData {
    pub priority:       u8,
    pub tile_map_addr:  u32,
    pub tile_data_addr: u32,
    pub mosaic:         bool,

    pub bg_ref_point_x: I24F8,
    pub bg_ref_point_y: I24F8,
    pub matrix_a:       I24F8,
    pub matrix_b:       I24F8,
    pub matrix_c:       I24F8,
    pub matrix_d:       I24F8,
    pub wrap:           bool,
    pub size:           u32,
}

/// Data for a bitmap background.
#[derive(Clone)]
pub struct BitmapBackgroundData {
    pub priority:       u8,
    pub data_addr:      u32,
    pub use_15bpp:      bool,
    pub mosaic:         bool,
}

/************* BG DATA ***************/

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

#[derive(Default)]
pub struct VideoRegisters {
    lcd_control:    LCDControl,
    lcd_status:     LCDStatus,
    vcount:         u16,

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
    
    bg2_matrix_a:   u16,
    bg2_matrix_b:   u16,
    bg2_matrix_c:   u16,
    bg2_matrix_d:   u16,
    bg2_ref_x:      u32,
    bg2_ref_y:      u32,

    bg3_matrix_a:   u16,
    bg3_matrix_b:   u16,
    bg3_matrix_c:   u16,
    bg3_matrix_d:   u16,
    bg3_ref_x:      u32,
    bg3_ref_y:      u32,

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

    /// Set V-count.
    pub fn set_v_count(&mut self, count: u16) {
        self.vcount = count;
        self.lcd_status.set(LCDStatus::VCOUNT_FLAG, bytes::u16::lo(count) == bytes::u16::hi(self.lcd_status.bits()));
    }

    pub fn v_blank_irq(&self) -> Interrupts {
        if self.lcd_status.contains(LCDStatus::VBLANK_IRQ | LCDStatus::VBLANK_FLAG) {
            Interrupts::V_BLANK
        } else {
            Interrupts::default()
        }
    }

    pub fn h_blank_irq(&self) -> Interrupts {
        if self.lcd_status.contains(LCDStatus::HBLANK_IRQ | LCDStatus::HBLANK_FLAG)  {
            Interrupts::H_BLANK
        } else {
            Interrupts::default()
        }
    }

    pub fn v_count_irq(&self) -> Interrupts {
        if self.lcd_status.contains(LCDStatus::VCOUNT_IRQ | LCDStatus::VCOUNT_FLAG)  {
            Interrupts::V_COUNTER
        } else {
            Interrupts::default()
        }
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
                    if bg_data.priority() < backgrounds[i].priority() {
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
            3 => if let Some(bg_data) = self.get_bitmap_bg(0, true) {
                backgrounds.push(bg_data);
            },
            4 => if let Some(bg_data) = self.get_bitmap_bg(if self.bitmap_frame() {0x9600} else {0}, false) {
                backgrounds.push(bg_data);
            },
            5 => if let Some(bg_data) = self.get_bitmap_bg(if self.bitmap_frame() {0xA000} else {0}, true) {
                backgrounds.push(bg_data);
            },
            _ => unreachable!()
        }
        backgrounds
    }

    fn get_tiled_bg0(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG0) {
            Some(BackgroundData::Tiled(TiledBackgroundData {
                priority:       self.bg0_control.priority(),
                tile_map_addr:  self.bg0_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg0_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg0_control.use_8_bpp(),
                mosaic:         self.bg0_control.is_mosaic(),
                scroll_x:       self.bg0_x_offset,
                scroll_y:       self.bg0_y_offset,
                layout:         self.bg0_control.layout(),
            }))
        } else {
            None
        }
    }

    fn get_tiled_bg1(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG1) {
            Some(BackgroundData::Tiled(TiledBackgroundData {
                priority:       self.bg1_control.priority(),
                tile_map_addr:  self.bg1_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg1_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg1_control.use_8_bpp(),
                mosaic:         self.bg1_control.is_mosaic(),
                scroll_x:       self.bg1_x_offset,
                scroll_y:       self.bg1_y_offset,
                layout:         self.bg1_control.layout(),
            }))
        } else {
            None
        }
    }

    fn get_tiled_bg2(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG2) {
            Some(BackgroundData::Tiled(TiledBackgroundData {
                priority:       self.bg2_control.priority(),
                tile_map_addr:  self.bg2_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg2_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg2_control.use_8_bpp(),
                mosaic:         self.bg2_control.is_mosaic(),
                scroll_x:       self.bg2_x_offset,
                scroll_y:       self.bg2_y_offset,
                layout:         self.bg2_control.layout(),
            }))
        } else {
            None
        }
    }

    fn get_tiled_bg3(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG3) {
            Some(BackgroundData::Tiled(TiledBackgroundData {
                priority:       self.bg3_control.priority(),
                tile_map_addr:  self.bg3_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg3_control.tile_data_block() * 16 * 1024,
                use_8bpp:       self.bg3_control.use_8_bpp(),
                mosaic:         self.bg3_control.is_mosaic(),
                scroll_x:       self.bg3_x_offset,
                scroll_y:       self.bg3_y_offset,
                layout:         self.bg3_control.layout(),
            }))
        } else {
            None
        }
    }

    fn get_affine_bg2(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG2) {
            Some(BackgroundData::Affine(AffineBackgroundData {
                priority:       self.bg2_control.priority(),
                tile_map_addr:  self.bg2_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg2_control.tile_data_block() * 16 * 1024,
                mosaic:         self.bg2_control.is_mosaic(),
                bg_ref_point_x: I24F8::from_bits(self.bg2_ref_x as i32),
                bg_ref_point_y: I24F8::from_bits(self.bg2_ref_y as i32),
                matrix_a:       I24F8::from_bits((self.bg2_matrix_a as i16) as i32),
                matrix_b:       I24F8::from_bits((self.bg2_matrix_b as i16) as i32),
                matrix_c:       I24F8::from_bits((self.bg2_matrix_c as i16) as i32),
                matrix_d:       I24F8::from_bits((self.bg2_matrix_d as i16) as i32),
                wrap:           self.bg2_control.affine_wraparound(),
                size:           self.bg2_control.affine_size(),
            }))
        } else {
            None
        }
    }

    fn get_affine_bg3(&self) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG3) {
            Some(BackgroundData::Affine(AffineBackgroundData {
                priority:       self.bg3_control.priority(),
                tile_map_addr:  self.bg3_control.tile_map_block() * 2 * 1024,
                tile_data_addr: self.bg3_control.tile_data_block() * 16 * 1024,
                mosaic:         self.bg3_control.is_mosaic(),
                bg_ref_point_x: I24F8::from_bits(self.bg3_ref_x as i32),
                bg_ref_point_y: I24F8::from_bits(self.bg3_ref_y as i32),
                matrix_a:       I24F8::from_bits((self.bg3_matrix_a as i16) as i32),
                matrix_b:       I24F8::from_bits((self.bg3_matrix_b as i16) as i32),
                matrix_c:       I24F8::from_bits((self.bg3_matrix_c as i16) as i32),
                matrix_d:       I24F8::from_bits((self.bg3_matrix_d as i16) as i32),
                wrap:           self.bg3_control.affine_wraparound(),
                size:           self.bg3_control.affine_size(),
            }))
        } else {
            None
        }
    }

    fn get_bitmap_bg(&self, offset: u32, use_15bpp: bool) -> Option<BackgroundData> {
        if self.lcd_control.contains(LCDControl::DISPLAY_BG2) {
            Some(BackgroundData::Bitmap(BitmapBackgroundData {
                priority:   self.bg2_control.priority(),
                data_addr:  offset,
                use_15bpp:  use_15bpp,
                mosaic:     self.bg2_control.is_mosaic(),
            }))
        } else {
            None
        }
    }

    pub fn is_obj_enabled(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_OBJ)
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

    pub fn obj_window_0(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_WIN0) && self.win0_inside.contains(WindowControl::OBJ_ENABLE)
    }

    /// Check if window 0 should be used for this line.
    pub fn y_inside_window_0(&self, y: u8) -> bool {
        y >= self.win0_y_top && y < self.win0_y_bottom
    }

    pub fn x_inside_window_0(&self, x: u8) -> bool {
        x >= self.win0_x_left && x < self.win0_x_right
    }

    pub fn obj_window_1(&self) -> bool {
        self.lcd_control.contains(LCDControl::DISPLAY_WIN1) && self.win1_inside.contains(WindowControl::OBJ_ENABLE)
    }

    /// Check if window 0 should be used for this line.
    pub fn y_inside_window_1(&self, y: u8) -> bool {
        y >= self.win1_y_top && y < self.win1_y_bottom
    }

    pub fn x_inside_window_1(&self, x: u8) -> bool {
        x >= self.win1_x_left && x < self.win1_x_right
    }
}

impl MemInterface16 for VideoRegisters {
    fn read_halfword(&self, addr: u32) -> u16 {
        match addr {
            0x0 => self.lcd_control.bits(),
            0x2 => 0, // TODO: green swap
            0x4 => self.lcd_status.bits(),
            0x6 => self.vcount,
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
            0x20 => self.bg2_matrix_a = data,
            0x22 => self.bg2_matrix_b = data,
            0x24 => self.bg2_matrix_c = data,
            0x26 => self.bg2_matrix_d = data,
            0x28 => self.bg2_ref_x = bytes::u32::set_lo(self.bg2_ref_x, data),
            0x2A => self.bg2_ref_x = bytes::u32::set_hi(self.bg2_ref_x, sign_extend_12bit(data & 0xFFF)),
            0x2C => self.bg2_ref_y = bytes::u32::set_lo(self.bg2_ref_y, data),
            0x2E => self.bg2_ref_y = bytes::u32::set_hi(self.bg2_ref_y, sign_extend_12bit(data & 0xFFF)),
            0x30 => self.bg3_matrix_a = data,
            0x32 => self.bg3_matrix_b = data,
            0x34 => self.bg3_matrix_c = data,
            0x36 => self.bg3_matrix_d = data,
            0x38 => self.bg3_ref_x = bytes::u32::set_lo(self.bg3_ref_x, data),
            0x3A => self.bg3_ref_x = bytes::u32::set_hi(self.bg3_ref_x, sign_extend_12bit(data & 0xFFF)),
            0x3C => self.bg3_ref_y = bytes::u32::set_lo(self.bg3_ref_y, data),
            0x3E => self.bg3_ref_y = bytes::u32::set_hi(self.bg3_ref_y, sign_extend_12bit(data & 0xFFF)),
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

fn sign_extend_12bit(val: u16) -> u16 {
    if u16::test_bit(val, 11) {
        val | 0xF000
    } else {
        val
    }
}