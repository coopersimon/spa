/// Simple BIOS for GBA

use std::{
    io::{
        Result, Read
    },
    fs::File,
    convert::TryInto
};

/// A simple BIOS if the full ROM is not available.
/// Should work for games that don't make use of BIOS calls.
const BIOS_ROM: &[u8] = &[];

pub struct BIOS {
    data: Vec<u8>
}

impl BIOS {
    pub fn new(bios_path: Option<String>) -> Result<Self> {
        let data = if let Some(path) = bios_path {
            let mut cart_file = File::open(path)?;
            let mut buffer = Vec::new();
            cart_file.read_to_end(&mut buffer)?;
            buffer
        } else {
            let mut buffer = Vec::new();
            buffer.extend_from_slice(BIOS_ROM);
            buffer
        };
        Ok(Self {
            data
        })
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