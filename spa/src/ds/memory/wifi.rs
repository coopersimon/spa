use crate::utils::meminterface::MemInterface16;
use crate::common::mem::ram::RAM;

pub struct Wifi {
    // TOOD: regs

    ram: RAM
}

impl Wifi {
    pub fn new() -> Self {
        Self {
            ram: RAM::new(0x2000)
        }
    }
}

impl MemInterface16 for Wifi {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0480_4000..=0x0480_5FFF => self.ram.read_halfword(addr - 0x0480_4000),
            
            0x0480_8000 => 0x1440,  // ID
            _ => {
                //println!("wifi read {:X}", addr);
                0
            }
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0480_4000..=0x0480_5FFF => self.ram.write_halfword(addr - 0x0480_4000, data),
            _ => {
                //println!("wifi write {:X} => {:X}", data, addr);
            }
        }
    }
}