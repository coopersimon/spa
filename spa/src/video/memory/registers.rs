/// Video registers

use bitflags::bitflags;
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
    pub struct LCDControl: u16 {
        const DISPLAY_OBJ_WIN   = u16::bit(15);
        const DISPLAY_WIN1      = u16::bit(14);
        const DISPLAY_WIN0      = u16::bit(13);
        const DISPLAY_OBJ       = u16::bit(12);
        const DISPLAY_BG3       = u16::bit(11);
        const DISPLAY_BG2       = u16::bit(10);
        const DISPLAY_BG1       = u16::bit(9);
        const DISPLAY_BG0       = u16::bit(8);
        const FORCED_BLANK      = u16::bit(7);
        const OBJ_TILE_WRAP     = u16::bit(6);
        const HBLANK_INTERVAL   = u16::bit(5);
        const FRAME_DISPLAY     = u16::bit(4);
        const CGB_MODE          = u16::bit(3);
        const MODE              = u16::bits(0, 2);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct LCDStatus: u16 {
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
    pub struct BGControl: u16 {
        const SCREEN_SIZE   = u16::bits(14, 15);
        const OVERFLOW      = u16::bit(13);
        const MAP_BASE      = u16::bits(8, 12);
        const NUM_COLOURS   = u16::bit(7);
        const MOSAIC        = u16::bit(6);
        const TILE_BASE     = u16::bits(2, 3);
        const PRIORITY      = u16::bits(0, 1);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct WindowControl: u8 {
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
    pub struct ColourSpecialControl: u16 {
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

    mosaic: u16,
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
            0x2A => self.bg2_ref_x = bytes::u32::set_hi(self.bg2_ref_x, data),
            0x2C => self.bg2_ref_y = bytes::u32::set_lo(self.bg2_ref_y, data),
            0x2E => self.bg2_ref_y = bytes::u32::set_hi(self.bg2_ref_y, data),
            0x30 => self.bg3_matrix_a = data,
            0x32 => self.bg3_matrix_b = data,
            0x34 => self.bg3_matrix_c = data,
            0x36 => self.bg3_matrix_d = data,
            0x38 => self.bg3_ref_x = bytes::u32::set_lo(self.bg3_ref_x, data),
            0x3A => self.bg3_ref_x = bytes::u32::set_hi(self.bg3_ref_x, data),
            0x3C => self.bg3_ref_y = bytes::u32::set_lo(self.bg3_ref_y, data),
            0x3E => self.bg3_ref_y = bytes::u32::set_hi(self.bg3_ref_y, data),
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