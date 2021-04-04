/// Simple BIOS for GBA

use std::{
    io::{
        Result, Read
    },
    path::Path,
    fs::File,
    convert::TryInto
};

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
            construct_bios()
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

/// A simple BIOS if the full ROM is not available.
/// 
/// This just deals with IRQ interrupt handling.
/// 
/// Should work for games that don't make use of SWI calls.
fn construct_bios() -> Vec<u8> {
    let mut bios_mem = vec![0; 0x4000];

    write_word_to_mem(&mut bios_mem, 0x18, 0xEA00_0042); // B 0x128
    write_word_to_mem(&mut bios_mem, 0x128, 0xE92D_500F); // STMFD SP! R0-3,R12,R14
    write_word_to_mem(&mut bios_mem, 0x12C, 0xE3A0_0301); // MOV R0,#0400_0000
    write_word_to_mem(&mut bios_mem, 0x130, 0xE28F_E000); // ADD R14,R15,0
    write_word_to_mem(&mut bios_mem, 0x134, 0xE510_F004); // LDR R15,[R0,#-4]
    write_word_to_mem(&mut bios_mem, 0x138, 0xE8BD_500F); // LDMFD SP! R0-3,R12,R14
    write_word_to_mem(&mut bios_mem, 0x13C, 0xE25E_F004); // SUBS R15,R14,#4

    bios_mem
}

fn write_word_to_mem(mem: &mut [u8], addr: usize, data: u32) {
    let bytes = data.to_le_bytes();
    for (dest, byte) in mem[addr..(addr + 4)].iter_mut().zip(&bytes) {
        *dest = *byte;
    }
}
