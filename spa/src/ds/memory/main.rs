
use std::sync::{
    Arc, Mutex
};
use crate::common::wram::WRAM;

#[derive(Clone)]
pub struct MainRAM {
    ram: Arc<Mutex<WRAM>>
}

impl MainRAM {
    pub fn new() -> Self {
        Self {
            ram: Arc::new(Mutex::new(WRAM::new(4 * 1024 * 1024)))
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.ram.lock().unwrap().read_byte(addr)
    }
    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.ram.lock().unwrap().write_byte(addr, data);
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        self.ram.lock().unwrap().read_halfword(addr)
    }
    pub fn write_halfword(&mut self, addr: u32, data: u16) {
        self.ram.lock().unwrap().write_halfword(addr, data)
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        self.ram.lock().unwrap().read_word(addr)
    }
    pub fn write_word(&mut self, addr: u32, data: u32) {
        self.ram.lock().unwrap().write_word(addr, data)
    }
}
