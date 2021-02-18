use super::{
    SaveRAM,
    EEPROM_512_CODE, EEPROM_8K_CODE,
    EEPROM_512_SIZE, EEPROM_8K_SIZE
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
use crate::common::{
    meminterface::MemInterface8,
    bytes::u16
};

const EEPROM_512_ADDR_SIZE: u8 = 6;
const EEPROM_8K_ADDR_SIZE: u8 = 14;
/// Read stream: 6 bits for address, 1 at end.
const READ_STREAM_512_LEN: u8 = EEPROM_512_ADDR_SIZE + 1;
/// Read stream: 14 bits for address, 1 at end.
const READ_STREAM_8K_LEN: u8 = EEPROM_8K_ADDR_SIZE + 1;
/// Write stream: 6 bits for address, 64 bits of data, 1 at end.
const WRITE_STREAM_512_LEN: u8 = EEPROM_512_ADDR_SIZE + 64 + 1;
/// Write stream: 14 bits for address, 64 bits of data, 1 at end.
const WRITE_STREAM_8K_LEN: u8 = EEPROM_8K_ADDR_SIZE + 64 + 1;

#[derive(Clone, Copy, Debug)]
/// EEPROM modes of operation.
enum EEPROMMode {
    /// A neutral state.
    Null(u16),
    /// Write 1,1 to prepare read address.
    /// The address is 6 or 14 bits depending on the EEPROM size.
    PrepRead(u8),
    /// Read from buffer.
    Read(u8),
    /// Write 1,0 to prepare write address.
    /// The address is 6 or 14 bits depending on the EEPROM size.
    Write(u8),

    // TODO: optimise writing to known size.
}

#[derive(Clone, Copy, PartialEq)]
pub enum EEPROMSize {
    /// It is still unknown how large the EEPROM is.
    Unknown,
    /// The EEPROM is 512 Bytes large.
    B512,
    /// The EEPROM is 8 Kilobytes large.
    K8,
}

/// EEPROM save RAM. This is written to 1 bit at a time.
/// The address is unused.
pub struct EEPROM {
    ram:        Vec<u8>,
    file:       Option<File>,
    dirty:      bool,

    size:           EEPROMSize,
    mode:           EEPROMMode,
    write_buffer:   u128,
    read_buffer:    u64,
}

impl EEPROM {
    /// Create EEPROM from an existing save file.
    pub fn new_from_file(mut file: File, size: EEPROMSize) -> Result<Self> {
        let buffer_size = match size {
            EEPROMSize::B512 => EEPROM_512_SIZE,
            EEPROMSize::K8 => EEPROM_8K_SIZE,
            EEPROMSize::Unknown => panic!("don't create unknown EEPROM sizes from files")
        };
        let mut buffer = vec![0; buffer_size];
        file.read_to_end(&mut buffer)?;
        Ok(Self {
            ram:            buffer,
            file:           Some(file),
            dirty:          false,

            size:           size,
            mode:           EEPROMMode::Null(0),
            write_buffer:   0,
            read_buffer:    0,
        })
    }

    /// Create EEPROM from a new file.
    pub fn new(file: Option<File>) -> Self {
        Self {
            ram:            Vec::new(),
            file:           file,
            dirty:          false,

            size:           EEPROMSize::Unknown,
            mode:           EEPROMMode::Null(0),
            write_buffer:   0,
            read_buffer:    0,
        }
    }
}

impl EEPROM {
    fn set_size_512(&mut self) {
        if self.size == EEPROMSize::Unknown {
            self.size = EEPROMSize::B512;
            self.ram = vec![0; EEPROM_512_SIZE];
            if let Some(file) = &mut self.file {
                file.set_len((EEPROM_512_SIZE + 4) as u64).unwrap();
                file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
                file.write_all(EEPROM_512_CODE.as_bytes()).expect("Couldn't write to save file!");
            }
        }
    }
    fn set_size_8k(&mut self) {
        if self.size == EEPROMSize::Unknown {
            self.size = EEPROMSize::K8;
            self.ram = vec![0; EEPROM_8K_SIZE];
            if let Some(file) = &mut self.file {
                file.set_len((EEPROM_8K_SIZE + 4) as u64).unwrap();
                file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
                file.write_all(EEPROM_8K_CODE.as_bytes()).expect("Couldn't write to save file!");
            }
        }
    }

    fn prepare_read_buffer(&mut self, byte_addr: usize) {
        for i in 0..8 {
            let addr = byte_addr + i;
            let shift = i * 8;
            self.read_buffer |= (self.ram[addr] as u64) << shift;
        }
    }
    fn writeback(&mut self, byte_addr: usize, write_buffer: u64) {
        for i in 0..8 {
            let addr = byte_addr + i;
            let shift = i * 8;
            self.ram[addr] = (write_buffer >> shift) as u8;
        }
        self.dirty = true;
    }
}

impl MemInterface8 for EEPROM {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.read_halfword(addr) as u8
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.write_halfword(addr, data as u16);
    }

    fn read_halfword(&mut self, _addr: u32) -> u16 {
        use EEPROMMode::*;
        match self.mode {
            Null(_) => 1,
            Write(WRITE_STREAM_512_LEN) => {
                self.set_size_512();
                let addr = (self.write_buffer >> 65) as usize;
                let data = (self.write_buffer >> 1) as u64;
                self.writeback(addr << 3, data);
                self.write_buffer = 0;
                self.mode = Null(0);
                1
            },
            Write(WRITE_STREAM_8K_LEN) => {
                self.set_size_8k();
                let addr = (self.write_buffer >> 65) as usize;
                let data = (self.write_buffer >> 1) as u64;
                self.writeback(addr << 3, data);
                self.write_buffer = 0;
                self.mode = Null(0);
                1
            },
            Write(n) => panic!(format!("Writing with an unknown address: {}", n)),
            PrepRead(READ_STREAM_512_LEN) => {
                self.set_size_512();
                let addr = (self.write_buffer >> 1) as usize;
                self.prepare_read_buffer(addr << 3);
                self.write_buffer = 0;
                self.mode = Read(1);
                0
            },
            PrepRead(READ_STREAM_8K_LEN) => {
                self.set_size_8k();
                let addr = (self.write_buffer >> 1) as usize;
                self.prepare_read_buffer(addr << 3);
                self.write_buffer = 0;
                self.mode = Read(1);
                0
            },
            PrepRead(_) => panic!("Reading with an unknown address"),
            // First 4 bits are ignored.
            Read(n) if n < 4 => {
                self.mode = Read(n + 1);
                0
            },
            Read(n) if n >= 67 => {
                let bit = self.read_buffer >> 63;
                self.read_buffer <<= 1;
                self.mode = Null(0);
                bit as u16
            },
            Read(n) => {
                let bit = self.read_buffer >> 63;
                self.read_buffer <<= 1;
                self.mode = Read(n + 1);
                bit as u16
            }
        }
    }
    fn write_halfword(&mut self, _addr: u32, data: u16) {
        use EEPROMMode::*;
        let bit = data & 1;
        self.mode = match self.mode {
            Null(1) => if bit == 0 {
                Write(0)
            } else {
                PrepRead(0)
            },
            Null(_) => Null(bit),
            Write(n) => {
                self.write_buffer = (self.write_buffer << 1) | (bit as u128);
                Write(n + 1)
            },
            PrepRead(n) => {
                self.write_buffer = (self.write_buffer << 1) | (bit as u128);
                PrepRead(n + 1)
            },
            Read(_) => panic!("shouldn't write while reading")
        };
    }
}

impl SaveRAM for EEPROM {
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
