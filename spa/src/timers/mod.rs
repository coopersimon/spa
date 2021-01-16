/// GBA timers

mod timer;

use crate::common::bytes::u32;
use timer::Timer;

pub struct Timers {
    timers: [Timer; 4]
}

impl Timers {
    pub fn new() -> Self {
        Self {
            timers: [
                Timer::new(),
                Timer::new(),
                Timer::new(),
                Timer::new()
            ]
        }
    }

    pub fn clock(&mut self, cycles: usize) -> Option<arm::Exception> {
        let mut exception = None;
        for _ in 0..cycles {
            let mut overflow = [false; 4];
            overflow[0] = self.timers[0].clock();
            for t in 1..4 {
                if self.timers[t].cascade_enabled() {
                    if overflow[t-1] {
                        overflow[t] = self.timers[t].clock();
                    }
                } else {
                    overflow[t] = self.timers[t].clock();
                }
            }
            for t in 0..4 {
                if overflow[t] && self.timers[t].irq_enabled() {
                    exception = Some(arm::Exception::Interrupt);
                }
            }
        }
        exception
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