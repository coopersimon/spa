
use bitflags::bitflags;
use crate::utils::{
    bits::u8, bytes::u16, meminterface::MemInterface8
};

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
    post_boot_flag:     u8
}

impl DS9PowerControl {
    pub fn new(fast_boot: bool) -> Self {
        Self {
            post_boot_flag:     if fast_boot {1} else {0},
        }
    }
}

impl MemInterface8 for DS9PowerControl {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x0400_0300 => self.post_boot_flag,
            _ => 0
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x0400_0300 => self.post_boot_flag = data & 1,
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

    bios_prot: u16,
}

impl DS7PowerControl {
    pub fn new(fast_boot: bool) -> Self {
        Self {
            post_boot_flag: if fast_boot {1} else {0},
            halt:   false,
            sleep:  false,
            sound_wifi_control: SoundWifiPowerControl::default(),
            bios_prot: 0,
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
            0x0400_0308 => u16::lo(self.bios_prot),
            0x0400_0309 => u16::hi(self.bios_prot),
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
            0x0400_0308 => self.bios_prot = u16::set_lo(self.bios_prot, data),
            0x0400_0309 => self.bios_prot = u16::set_hi(self.bios_prot, data),
            _ => {}
        }
    }
}
