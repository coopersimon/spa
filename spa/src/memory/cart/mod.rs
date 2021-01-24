/// Cartridge interface.

mod controller;

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
use crate::common::meminterface::MemInterface16;

pub use controller::GamePakController;

/// The ROM and RAM inside a game pak (cartridge).
pub struct GamePak {
    rom: Vec<u8>
    // TODO: ram
}

impl GamePak {
    pub fn new(cart_path: &Path) -> Result<Self> {
        let mut cart_file = File::open(cart_path)?;
        let mut buffer = Vec::new();
        cart_file.read_to_end(&mut buffer)?;
        Ok(Self {
            rom: buffer
        })
    }
}

impl MemInterface16 for GamePak {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        let start = addr as usize;
        let end = (addr + 2) as usize;
        *try_from_bytes(&self.rom[start..end]).expect(&format!("cannot read halfword at 0x{:X}", addr))
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        // TODO: RAM
    }
}