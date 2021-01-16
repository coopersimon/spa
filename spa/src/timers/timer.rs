/// Individual GBA timer.

use bitflags::bitflags;
use crate::common::{
    bits::u8,
    bytes::{u16, u32}
};

bitflags!{
    #[derive(Default)]
    struct Control: u8 {
        const ENABLE    = u8::bit(7);
        const IRQ_EN    = u8::bit(6);
        const CASCADE   = u8::bit(2);
        const FREQ      = u8::bits(0, 1);
    }
}

pub struct Timer {
    /// Counter. Starts at reload value and counts up.
    counter:    u16,
    /// Internal counter. Used to clock divide.
    internal:   u16,
    /// Value to reload counter with upon start or overflow.
    reload:     u16,
    /// Timer settings.
    control:    Control
}

impl Timer {
    pub fn new() -> Self {
        Self {
            counter:    0,
            internal:   0,
            reload:     0,
            control:    Control::default(),
        }
    }

    /// Returns true if the timer overflowed.
    /// This _may_ trigger an interrupt if this happens - check irq_enabled
    /// TODO: optimise for multiple cycles.
    pub fn clock(&mut self) -> bool {
        if !self.control.contains(Control::ENABLE) {
            return false;
        }

        self.internal = self.internal.wrapping_add(1);
        let divide_ratio = match (self.control & Control::FREQ).bits() {
            0b00 => 1,
            0b01 => 64,
            0b10 => 256,
            0b11 => 1024,
            _ => unreachable!()
        };
        if self.internal == divide_ratio {
            self.counter = self.counter.wrapping_add(1);
            self.internal = 0;
        }

        if self.counter == 0 {
            self.counter = self.reload;
            true
        } else {
            false
        }
    }

    pub fn irq_enabled(&self) -> bool {
        self.control.contains(Control::IRQ_EN)
    }

    pub fn cascade_enabled(&self) -> bool {
        self.control.contains(Control::CASCADE)
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        match addr & 0x3 {
            0 => u16::lo(self.counter),
            1 => u16::hi(self.counter),
            2 => self.control.bits(),
            3 => 0,
            _ => unreachable!()
        }
    }
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        match addr & 0x3 {
            0 => self.reload = u16::set_lo(self.reload, data),
            1 => self.reload = u16::set_hi(self.reload, data),
            2 => self.set_control(data),
            3 => {},
            _ => unreachable!()
        }
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        match addr & 0x3 {
            0 => self.counter,
            2 => u16::make(0, self.control.bits()),
            _ => panic!("unaligned timer addr")
        }
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr & 0x3 {
            0 => self.reload = data,
            2 => self.set_control(u16::lo(data)),
            _ => panic!("unaligned timer addr")
        }
    }

    /*pub fn read_word(&self, addr: u32) -> u32 {
        match addr & 0x3 {
            0 => {
                let control = u16::make(0, self.control.bits());
                u32::make(control, self.counter)
            },
            _ => panic!("unaligned timer addr")
        }
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        match addr & 0x3 {
            0 => {
                self.reload = u32::lo(data);
                self.set_control(u16::lo(u32::hi(data)));
            },
            _ => panic!("unaligned timer addr")
        }
    }*/
}

// Internal
impl Timer {
    fn set_control(&mut self, data: u8) {
        self.control = Control::from_bits_truncate(data);
        if self.control.contains(Control::ENABLE) {
            self.counter = self.reload;
        }
    }
}