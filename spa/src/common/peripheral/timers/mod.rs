/// GBA timers

mod timer;

use crate::utils::{
    bytes::u32,
    bits::u16
};
use timer::Timer;

pub struct Timers {
    timers: [Timer; 4]
}

// Interrupt bits.
const TIMER_0: u16 = u16::bit(3);
const TIMER_1: u16 = u16::bit(4);
const TIMER_2: u16 = u16::bit(5);
const TIMER_3: u16 = u16::bit(6);

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::new(TIMER_0),
                Timer::new(TIMER_1),
                Timer::new(TIMER_2),
                Timer::new(TIMER_3)
            ]
        }
    }

    /// Clock the timers. Should be done as often as possible.
    /// 
    /// Returns any interrupts to request,
    /// as well as two bools indicating if timer 0 or 1 overflowed.
    pub fn clock(&mut self, cycles: usize) -> (u16, bool, bool) {
        let mut interrupts = 0;
        let mut timer_0 = false;
        let mut timer_1 = false;
        let mut overflows = self.timers[0].clock(cycles);
        if overflows > 0 {
            interrupts |= self.timers[0].get_interrupt();
            timer_0 = true;
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
                interrupts |= self.timers[t].get_interrupt();
                if t == 1 {
                    timer_1 = true;
                }
            }
        }
        (interrupts, timer_0, timer_1)
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        let addr = addr - 0x0400_0100;
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].read_byte(timer_addr)
    }
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        let addr = addr - 0x0400_0100;
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].write_byte(timer_addr, data);
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        let addr = addr - 0x0400_0100;
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].read_halfword(timer_addr)
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        let addr = addr - 0x0400_0100;
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].write_halfword(timer_addr, data);
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        let addr = addr - 0x0400_0100;
        let timer = addr / 4;
        let timer_addr = addr % 4;
        let lo = self.timers[timer as usize].read_halfword(timer_addr);
        let hi = self.timers[timer as usize].read_halfword(timer_addr + 2);
        u32::make(hi, lo)
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        let addr = addr - 0x0400_0100;
        let timer = addr / 4;
        let timer_addr = addr % 4;
        self.timers[timer as usize].write_halfword(timer_addr, u32::lo(data));
        self.timers[timer as usize].write_halfword(timer_addr + 2, u32::hi(data));
    }
}