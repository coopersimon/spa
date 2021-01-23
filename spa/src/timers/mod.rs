/// GBA timers

mod timer;

use crate::common::bytes::u32;
use crate::interrupt::Interrupts;
use timer::Timer;

pub struct Timers {
    timers: [Timer; 4]
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::new(Interrupts::TIMER_0),
                Timer::new(Interrupts::TIMER_1),
                Timer::new(Interrupts::TIMER_2),
                Timer::new(Interrupts::TIMER_3)
            ]
        }
    }

    /// Clock the timers. Should be done as often as possible.
    /// Returns any interrupts to request.
    pub fn clock(&mut self, cycles: usize) -> Interrupts {
        let mut interrupts = Interrupts::default();
        let mut overflows = self.timers[0].clock(cycles);
        if overflows > 0 {
            interrupts.insert(self.timers[0].get_interrupt());
        }
        for t in 1..4 {
            overflows = if self.timers[t].cascade_enabled() {
                if overflows > 0 {
                    self.timers[t].clock(overflows)
                } else {
                    0
                }
            } else {
                self.timers[t].clock(cycles)
            };
            if overflows > 0 {
                interrupts.insert(self.timers[t].get_interrupt());
            }
        }
        interrupts
    }

    pub fn read_byte(&mut self, addr: u32) -> u8 {
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].read_byte(timer_addr)
    }
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].write_byte(timer_addr, data);
    }

    pub fn read_halfword(&mut self, addr: u32) -> u16 {
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].read_halfword(timer_addr)
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].write_halfword(timer_addr, data);
    }

    pub fn read_word(&mut self, addr: u32) -> u32 {
        let timer = addr / 4;
        let timer_addr = addr % 4;
        let lo = self.timers[timer as usize].read_halfword(timer_addr);
        let hi = self.timers[timer as usize].read_halfword(timer_addr + 2);
        u32::make(hi, lo)
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].write_halfword(timer_addr, u32::lo(data));
        self.timers[timer as usize].write_halfword(timer_addr + 2, u32::hi(data));
    }
}