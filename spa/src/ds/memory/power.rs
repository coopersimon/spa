
use bitflags::bitflags;
use crate::utils::{
    bits::u8,
    meminterface::MemInterface8
};

bitflags! {
    #[derive(Default)]
    pub struct GraphicsPowerControlHi: u8 {
        const DISPLAY_SWAP  = u8::bit(7);
        const ENABLE_B      = u8::bit(1);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct GraphicsPowerControlLo: u8 {
        const GEOM_3D       = u8::bit(3);
        const RENDER_3D     = u8::bit(2);
        const ENGINE_A      = u8::bit(1);
        const ENABLE_LCD    = u8::bit(0);
    }
}

bitflags! {
    #[derive(Default)]
    pub struct SoundWifiPowerControl: u8 {
        const WIFI  = u8::bit(1);
        const SOUND = u8::bit(0);
    }
}


/// ARM9 power control register.
/// Contains BIOS post-boot flag, which is after BIOS boot procedure is done.
pub struct DS9PowerControl {
    post_boot_flag:     u8,
    graphics_cnt_lo:    GraphicsPowerControlLo,
    graphics_cnt_hi:    GraphicsPowerControlHi
}

impl DS9PowerControl {
    pub fn new(fast_boot: bool) -> Self {
        Self {
            post_boot_flag:     if fast_boot {1} else {0},
            graphics_cnt_lo:    GraphicsPowerControlLo::default(),
            graphics_cnt_hi:    GraphicsPowerControlHi::default()
        }
    }
}

impl MemInterface8 for DS9PowerControl {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x0400_0300 => self.post_boot_flag,
            0x0400_0304 => self.graphics_cnt_lo.bits(),
            0x0400_0305 => self.graphics_cnt_hi.bits(),
            _ => 0
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x0400_0300 => self.post_boot_flag = data & 1,
            0x0400_0304 => self.graphics_cnt_lo = GraphicsPowerControlLo::from_bits_truncate(data),
            0x0400_0305 => self.graphics_cnt_hi = GraphicsPowerControlHi::from_bits_truncate(data),
            _ => {}
        }
    }
}

/// Internal registers which are used by the BIOS.
pub struct DS7PowerControl {
    post_boot_flag: u8,
    
    pub halt:   bool,
    pub sleep:  bool,

    sound_wifi_control: SoundWifiPowerControl,
}

impl DS7PowerControl {
    pub fn new(fast_boot: bool) -> Self {
        Self {
            post_boot_flag: if fast_boot {1} else {0},
            halt:   false,
            sleep:  false,
            sound_wifi_control: SoundWifiPowerControl::default(),
        }
    }
}

impl MemInterface8 for DS7PowerControl {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x0400_0300 => self.post_boot_flag,
            0x0400_0301 => if self.sleep {
                3 << 6
            } else if self.halt {
                2 << 6
            } else {
                0
            },
            0x0400_0304 => self.sound_wifi_control.bits(),
            _ => 0
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x0400_0300 => self.post_boot_flag = data & 1,
            0x0400_0301 => if u8::test_bit(data, 7) {
                if u8::test_bit(data, 6) {
                    println!("Stop!");
                    self.sleep = true;
                } else {
                    self.halt = true;
                }
            } else {},
            0x0400_0304 => self.sound_wifi_control = SoundWifiPowerControl::from_bits_truncate(data),
            _ => {}
        }
    }
}
