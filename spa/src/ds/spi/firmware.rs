
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
    can_read:   bool,
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
            can_read:   false,
            can_write:  false,
        })
    }

    pub fn deselect(&mut self) {
        self.instr = Instruction::None;
        self.addr = 0;
        self.can_read = false;
    }

    pub fn read(&mut self) -> u8 {
        use Instruction::*;
        match self.instr {
            Read(_) if self.can_read => {
                self.can_read = false;
                let data = self.data[self.addr as usize];
                self.addr += 1;
                data
            },
            ReadStatus => 0,
            _ => 0,
        }
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
            Read(0) => { // Dummy write
                self.can_read = true;
            },
            Read(n) => {
                // Addr written in MSB first
                self.addr |= (data as u32) << ((n - 1) * 8);
                self.instr = Read(n-1);
            },
            _ => {}
        }
    }
}
