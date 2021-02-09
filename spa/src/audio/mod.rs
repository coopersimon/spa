/// Audio for GBA

use crate::common::{
    meminterface::MemInterface8,
    bytes::u16
};

pub struct GBAAudio {
    soundbias:  u16
}

impl GBAAudio {
    pub fn new() -> Self {
        Self {
            soundbias:  0x200,
        }
    }
}

impl MemInterface8 for GBAAudio {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0x28 => u16::lo(self.soundbias),
            0x29 => u16::hi(self.soundbias),
            _ => 0
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x28 => self.soundbias = u16::set_lo(self.soundbias, data),
            0x29 => self.soundbias = u16::set_hi(self.soundbias, data),
            _ => {}
        }
    }
}