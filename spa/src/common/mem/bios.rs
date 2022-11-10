/// Internal BIOS for GBA and NDS

use std::{
    io::{
        Result, Read
    },
    fs::File,
    path::Path
};
use crate::common::mem::ram::RAM;

/// BIOS that can be loaded from file.
pub struct BIOS {
    data: RAM
}

impl BIOS {
    pub fn new_from_file(bios_path: &Path) -> Result<Self> {
        let mut cart_file = File::open(bios_path)?;
        let mut buffer = Vec::new();
        cart_file.read_to_end(&mut buffer)?;
        Ok(Self {
            data: buffer.into()
        })
    }

    pub fn new_from_data(data: Vec<u8>) -> Self {
        Self {
            data: data.into()
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.data.read_byte(addr)
    }

    pub fn read_halfword(&self, addr: u32) -> u16 {
        self.data.read_halfword(addr)
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        self.data.read_word(addr)
    }
}
