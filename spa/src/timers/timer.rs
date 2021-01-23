/// Individual GBA timer.

use bitflags::bitflags;
use crate::common::{
    bits::u8,
    bytes::{u16, u32}
};
use crate::interrupt::Interrupts;

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
    control:    Control,
    /// Which interrupt this timer is responsible for.
    interrupt:  Interrupts,
}

impl Timer {
    pub fn new(interrupt: Interrupts) -> Self {
        Self {
            counter:    0,
            internal:   0,
            reload:     0,
            control:    Control::default(),
            interrupt:  interrupt,
        }
    }

    /// Returns the number of times that the timer overflowed.
    /// This _may_ trigger an interrupt if this happens - check irq_enabled
    pub fn clock(&mut self, cycles: usize) -> usize {
        if !self.control.contains(Control::ENABLE) {
            return 0;
        }

        self.internal = self.internal.wrapping_add(cycles as u16);
        let divide_ratio = match (self.control & Control::FREQ).bits() {
            0b00 => 1,
            0b01 => 64,
            0b10 => 256,
            0b11 => 1024,
            _ => unreachable!()
        };

        let mut overflows = 0;
        while self.internal >= divide_ratio {
            let (new_count, overflow) = self.counter.overflowing_add(1);
            self.internal = self.internal.wrapping_sub(divide_ratio);

            if overflow {
                overflows += 1;
                self.counter = self.reload;
            } else {
                self.counter = new_count;
            }
        }
        overflows
    }

    /// If the timer overflowed, we call this to get the interrupt the timer is responsible for.
    /// 
    /// This could be nothing if the timer disabled interrupts.
    pub fn get_interrupt(&self) -> Interrupts {
        if self.control.contains(Control::IRQ_EN) {
            self.interrupt
        } else {
            Interrupts::default()
        }
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