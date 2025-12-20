
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
    Read(u8),
    ReadStatus
}

/// Internal NDS firmware
pub struct Firmware {
    instr:      Instruction,
    addr:       u32,

    data:       Vec<u8>,
    read_buffer: u8,
    can_write:  bool,
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
            instr:      Instruction::None,
            addr:       0,

            data:       data,
            read_buffer: 0,
            can_write:  false,
        })
    }

    pub fn deselect(&mut self) {
        self.instr = Instruction::None;
        self.addr = 0;
    }

    pub fn read(&mut self) -> u8 {
        self.read_buffer
    }

    pub fn write(&mut self, data: u8) {
        use Instruction::*;
        match self.instr {
            None => match data {
                0x03 => self.instr = Read(3),
                0x05 => self.instr = ReadStatus,
                0x06 => self.can_write = true,
                0x04 => self.can_write = false,
                _ => panic!("unsupported instr {:X}", data),
            },
            ReadStatus => {
                self.read_buffer = if self.can_write {1} else {0};
            },
            Read(0) => { // Strobe
                self.read_buffer = self.data[self.addr as usize];
                self.addr += 1;
            },
            Read(n) => {
                // Addr written in MSB first
                self.addr |= (data as u32) << ((n - 1) * 8);
                self.instr = Read(n-1);
            },
        }
    }
}
