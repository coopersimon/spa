
use bitflags::bitflags;
use std::path::PathBuf;
use crate::utils::bits::u8;

use super::{SaveSPI, State, SaveType, file::SaveFile};

bitflags!{
    #[derive(Default)]
    struct Status: u8 {
        const SMALL         = u8::bits(4, 7);
        const WRITE_PROTECT = u8::bits(2, 3);
        const WRITE_ENABLE  = u8::bit(1);
        const WRITE_ACTIVE  = u8::bit(0);
    }
}

const SMALL_EEPROM_SIZE: usize = 512;
const MEDIUM_EEPROM_SIZE: usize = 64 * 1024;
pub const LARGE_EEPROM_SIZE: usize = 128 * 1024;

/// EEPROM with 9-bit address (4kbit / 512B)
pub struct SmallEEPROM {
    file:       SaveFile,

    status:     Status,
    state:      State,
    can_read:   bool,
}

impl SmallEEPROM {
    pub fn new(save_path: &Option<PathBuf>, write_enable: bool) -> Self {
        println!("detected EEPROM 9-bit");
        Self {
            file:   SaveFile::from_type(save_path, SaveType::SmallEEPROM(SMALL_EEPROM_SIZE)),

            status:     Status::SMALL | if write_enable {Status::WRITE_ENABLE} else {Status::empty()},
            state:      State::Idle,
            can_read:   false,
        }
    }
    
    pub fn new_from_file(file: SaveFile) -> Self {
        Self {
            file,

            status:     Status::SMALL,
            state:      State::Idle,
            can_read:   false,
        }
    }
}

impl SaveSPI for SmallEEPROM {
    fn read_byte(&mut self) -> u8 {
        use State::*;
        match self.state {
            ReadStatus if self.can_read => {
                self.state = Idle;
                self.status.bits()
            },
            Read(addr) if self.can_read => {
                let data = self.file.read_byte(addr);
                self.state = Read(addr + 1);
                data
            },
            _ => 0,
        }
    }
    fn write_byte(&mut self, data: u8) {
        use State::*;
        match self.state {
            Idle => match data {
                // All types
                0x06 => self.status.insert(Status::WRITE_ENABLE),
                0x04 => self.status.remove(Status::WRITE_ENABLE),
                0x05 => self.state = ReadStatus,
                0x01 => self.state = WriteStatus,

                0x03 => self.state = PrepRead { byte: 0, addr: 0 },
                0x0B => self.state = PrepRead { byte: 0, addr: 0x100 },

                0x02 => self.state = PrepWrite { byte: 0, addr: 0 },
                0x0A => self.state = PrepWrite { byte: 0, addr: 0x100 },

                0x0 => {},
                _ => panic!("unsupported save ram op {:X}", data),
            },
            PrepRead{byte: _, addr} => {
                let addr = addr | (data as u32);
                self.state = Read(addr);
            },
            PrepWrite{byte: _, addr} => {
                let addr = addr | (data as u32);
                self.state = Write(addr);
            },
            WriteStatus => {
                self.status = Status::from_bits_truncate(data);
                self.state = Idle;
            },
            Write(addr) => {
                self.file.write_byte(addr, data);
                self.state = Write(addr + 1);
            },
            _ => self.can_read = true,
        }
    }

    fn deselect(&mut self) {
        self.state = State::Idle;
        self.can_read = false;
    }

    fn flush(&mut self) {
        self.file.flush();
    }
}

/// EEPROM with 16-bit address (8-512kbit / 1-64kB)
pub struct MediumEEPROM {
    file:       SaveFile,

    status:     Status,
    state:      State,
    can_read:   bool,
}

impl MediumEEPROM {
    pub fn new(save_path: &Option<PathBuf>, write_enable: bool) -> Self {
        println!("detected EEPROM 16-bit");
        Self {
            file:   SaveFile::from_type(save_path, SaveType::EEPROM(MEDIUM_EEPROM_SIZE)),

            status:     if write_enable {Status::WRITE_ENABLE} else {Status::empty()},
            state:      State::Idle,
            can_read:   false,
        }
    }
    
    pub fn new_from_file(file: SaveFile) -> Self {
        Self {
            file,

            status:     Status::empty(),
            state:      State::Idle,
            can_read:   false,
        }
    }
}

impl SaveSPI for MediumEEPROM {
    fn read_byte(&mut self) -> u8 {
        use State::*;
        match self.state {
            ReadStatus if self.can_read => {
                self.state = Idle;
                self.status.bits()
            },
            Read(addr) if self.can_read => {
                let data = self.file.read_byte(addr);
                self.state = Read(addr + 1);
                data
            },
            _ => 0,
        }
    }
    fn write_byte(&mut self, data: u8) {
        use State::*;
        match self.state {
            Idle => match data {
                // All types
                0x06 => self.status.insert(Status::WRITE_ENABLE),
                0x04 => self.status.remove(Status::WRITE_ENABLE),
                0x05 => self.state = ReadStatus,
                0x01 => self.state = WriteStatus,

                0x03 => self.state = PrepRead { byte: 0, addr: 0 },

                0x02 => self.state = PrepWrite { byte: 0, addr: 0 },

                0x0 => {},
                _ => panic!("unsupported save ram op {:X}", data),
            },
            PrepRead{byte: 1, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = Read(addr);
            },
            PrepRead{byte: 0, addr: _} => {
                let addr = data as u32;
                self.state = PrepRead{byte: 1, addr};
            },
            PrepWrite{byte: 1, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = Write(addr);
            },
            PrepWrite{byte: 0, addr: _} => {
                let addr = data as u32;
                self.state = PrepWrite{byte: 1, addr};
            },
            WriteStatus => {
                self.status = Status::from_bits_truncate(data);
                self.state = Idle;
            },
            Write(addr) => {
                self.file.write_byte(addr, data);
                self.state = Write(addr + 1);
            },
            _ => self.can_read = true,
        }
    }

    fn deselect(&mut self) {
        self.state = State::Idle;
        self.can_read = false;
    }

    fn flush(&mut self) {
        self.file.flush();
    }
}


/// EEPROM with 24-bit address (1Mbit / 128kB)
pub struct LargeEEPROM {
    file:       SaveFile,

    status:     Status,
    state:      State,
    can_read:   bool,
}

impl LargeEEPROM {
    pub fn new(save_path: &Option<PathBuf>, write_enable: bool) -> Self {
        println!("detected EEPROM 17-bit");
        Self {
            file:   SaveFile::from_type(save_path, SaveType::EEPROM(LARGE_EEPROM_SIZE)),

            status:     if write_enable {Status::WRITE_ENABLE} else {Status::empty()},
            state:      State::Idle,
            can_read:   false,
        }
    }
    
    pub fn new_from_file(file: SaveFile) -> Self {
        Self {
            file,

            status:     Status::empty(),
            state:      State::Idle,
            can_read:   false,
        }
    }
}

impl SaveSPI for LargeEEPROM {
    fn read_byte(&mut self) -> u8 {
        use State::*;
        match self.state {
            ReadStatus if self.can_read => {
                self.state = Idle;
                self.status.bits()
            },
            Read(addr) if self.can_read => {
                let data = self.file.read_byte(addr);
                self.state = Read(addr + 1);
                data
            },
            _ => 0,
        }
    }
    fn write_byte(&mut self, data: u8) {
        use State::*;
        match self.state {
            Idle => match data {
                // All types
                0x06 => self.status.insert(Status::WRITE_ENABLE),
                0x04 => self.status.remove(Status::WRITE_ENABLE),
                0x05 => self.state = ReadStatus,
                0x01 => self.state = WriteStatus,

                0x03 => self.state = PrepRead { byte: 0, addr: 0 },

                0x02 => self.state = PrepWrite { byte: 0, addr: 0 },

                0x0 => {},
                _ => panic!("unsupported save ram op {:X}", data),
            },
            PrepRead{byte: 2, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = Read(addr);
            },
            PrepRead{byte: 1, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = PrepRead{byte: 2, addr};
            },
            PrepRead{byte: 0, addr: _} => {
                let addr = data as u32;
                self.state = PrepRead{byte: 1, addr};
            },
            PrepWrite{byte: 2, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = Write(addr);
            },
            PrepWrite{byte: 1, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = PrepWrite{byte: 2, addr};
            },
            PrepWrite{byte: 0, addr: _} => {
                let addr = data as u32;
                self.state = PrepWrite{byte: 1, addr};
            },
            WriteStatus => {
                self.status = Status::from_bits_truncate(data);
                self.state = Idle;
            },
            Write(addr) => {
                self.file.write_byte(addr, data);
                self.state = Write(addr + 1);
            },
            _ => self.can_read = true,
        }
    }

    fn deselect(&mut self) {
        self.state = State::Idle;
        self.can_read = false;
    }

    fn flush(&mut self) {
        self.file.flush();
    }
}
