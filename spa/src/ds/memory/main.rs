
use std::sync::Arc;
use std::cell::UnsafeCell;
use crate::common::mem::ram::RAM;

// This code is pretty unsafe. The main RAM can be accessed from both the
// processors simultaneously. For perf reasons we want to allow both to freely
// access the RAM. The emulated code is responsible for synchronisation.

#[derive(Clone)]
pub struct MainRAM {
    ram: Arc<UnsafeCell<RAM>>
}

unsafe impl Send for MainRAM {}

impl MainRAM {
    pub fn new() -> Self {
        Self {
            ram: Arc::new(UnsafeCell::new(RAM::new(4 * 1024 * 1024)))
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        unsafe { // Read shared memory without lock
            let ram = &*self.ram.get();
            ram.read_byte(addr)
        }
    }
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        unsafe { // Write shared memory without lock
            let ram = &mut *self.ram.get();
            ram.write_byte(addr, data)
        }
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        unsafe { // Read shared memory without lock
            let ram = &*self.ram.get();
            ram.read_halfword(addr)
        }
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        unsafe { // Write shared memory without lock
            let ram = &mut *self.ram.get();
            ram.write_halfword(addr, data)
        }
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        unsafe { // Read shared memory without lock
            let ram = &*self.ram.get();
            ram.read_word(addr)
        }
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        unsafe { // Write shared memory without lock
            let ram = &mut *self.ram.get();
            ram.write_word(addr, data)
        }
    }
}
