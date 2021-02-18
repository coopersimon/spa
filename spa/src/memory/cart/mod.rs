/// Cartridge interface.

mod controller;
mod ram;

use bytemuck::{
    try_from_bytes
};
use std::{
    io::{
        Result, Read
    },
    path::Path,
    fs::File
};
use crate::common::{
    bytes::u16,
    meminterface::MemInterface16
};

pub use controller::GamePakController;
use ram::*;

/// The ROM and RAM inside a game pak (cartridge).
pub struct GamePak {
    rom:    Vec<u8>,
    ram:    Box<dyn SaveRAM>,
    /// ROM is larger than 16MB
    large:  bool,
    eeprom: bool,
}

impl GamePak {
    pub fn new(cart_path: &Path) -> Result<Self> {
        let mut cart_file = File::open(cart_path)?;
        let mut buffer = Vec::new();
        cart_file.read_to_end(&mut buffer)?;

        // Detect save file type.
        let (ram, eeprom) = make_save_ram(&buffer, None);
        let is_large = buffer.len() > 0x0100_0000;

        // Fill buffer with garbage.
        let start = buffer.len() / 2;
        for i in start..0x0100_0000 {
            let data = i as u16;
            buffer.push(u16::lo(data));
            buffer.push(u16::hi(data));
        }
        Ok(Self {
            rom:    buffer,
            ram:    ram,
            large:  is_large,
            eeprom: eeprom,
        })
    }
}

impl MemInterface16 for GamePak {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let start = (addr % 0x0200_0000) as usize;
        let end = start + 2;
        match addr {
            0x0900_0000..=0x09FF_FEFF if self.eeprom && self.large => *try_from_bytes(&self.rom[start..end]).expect(&format!("cannot read ROM halfword at 0x{:X}", addr)),
            0x0900_0000..=0x09FF_FFFF if self.eeprom => self.ram.read_halfword(addr),
            0x0B00_0000..=0x0BFF_FEFF if self.eeprom && self.large => *try_from_bytes(&self.rom[start..end]).expect(&format!("cannot read ROM halfword at 0x{:X}", addr)),
            0x0B00_0000..=0x0BFF_FFFF if self.eeprom => self.ram.read_halfword(addr),
            0x0D00_0000..=0x0DFF_FEFF if self.eeprom && self.large => *try_from_bytes(&self.rom[start..end]).expect(&format!("cannot read ROM halfword at 0x{:X}", addr)),
            0x0D00_0000..=0x0DFF_FFFF if self.eeprom => self.ram.read_halfword(addr),
            0x0800_0000..=0x0DFF_FFFF => *try_from_bytes(&self.rom[start..end]).expect(&format!("cannot read ROM halfword at 0x{:X}", addr)),
            0x0E00_0000..=0x0EFF_FFFF => self.ram.read_halfword(addr & 0xFFFF),
            _ => unreachable!()
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0900_0000..=0x09FF_FFFF if self.eeprom => self.ram.write_halfword(addr, data),
            0x0B00_0000..=0x0BFF_FFFF if self.eeprom => self.ram.write_halfword(addr, data),
            0x0D00_0000..=0x0DFF_FFFF if self.eeprom => self.ram.write_halfword(addr, data),
            0x0E00_0000..=0x0EFF_FFFF => self.ram.write_halfword(addr & 0xFFFF, data),
            0x0800_0000..=0x0DFF_FFFF => panic!(format!("Trying to write to ROM 0x{:X}", addr)),
            _ => unreachable!()
        }
    }
}