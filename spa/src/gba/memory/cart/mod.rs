/// Cartridge interface.

mod controller;
mod ram;

use std::{
    io::{
        Result, Read
    },
    fs::File,
    convert::TryInto
};
use crate::utils::{
    bytes::u16,
    meminterface::MemInterface16
};

pub use controller::GamePakController;
use ram::*;

/// The ROM and RAM inside a game pak (cartridge).
pub struct GamePak {
    rom:    Vec<u8>,
    ram:    Box<dyn SaveRAM + Send>,
    /// ROM is larger than 16MB
    large:  bool,
    eeprom: bool,
}

impl GamePak {
    pub fn new(rom_path: String, save_path: Option<String>) -> Result<Self> {
        let mut rom_file = File::open(rom_path)?;
        let mut buffer = Vec::new();
        rom_file.read_to_end(&mut buffer)?;

        // Detect save file type.
        let (ram, eeprom) = make_save_ram(&buffer, save_path);
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

    /// Write the save to the save file.
    pub fn flush_save(&mut self) {
        self.ram.flush();
    }
}

impl MemInterface16 for GamePak {
    fn read_byte(&mut self, addr: u32) -> u8 {
        let rom_addr = (addr % 0x0200_0000) as usize;
        match addr {
            0x0900_0000..=0x09FF_FEFF if self.eeprom && self.large => self.rom[rom_addr],
            0x0900_0000..=0x09FF_FFFF if self.eeprom => self.ram.read_byte(addr),
            0x0B00_0000..=0x0BFF_FEFF if self.eeprom && self.large => self.rom[rom_addr],
            0x0B00_0000..=0x0BFF_FFFF if self.eeprom => self.ram.read_byte(addr),
            0x0D00_0000..=0x0DFF_FEFF if self.eeprom && self.large => self.rom[rom_addr],
            0x0D00_0000..=0x0DFF_FFFF if self.eeprom => self.ram.read_byte(addr),
            0x0800_0000..=0x0DFF_FFFF => self.rom[rom_addr],
            0x0E00_0000..=0x0EFF_FFFF => self.ram.read_byte(addr & 0xFFFF),
            _ => unreachable!()
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x0900_0000..=0x09FF_FFFF if self.eeprom => self.ram.write_byte(addr, data),
            0x0B00_0000..=0x0BFF_FFFF if self.eeprom => self.ram.write_byte(addr, data),
            0x0D00_0000..=0x0DFF_FFFF if self.eeprom => self.ram.write_byte(addr, data),
            0x0E00_0000..=0x0EFF_FFFF => self.ram.write_byte(addr & 0xFFFF, data),
            0x0800_0000..=0x0DFF_FFFF => panic!("Trying to write to ROM 0x{:X}", addr),
            _ => unreachable!()
        }
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0900_0000..=0x09FF_FEFF if self.eeprom && self.large => self.read_u16(addr % 0x0200_0000),
            0x0900_0000..=0x09FF_FFFF if self.eeprom => self.ram.read_halfword(addr),
            0x0B00_0000..=0x0BFF_FEFF if self.eeprom && self.large => self.read_u16(addr % 0x0200_0000),
            0x0B00_0000..=0x0BFF_FFFF if self.eeprom => self.ram.read_halfword(addr),
            0x0D00_0000..=0x0DFF_FEFF if self.eeprom && self.large => self.read_u16(addr % 0x0200_0000),
            0x0D00_0000..=0x0DFF_FFFF if self.eeprom => self.ram.read_halfword(addr),
            0x0800_0000..=0x0DFF_FFFF => self.read_u16(addr % 0x0200_0000),
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
            0x0800_0000..=0x0DFF_FFFF => panic!("Trying to write to ROM 0x{:X}", addr),
            _ => unreachable!()
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0900_0000..=0x09FF_FEFF if self.eeprom && self.large => self.read_u32(addr % 0x0200_0000),
            0x0900_0000..=0x09FF_FFFF if self.eeprom => self.ram.read_word(addr),
            0x0B00_0000..=0x0BFF_FEFF if self.eeprom && self.large => self.read_u32(addr % 0x0200_0000),
            0x0B00_0000..=0x0BFF_FFFF if self.eeprom => self.ram.read_word(addr),
            0x0D00_0000..=0x0DFF_FEFF if self.eeprom && self.large => self.read_u32(addr % 0x0200_0000),
            0x0D00_0000..=0x0DFF_FFFF if self.eeprom => self.ram.read_word(addr),
            0x0800_0000..=0x0DFF_FFFF => self.read_u32(addr % 0x0200_0000),
            0x0E00_0000..=0x0EFF_FFFF => self.ram.read_word(addr & 0xFFFF),
            _ => unreachable!()
        }
    }
}

// Internal: ROM access
impl GamePak {
    fn read_u16(&self, addr: u32) -> u16 {
        let start = addr as usize;
        let end = start + 2;
        let data = (self.rom[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }

    fn read_u32(&self, addr: u32) -> u32 {
        let start = addr as usize;
        let end = start + 4;
        let data = (self.rom[start..end]).try_into().unwrap();
        u32::from_le_bytes(data)
    }
}