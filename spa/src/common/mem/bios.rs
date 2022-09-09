/// Internal BIOS for GBA and NDS

use std::{
    io::{
        Result, Read
    },
    fs::File,
    convert::TryInto,
    path::Path
};

/// BIOS that can be loaded from file.
pub struct BIOS {
    data: Vec<u8>
}

impl BIOS {
    pub fn new_from_file(bios_path: &Path) -> Result<Self> {
        let mut cart_file = File::open(bios_path)?;
        let mut buffer = Vec::new();
        cart_file.read_to_end(&mut buffer)?;
        Ok(Self {
            data: buffer
        })
    }

    pub fn new_from_data(data: Vec<u8>) -> Self {
        Self {
            data
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.data[addr as usize]
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        let start = addr as usize;
        let end = start + 2;
        let data = (self.data[start..end]).try_into().unwrap();
        u16::from_le_bytes(data)
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        let start = addr as usize;
        let end = start + 4;
        let data = (self.data[start..end]).try_into().unwrap();
        u32::from_le_bytes(data)
    }
}
