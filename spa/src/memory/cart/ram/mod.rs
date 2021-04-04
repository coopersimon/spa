/// Save RAM

mod sram;
mod flash;
mod eeprom;

use std::{
    path::Path,
    io::{
        Result,
        Read,
    },
    fs::{
        File,
        OpenOptions
    }
};
use crate::common::meminterface::MemInterface8;

use sram::SRAM;
use flash::FLASH;
use eeprom::*;

const SRAM_CODE: &'static str = "SRAM";
const EEPROM_512_CODE: &'static str = "EPR5";
const EEPROM_8K_CODE: &'static str = "EPR8";
const FLASH_64_CODE: &'static str = "FL64";
const FLASH_128_CODE: &'static str = "F128";

const SRAM_SIZE: usize = 32 * 1024;
const EEPROM_512_SIZE: usize = 512;
const EEPROM_8K_SIZE: usize = 8 * 1024;
const FLASH_64_SIZE: usize = 64 * 1024;
const FLASH_128_SIZE: usize = 128 * 1024;

/// Detect the save RAM from the game pak ROM and return it.
/// 
/// It will try to open the file at save_path, and will create it if it doesn't exist.
/// If no save_path is provided, the save data will be lost on shutdown!
/// 
/// If the boolean returned is true, then the RAM is EEPROM and must be addressed accordingly.
pub fn make_save_ram(rom: &[u8], save_path: Option<&Path>) -> (Box<dyn SaveRAM>, bool) {

    // See if save file exists.
    let file = if let Some(path) = save_path {
        if let Ok(file) = OpenOptions::new().read(true).write(true).open(path) {
            match make_from_existing(file) {
                Ok(save_ram) => return save_ram,
                Err(e) => println!("Can't use existing file: {}", e)
            }
        }

        Some(File::create(path).unwrap())
    } else {
        None
    };

    use regex::bytes::Regex;
    let re = Regex::new("(EEPROM|SRAM|FLASH|FLASH512|FLASH1M)_V...").expect("couldn't compile regex");
    if let Some(found) = re.find(rom) {
        println!("Found: {}", String::from_utf8(found.as_bytes().to_vec()).unwrap());
    
        match found.as_bytes().len() {
            9 => (Box::new(SRAM::new(file)), false),
            10 | 13 => (Box::new(FLASH::new_64(file)), false),
            11 => (Box::new(EEPROM::new(file)), true),
            12 => (Box::new(FLASH::new_128(file)), false),
            _ => unreachable!()
        }
    } else {
        (Box::new(NoSaveRAM{}), false)
    }
}

fn make_from_existing(mut file: File) -> Result<(Box<dyn SaveRAM>, bool)> {
    let mut buf = vec![0; 4];
    file.read_exact(&mut buf)?;
    let s = String::from_utf8(buf).unwrap();
    println!("Found existing save: {}", s);
    match s.as_str() {
        SRAM_CODE => Ok((Box::new(SRAM::new_from_file(file)?), false)),
        FLASH_64_CODE => Ok((Box::new(FLASH::new_from_file(file, FLASH_64_SIZE)?), false)),
        FLASH_128_CODE => Ok((Box::new(FLASH::new_from_file(file, FLASH_128_SIZE)?), false)),
        EEPROM_512_CODE => Ok((Box::new(EEPROM::new_from_file(file, EEPROMSize::B512)?), true)),
        EEPROM_8K_CODE => Ok((Box::new(EEPROM::new_from_file(file, EEPROMSize::K8)?), true)),
        c => panic!("Unknown save code {}", c)
    }
}

/// Save RAM interface
pub trait SaveRAM: MemInterface8 {
    fn flush(&mut self);
}

/// For games with no save backup.
pub struct NoSaveRAM {}

impl MemInterface8 for NoSaveRAM {
    fn read_byte(&mut self, _addr: u32) -> u8 {
        0
    }
    fn write_byte(&mut self, _addr: u32, _data: u8) {}
}

impl SaveRAM for NoSaveRAM {
    fn flush(&mut self) {}
}
