use super::{
    SaveRAM, SRAM_CODE, SRAM_SIZE
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
use crate::utils::meminterface::MemInterface8;


/// SRAM. Simple 32kB region of 8-bit battery-backed memory.
pub struct SRAM {
    ram:    Vec<u8>,
    file:   Option<File>,
    dirty:  bool,
}

impl SRAM {
    /// Create SRAM from an existing save file.
    pub fn new_from_file(mut file: File) -> Result<Self> {
        let mut buffer = vec![0; SRAM_SIZE];
        file.seek(SeekFrom::Start(4))?;
        file.read_exact(&mut buffer)?;
        Ok(Self {
            ram:    buffer,
            file:   Some(file),
            dirty:  false,
        })
    }

    /// Create SRAM from a new file.
    pub fn new(mut file: Option<File>) -> Self {
        if let Some(file) = &mut file {
            file.set_len((SRAM_SIZE + 4) as u64).unwrap();
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(SRAM_CODE.as_bytes()).expect("Couldn't write to save file!");
        }
        Self {
            ram:    vec![0; SRAM_SIZE],
            file:   file,
            dirty:  true,
        }
    }
}

impl MemInterface8 for SRAM {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.ram[addr as usize]
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.ram[addr as usize] = data;
        self.dirty = true;
    }
}

impl SaveRAM for SRAM {
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
