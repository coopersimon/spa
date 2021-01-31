/// Simple BIOS for GBA

use bytemuck::try_from_bytes;
use std::{
    io::{
        Result, Read
    },
    path::Path,
    fs::File
};

/// A simple BIOS if the full ROM is not available.
/// Should work for games that don't make use of BIOS calls.
const BIOS_ROM: &[u8] = &[];

pub struct BIOS {
    data: Vec<u8>
}

impl BIOS {
    pub fn new(bios_path: Option<&Path>) -> Result<Self> {
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
        let end = (addr + 2) as usize;
        *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read halfword at 0x{:X}", addr))
    }

    pub fn read_word(&self, addr: u32) -> u32 {
        let start = addr as usize;
        let end = (addr + 4) as usize;
        *try_from_bytes(&self.data[start..end]).expect(&format!("cannot read word at 0x{:X}", addr))
    }
}