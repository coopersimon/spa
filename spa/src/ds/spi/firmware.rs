
use std::{
    io::{
        Result,
        Read,
        Seek,
        SeekFrom
    },
    fs::File,
    path::Path
};

const FIRMWARE_SIZE: u32 = 256 * 1024;

enum Instruction {
    None,
    Read(u8)
}

/// Internal NDS firmware
pub struct Firmware {
    instr:  Instruction,
    addr:   u32,

    data: Vec<u8>,
}

impl Firmware {
    pub fn new(path: Option<&Path>) -> Result<Self> {
        let data = if let Some(path) = path {
            let mut firmware_file = File::open(path)?;
            let mut buffer = vec![0; FIRMWARE_SIZE as usize];

            firmware_file.seek(SeekFrom::Start(0))?;
            firmware_file.read(&mut buffer)?;

            buffer
        } else {
            Vec::new()
        };

        Ok(Self {
            instr:  Instruction::None,
            addr:   0,

            data: data,
        })
    }

    pub fn deselect(&mut self) {
        self.instr = Instruction::None;
        self.addr = 0;
    }

    pub fn read(&mut self) -> u8 {
        self.data[self.addr as usize]
    }

    pub fn write(&mut self, data: u8) {
        use Instruction::*;
        match self.instr {
            None => match data {
                0x03 => self.instr = Read(3),
                _ => panic!("unsupported instr {:X}", data),
            },
            Read(0) => {},  // Dummy write
            Read(n) => {
                self.addr |= (data as u32) << ((3 - n) * 8);
                self.instr = Read(n-1);
            }
        }
    }
}

