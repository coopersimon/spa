/// Button inputs

use bitflags::bitflags;
use crate::utils::{
    bits::u16,
    meminterface::MemInterface16,
};

bitflags!{
    #[derive(Default)]
    pub struct Buttons: u16 {
        const L         = u16::bit(9);
        const R         = u16::bit(8);
        const DOWN      = u16::bit(7);
        const UP        = u16::bit(6);
        const LEFT      = u16::bit(5);
        const RIGHT     = u16::bit(4);
        const START     = u16::bit(3);
        const SELECT    = u16::bit(2);
        const B         = u16::bit(1);
        const A         = u16::bit(0);
    }
}

pub struct Joypad {
    buttons_pressed:    Buttons,
    interrupt_control:  Buttons,
    interrupt_enable:   bool,
    interrupt_cond:     bool,
}

impl Joypad {
    pub fn new() -> Self {
        Self {
            buttons_pressed:    Buttons::from_bits_truncate(0x3FF),
            interrupt_control:  Buttons::default(),
            interrupt_enable:   false,
            interrupt_cond:     false,
        }
    }

    pub fn set_all_buttons(&mut self, buttons: Buttons) {
        self.buttons_pressed = buttons;
    }

    pub fn get_interrupt(&self) -> bool {
        if !self.interrupt_enable {
            return false;
        }

        let set_buttons = Buttons::from_bits_truncate(self.buttons_pressed.bits() ^ 0x3FF);
        if self.interrupt_cond { // AND
            set_buttons.contains(self.interrupt_control)
        } else {                 // OR
            set_buttons.intersects(self.interrupt_control)
        }
    }
}

impl MemInterface16 for Joypad {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0130 => self.buttons_pressed.bits(),
            0x0400_0132 => self.get_interrupt_control(),
            _ => unreachable!()
        }
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0130 => {},    // Buttons are not written via this function. Use `set_button` instead.
            0x0400_0132 => self.set_interrupt_control(data),
            _ => unreachable!()
        }
    }
}

// Internal
impl Joypad {
    fn set_interrupt_control(&mut self, data: u16) {
        self.interrupt_control = Buttons::from_bits_truncate(data);
        self.interrupt_enable = u16::test_bit(data, 14);
        self.interrupt_cond = u16::test_bit(data, 15);
    }

    fn get_interrupt_control(&self) -> u16 {
        self.interrupt_control.bits() |
        if self.interrupt_enable    {u16::bit(14)} else {0} |
        if self.interrupt_cond      {u16::bit(15)} else {0}
    }
}