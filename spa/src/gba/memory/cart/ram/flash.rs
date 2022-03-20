use super::{
    SaveRAM,
    FLASH_64_CODE, FLASH_128_CODE,
    FLASH_64_SIZE, FLASH_128_SIZE
};

use std::{
    io::{
        Result,
        Read,
        Write,
        Seek,
        SeekFrom
    },
    fs::File
};
use crate::utils::{
    meminterface::MemInterface8,
    bytes::u16
};

#[allow(dead_code)]
mod flashdev {
    pub const SST_64: u16 = 0xD4BF;
    pub const MACRONIX_64: u16 = 0x1CC2;
    pub const PANASONIC_64: u16 = 0x1B32;
    pub const ATMEL_64: u16 = 0x3D1F;
    pub const SANYO_128: u16 = 0x1362;
    pub const MACRONIX_128: u16 = 0x09C2;
}

#[derive(Clone, Copy)]
enum FlashMode {
    Read,
    ModeAA,
    Mode55,
    Erase,      // 0x80
    GetID,      // 0x90
    Write,      // 0xA0
    SelectBank, // 0xB0
}

pub struct FLASH {
    ram:            Vec<u8>,
    file:           Option<File>,
    bank_offset:    usize,
    mode:           FlashMode,
    device_type:    u16,
    dirty:          bool,
}

impl FLASH {
    /// Create FLASH from an existing save file.
    pub fn new_from_file(mut file: File, size: usize) -> Result<Self> {
        let mut buffer = vec![0; size];
        file.seek(SeekFrom::Start(4))?;
        file.read_exact(&mut buffer)?;
        Ok(Self {
            ram:            buffer,
            file:           Some(file),
            bank_offset:    0,
            mode:           FlashMode::Read,
            device_type:    if size > FLASH_64_SIZE {flashdev::SANYO_128} else {flashdev::PANASONIC_64},
            dirty:          false,
        })
    }

    /// Create FLASH 64KB from a new file.
    pub fn new_64(mut file: Option<File>) -> Self {
        if let Some(file) = &mut file {
            file.set_len((FLASH_64_SIZE + 4) as u64).unwrap();
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(FLASH_64_CODE.as_bytes()).expect("Couldn't write to save file!");
        }
        Self {
            ram:            vec![0xFF; FLASH_64_SIZE],
            file:           file,
            bank_offset:    0,
            mode:           FlashMode::Read,
            device_type:    flashdev::PANASONIC_64,
            dirty:          true,
        }
    }

    /// Create FLASH 128KB from a new file.
    pub fn new_128(mut file: Option<File>) -> Self {
        if let Some(file) = &mut file {
            file.set_len((FLASH_128_SIZE + 4) as u64).unwrap();
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(FLASH_128_CODE.as_bytes()).expect("Couldn't write to save file!");
        }
        Self {
            ram:            vec![0xFF; FLASH_128_SIZE],
            file:           file,
            bank_offset:    0,
            mode:           FlashMode::Read,
            device_type:    flashdev::SANYO_128,
            dirty:          true,
        }
    }
}

// Internal
impl FLASH {
    fn erase_all(&mut self) {
        for b in &mut self.ram {
            *b = 0xFF;
        }
        self.dirty = true;
    }

    fn erase_sector(&mut self, start: usize) {
        let start = start + self.bank_offset;
        let end = start + 0x1000;
        for b in &mut self.ram[start..end] {
            *b = 0xFF;
        }
        self.dirty = true;
    }
}

impl MemInterface8 for FLASH {
    fn read_byte(&mut self, addr: u32) -> u8 {
        use FlashMode::*;
        match self.mode {
            // SST
            GetID => if addr == 0 {
                u16::lo(self.device_type)
            } else {
                u16::hi(self.device_type)
            },
            _ => self.ram[(addr as usize) + self.bank_offset],
        }
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        use FlashMode::*;
        match self.mode {
            ModeAA if addr == 0x2AAA && data == 0x55 => {
                self.mode = Mode55;
            },
            Mode55 if addr == 0x5555 => {
                self.mode = match data {
                    0x80 => Erase,
                    0x10 => {
                        self.erase_all();
                        Read
                    },
                    0x90 => GetID,
                    0xA0 => Write,
                    0xB0 => SelectBank,
                    0xF0 => Read,
                    _ => self.mode
                };
            },
            Mode55 if data == 0x30 => {
                let sector_start = addr & 0xF000;
                self.erase_sector(sector_start as usize);
                self.mode = Read;
                self.dirty = true;
            },
            Write => {
                self.ram[(addr as usize) + self.bank_offset] = data;
                self.mode = Read;
                self.dirty = true;
            },
            SelectBank if addr == 0 => {
                self.bank_offset = (data as usize) * 0x10000;
                self.mode = Read;
            },
            _ if addr == 0x5555 && data == 0xAA => {
                self.mode = ModeAA;
            },
            _ => {}
        }
    }
}

impl SaveRAM for FLASH {
    fn flush(&mut self) {
        if let Some(file) = &mut self.file {
            if self.dirty {
                // First 4 bytes contain save ram type code.
                file.seek(SeekFrom::Start(4)).expect("Couldn't seek to start of save file!");
                file.write_all(&self.ram).expect("Couldn't write to save file!");
                self.dirty = false;
            }
        }
    }
}
