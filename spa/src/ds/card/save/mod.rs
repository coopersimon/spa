// SPI for interfacing with save RAM.

mod file;
mod eeprom;
mod flash;

use std::{
    fs::OpenOptions,
    path::PathBuf
};
use crate::utils::bits::u8;

use eeprom::*;
use flash::*;
use file::*;

/// Save type extracted from save file,
/// or specified by user.
pub enum SaveType {
    SmallEEPROM(usize),
    EEPROM(usize),
    FLASH(usize)
}

impl SaveType {
    /// Construct a header for the save file.
    fn to_buffer(&self) -> Vec<u8> {
        use SaveType::*;
        match self {
            SmallEEPROM(n) => {
                let size = n;
                let mut string = String::new();
                string.push_str(SMALL_EEPROM_CODE);
                string.push_str(&format!("{:07}", size));
                string.as_bytes().to_vec()
            },
            EEPROM(n) => {
                let size_kb = n / 1024;
                let mut string = String::new();
                string.push_str(EEPROM_CODE);
                string.push_str(&format!("{:07}", size_kb));
                string.as_bytes().to_vec()
            },
            FLASH(n) => {
                let size_kb = n / 1024;
                let mut string = String::new();
                string.push_str(FLASH_CODE);
                string.push_str(&format!("{:07}", size_kb));
                string.as_bytes().to_vec()
            }
        }
    }
}

/// A save device: EEPROM or FLASH
/// 
/// Serial peripheral interface.
trait SaveSPI {
    fn read_byte(&mut self) -> u8;
    fn write_byte(&mut self, data: u8);

    fn deselect(&mut self);

    fn flush(&mut self);
}

enum State {
    Idle,
    ReadStatus,
    WriteStatus,
    PrepRead{
        byte: u8,
        addr: u32
    },
    Read(u32),
    PrepWrite{
        byte: u8,
        addr: u32
    },
    Write(u32)
}

enum Device {
    Save(Box<dyn SaveSPI + Send>),
    Unknown(Box<UnknownDevice>)
}

pub struct SPI {
    device: Device,
}

impl SPI {
    pub fn new(save_path: Option<PathBuf>) -> Self {
        if let Some(path) = &save_path {
            if let Ok(mut save_file) = OpenOptions::new().read(true).write(true).open(path) {
                let device: Box<dyn SaveSPI + Send> = match file::type_from_file(&mut save_file) {
                    SaveType::SmallEEPROM(n) => {
                        let save = SaveFile::from_file(Some(save_file), n).unwrap();
                        Box::new(SmallEEPROM::new_from_file(save))
                    },
                    SaveType::EEPROM(LARGE_EEPROM_SIZE) => {
                        let save = SaveFile::from_file(Some(save_file), LARGE_EEPROM_SIZE).unwrap();
                        Box::new(LargeEEPROM::new_from_file(save))
                    },
                    SaveType::EEPROM(n) => {
                        let save = SaveFile::from_file(Some(save_file), n).unwrap();
                        Box::new(MediumEEPROM::new_from_file(save))
                    },
                    SaveType::FLASH(n) => {
                        let save = SaveFile::from_file(Some(save_file), n).unwrap();
                        Box::new(Flash::new_from_file(save))
                    }
                };
                return Self {
                    device: Device::Save(device)
                };
            }
        }
        
        Self {
            device: Device::Unknown(Box::new(UnknownDevice::new(save_path)))
        }
    }

    pub fn deselect(&mut self) {
        match &mut self.device {
            Device::Save(d) => d.deselect(),
            Device::Unknown(d) => d.deselect(),
        }
    }

    pub fn read(&mut self) -> u8 {
        match &mut self.device {
            Device::Save(d) => d.read_byte(),
            Device::Unknown(_) => 0,
        }
    }

    pub fn write(&mut self, data: u8) {
        match &mut self.device {
            Device::Save(d) => d.write_byte(data),
            Device::Unknown(d) => {
                if let Some(mut save_device) = d.write_byte(data) {
                    save_device.write_byte(data);
                    self.device = Device::Save(save_device);
                }
            },
        }
    }

    pub fn flush(&mut self) {
        if let Device::Save(device) = &mut self.device {
            device.flush();
        }
    }
}

/// When starting up with no save file, we don't know
/// which the game will use.
/// 
/// Contains hints that can be used to detect the type.
struct UnknownDevice {
    save_path:          Option<PathBuf>,

    state:              State,
    write_enable:       bool,
    block_bytes_read:   u32,

    /// If 0xF0 was written to status, this is small EEPROM.
    small_eeprom_status:    bool,
    /// If an address MSB larger than 1 was written, this
    /// is either medium EEPROM or FLASH.
    large_addr_msb:         bool,
    /// Estimation of number of address bytes used.
    estimated_addr_size:    usize,
    /// Previous address used.
    previous_addr:          u32,
    /// Number of bytes read by last read op before deselection.
    prev_bytes_read:        u32,
}

impl UnknownDevice {
    fn new(save_path: Option<PathBuf>) -> Self {
        Self {
            save_path:          save_path,        
            state:              State::Idle,
            write_enable:       false,
            block_bytes_read:   0,

            small_eeprom_status:    false,
            large_addr_msb:         false,
            estimated_addr_size:    0,
            previous_addr:          0,
            prev_bytes_read:        0,
        }
    }

    /// Write to the unknown device.
    /// 
    /// It might figure out which save type this game is using,
    /// if so it will return it.
    fn write_byte(&mut self, data: u8) -> Option<Box<dyn SaveSPI + Send>> {
        use State::*;
        match self.state {
            Idle => match data {
                /*** Various immutable commands... ***/
                0x06 => self.write_enable = true,
                0x04 => self.write_enable = false,
                0x05 => self.state = ReadStatus,
                0x01 => self.state = WriteStatus,
                0x03 => {
                    self.state = PrepRead { byte: 0, addr: 0 };
                    self.block_bytes_read = 0;
                },

                /*** Mutable commands. We need to assert the save type. ***/

                // Usually only called by EEPROM.
                0x02 => return Some(if self.small_eeprom_status {
                    Box::new(SmallEEPROM::new(&self.save_path, self.write_enable))
                } else {
                    match self.estimated_addr_size {
                        1 => Box::new(SmallEEPROM::new(&self.save_path, self.write_enable)),
                        2 => Box::new(MediumEEPROM::new(&self.save_path, self.write_enable)),
                        3 => if self.large_addr_msb {
                            Box::new(MediumEEPROM::new(&self.save_path, self.write_enable))
                        } else {
                            Box::new(LargeEEPROM::new(&self.save_path, self.write_enable))
                        },
                        _ => panic!("unknown save RAM"),
                    }
                }),
                // Upper write for small EEPROM, write for FLASH
                0x0A => return Some(if self.small_eeprom_status || self.estimated_addr_size < 3 {
                    Box::new(SmallEEPROM::new(&self.save_path, self.write_enable))
                } else {
                    Box::new(Flash::new(&self.save_path, self.write_enable))
                }),

                // Only small EEPROM uses this command!
                0x0B => return Some(Box::new(SmallEEPROM::new(&self.save_path, self.write_enable))),

                _ => {},    // Don't care.
            },
            WriteStatus => {
                if data & 0xF0 == 0xF0 {
                    self.small_eeprom_status = true;
                }
            },
            PrepRead { byte: 3, addr } => {
                // Assume this is the first dummy write before reading.
                self.state = Read(addr);
            },
            PrepRead { byte, addr } => {
                let addr = (addr << 8) | (data as u32);
                self.state = PrepRead { byte: byte + 1, addr };
            },
            Read(_) => {
                self.block_bytes_read += 1;
            },
            // We don't care.
            _ => {}
        }
        
        // We can't figure out the save type yet!
        None
    }

    fn deselect(&mut self) {
        use State::*;
        match self.state {
            PrepRead { byte: 2, addr } => {
                // Wrote one byte of address, one dummy write, and one read.
                // This is probably a small EEPROM.
                self.estimated_addr_size = 1;
                self.previous_addr = addr >> 8;
                self.prev_bytes_read = self.block_bytes_read;
            },
            PrepRead { byte: 3, addr } => {
                // Wrote two bytes of address, one dummy write, and one read.
                // This is probably a mid-sized EEPROM.
                // (It might be a small EEPROM)
                self.estimated_addr_size = 2;
                self.previous_addr = addr >> 8;
                self.prev_bytes_read = self.block_bytes_read;
                // TODO: addr offset check for small EEPROM ?
            },
            Read(addr) => {
                // Wrote 4+ bytes. This is up to 3 bytes of address.
                if self.estimated_addr_size == 0 {
                    self.estimated_addr_size = 3;
                    if (addr >> 16) > 1 {
                        self.large_addr_msb = true;
                    }
                } else if self.estimated_addr_size == 3 {
                    // Not our first read block.
                    // We can assume that if the last read was at addr A and read N bytes,
                    // This read block started at addr (A+N).
                    if ((self.previous_addr >> 8) + (self.prev_bytes_read + 1)) == (addr >> 8) {
                        //println!("prev addr: {:X}", self.previous_addr, self.prev_bytes_read, );
                        // If the address is only 2 bytes, we need to assume there was one extra byte read.
                        self.estimated_addr_size = 2;
                        self.previous_addr = addr >> 8;
                    } else if (self.previous_addr + self.prev_bytes_read) == addr {
                        // We can be fairly sure this is a 3 byte address.
                        // TODO: extra var to track this?
                    }

                    if (addr >> 16) > 1 {
                        self.large_addr_msb = true;
                    }
                }
                self.previous_addr = addr;
                self.prev_bytes_read = self.block_bytes_read;
            },
            _ => {}
        }

        self.state = Idle;
    }
}
