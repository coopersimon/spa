/// DS button extensions

use bitflags::bitflags;
use crate::utils::{
    meminterface::MemInterface16,
    bits::u16
};

bitflags!{
    #[derive(Default)]
    pub struct DSButtons: u16 {
        const HINGE_DOWN    = u16::bit(7);
        const PEN_DOWN      = u16::bit(6);
        const DEBUG         = u16::bit(3);
        const Y             = u16::bit(1);
        const X             = u16::bit(0);
    }
}

pub struct DSJoypad {
    rcnt: u16,
    buttons_pressed: DSButtons,
}

impl DSJoypad {
    pub fn new() -> Self {
        Self {
            rcnt: 0,
            buttons_pressed: DSButtons::from_bits_truncate(0x4B),
        }
    }

    pub fn set_all_buttons(&mut self, buttons: DSButtons) {
        self.buttons_pressed = buttons;
    }
}

impl MemInterface16 for DSJoypad {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0134 => self.rcnt,
            0x0400_0136 => self.buttons_pressed.bits(),
            _ => panic!("ds joypad invalid addr")
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0134 => self.rcnt = data,
            0x0400_0136 => {}, // Buttons are not written via this function. Use `set_button` instead.
            _ => panic!("ds joypad invalid addr")
        }
    }
}
