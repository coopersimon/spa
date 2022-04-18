
use crate::utils::{
    bits::u8,
    meminterface::MemInterface8
};

/// BIOS post-boot flag. Set after BIOS boot procedure is done.
pub struct DS9PostFlag {
    post_boot_flag: u8,
}

impl DS9PostFlag {
    pub fn new() -> Self {
        Self {
            post_boot_flag: 0,
        }
    }
}

impl MemInterface8 for DS9PostFlag {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0 => self.post_boot_flag,
            1 => 0,
            2 => 0,
            3 => 0,
            _ => unreachable!()
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0 => self.post_boot_flag = data & 1,
            1 => {}
            2 => {},
            3 => {},
            _ => unreachable!()
        }
    }
}

/// Internal registers which are used by the BIOS.
pub struct DS7PowerControl {
    post_boot_flag: u8,
    
    pub halt:   bool,
    pub sleep:  bool,
}

impl DS7PowerControl {
    pub fn new() -> Self {
        Self {
            post_boot_flag: 0,
            halt:   false,
            sleep:  false,
        }
    }
}

impl MemInterface8 for DS7PowerControl {
    fn read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0 => self.post_boot_flag,
            1 => if self.sleep {
                3 << 6
            } else if self.halt {
                2 << 6
            } else {
                0
            },
            2 => 0,
            3 => 0,
            _ => unreachable!()
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0 => self.post_boot_flag = data & 1,
            1 => if u8::test_bit(data, 7) {
                if u8::test_bit(data, 6) {
                    println!("Stop!");
                    self.sleep = true;
                } else {
                    self.halt = true;
                }
            } else {},
            2 => {},
            3 => {},
            _ => unreachable!()
        }
    }
}
