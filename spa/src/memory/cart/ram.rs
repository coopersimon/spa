/// Save RAM
use std::{
    path::Path,
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

const SRAM_CODE: &'static str = "SRAM";
//const EEPROM_512_CODE: &'static str = "EPR5";
//const EEPROM_8K_CODE: &'static str = "EPR8";
const FLASH_64_CODE: &'static str = "FL64";
const FLASH_128_CODE: &'static str = "F128";

const SRAM_SIZE: usize = 32 * 1024;
//const EEPROM_512_SIZE: usize = 512;
//const EEPROM_8K_SIZE: usize = 8 * 1024;
const FLASH_64_SIZE: usize = 64 * 1024;
const FLASH_128_SIZE: usize = 128 * 1024;

/// Detect the save RAM from the game pak ROM and return it.
/// 
/// It will try to open the file at save_path, and will create it if it doesn't exist.
/// 
/// If no save_path is provided, the save data will be lost on shutdown!
pub fn make_save_ram(rom: &[u8], save_path: Option<&Path>) -> Box<dyn SaveRAM> {

    // See if save file exists.
    let file = if let Some(path) = save_path {
        if let Ok(file) = File::open(path) {
            //let mut save_reader = BufReader::new(file);
            //save_reader.read_exact(&mut ram.data).map_err(|e| e.to_string())?;
            match make_from_existing(file) {
                Ok(save_ram) => return save_ram,
                Err(e) => println!("Can't use existing file: {}", e)
            }
        }

        Some(File::create(path).unwrap())
    } else {
        None
    };

    use regex::bytes::Regex;
    let re = Regex::new("(EEPROM|SRAM|FLASH|FLASH512|FLASH1M)_V...").expect("couldn't compile regex");
    if let Some(found) = re.find(rom) {
        println!("Found: {}", String::from_utf8(found.as_bytes().to_vec()).unwrap());
    
        match found.as_bytes().len() {
            9 => Box::new(SRAM::new(file)),
            10 | 13 => Box::new(FLASH::new_64(file)),
            11 => panic!("EEPROM not supported yet"),
            12 => Box::new(FLASH::new_128(file)),
            _ => unreachable!()
        }
    } else {
        Box::new(NoSaveRAM{})
    }
}

fn make_from_existing(mut file: File) -> Result<Box<dyn SaveRAM>> {
    let mut buf = vec![0; 4];
    file.read_exact(&mut buf)?;
    match String::from_utf8(buf).unwrap().as_str() {
        SRAM_CODE => Ok(Box::new(SRAM::new_from_file(file)?)),
        FLASH_64_CODE => Ok(Box::new(FLASH::new_from_file(file, FLASH_64_SIZE)?)),
        FLASH_128_CODE => Ok(Box::new(FLASH::new_from_file(file, FLASH_128_SIZE)?)),
        c => panic!(format!("Unknown save code {}", c))
    }
}

/// Save RAM interface
pub trait SaveRAM: MemInterface8 {
    fn flush(&mut self);
}

/// For games with no save backup.
pub struct NoSaveRAM {}

impl MemInterface8 for NoSaveRAM {
    fn read_byte(&self, _addr: u32) -> u8 {
        0
    }
    fn write_byte(&mut self, _addr: u32, _data: u8) {}
}

impl SaveRAM for NoSaveRAM {
    fn flush(&mut self) {}
}


/// SRAM. Simple 32kB region of 8-bit battery-backed memory.
pub struct SRAM {
    ram:    Vec<u8>,
    file:   Option<File>,
    dirty:  bool,
}

impl SRAM {
    /// Create SRAM from an existing save file.
    pub fn new_from_file(mut file: File) -> Result<Self> {
        let mut buffer = vec![0; SRAM_SIZE];
        file.read_to_end(&mut buffer)?;
        Ok(Self {
            ram:    buffer,
            file:   Some(file),
            dirty:  false,
        })
    }

    /// Create SRAM from a new file.
    pub fn new(mut file: Option<File>) -> Self {
        if let Some(file) = &mut file {
            file.set_len((SRAM_SIZE + 4) as u64).unwrap();
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(SRAM_CODE.as_bytes()).expect("Couldn't write to save file!");
        }
        Self {
            ram:    vec![0; SRAM_SIZE],
            file:   file,
            dirty:  true,
        }
    }
}

impl MemInterface8 for SRAM {
    fn read_byte(&self, addr: u32) -> u8 {
        self.ram[addr as usize]
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.ram[addr as usize] = data;
        self.dirty = true;
    }
}

impl SaveRAM for SRAM {
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

mod flashdev {
    pub const SST_64: u16 = 0xD4BF;
    pub const MACRONIX_64: u16 = 0x1CC2;
    pub const PANASONIC_64: u16 = 0x1B32;
    pub const ATMEL_64: u16 = 0x3D1F;
    pub const SANYO_128: u16 = 0x1362;
    pub const MACRONIX_128: u16 = 0x09C2;
}

#[derive(Clone, Copy)]
enum FlashMode {
    Read,
    ModeAA,
    Mode55,
    Erase,      // 0x80
    GetID,      // 0x90
    Write,      // 0xA0
    SelectBank, // 0xB0
}

pub struct FLASH {
    ram:            Vec<u8>,
    file:           Option<File>,
    bank_offset:    usize,
    mode:           FlashMode,
    device_type:    u16,
    dirty:          bool,
}

impl FLASH {
    /// Create FLASH from an existing save file.
    pub fn new_from_file(mut file: File, size: usize) -> Result<Self> {
        let mut buffer = vec![0; size];
        file.read_to_end(&mut buffer)?;
        Ok(Self {
            ram:            buffer,
            file:           Some(file),
            bank_offset:    0,
            mode:           FlashMode::Read,
            device_type:    if size > FLASH_64_SIZE {flashdev::SANYO_128} else {flashdev::PANASONIC_64},
            dirty:          false,
        })
    }

    /// Create FLASH 64KB from a new file.
    pub fn new_64(mut file: Option<File>) -> Self {
        if let Some(file) = &mut file {
            file.set_len((FLASH_64_SIZE + 4) as u64).unwrap();
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(FLASH_64_CODE.as_bytes()).expect("Couldn't write to save file!");
        }
        Self {
            ram:            vec![0xFF; FLASH_64_SIZE],
            file:           file,
            bank_offset:    0,
            mode:           FlashMode::Read,
            device_type:    flashdev::PANASONIC_64,
            dirty:          true,
        }
    }

    /// Create FLASH 128KB from a new file.
    pub fn new_128(mut file: Option<File>) -> Self {
        if let Some(file) = &mut file {
            file.set_len((FLASH_128_SIZE + 4) as u64).unwrap();
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(FLASH_128_CODE.as_bytes()).expect("Couldn't write to save file!");
        }
        Self {
            ram:            vec![0xFF; FLASH_128_SIZE],
            file:           file,
            bank_offset:    0,
            mode:           FlashMode::Read,
            device_type:    flashdev::SANYO_128,
            dirty:          true,
        }
    }
}

// Internal
impl FLASH {
    fn erase_all(&mut self) {
        for b in &mut self.ram {
            *b = 0xFF;
        }
        self.dirty = true;
    }

    fn erase_sector(&mut self, start: usize) {
        let start = start + self.bank_offset;
        let end = start + 0x1000;
        for b in &mut self.ram[start..end] {
            *b = 0xFF;
        }
        self.dirty = true;
    }
}

impl MemInterface8 for FLASH {
    fn read_byte(&self, addr: u32) -> u8 {
        use FlashMode::*;
        let data = match self.mode {
            // SST
            GetID => if addr == 0 {
                u16::lo(self.device_type)
            } else {
                u16::hi(self.device_type)
            },
            _ => self.ram[(addr as usize) + self.bank_offset],
        };
        println!("FLASH: {:X} -> d{:X}", addr, data);
        data
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        println!("FLASH: d{:X} -> {:X}", data, addr);
        use FlashMode::*;
        match self.mode {
            ModeAA if addr == 0x2AAA && data == 0x55 => {
                self.mode = Mode55;
            },
            Mode55 if addr == 0x5555 => {
                self.mode = match data {
                    0x80 => Erase,
                    0x10 => {
                        self.erase_all();
                        Read
                    },
                    0x90 => GetID,
                    0xA0 => Write,
                    0xB0 => SelectBank,
                    0xF0 => Read,
                    _ => self.mode
                };
            },
            Mode55 if data == 0x30 => {
                let sector_start = addr & 0xF000;
                self.erase_sector(sector_start as usize);
                self.mode = Read;
                self.dirty = true;
            },
            Write => {
                self.ram[(addr as usize) + self.bank_offset] = data;
                self.mode = Read;
                self.dirty = true;
            },
            SelectBank if addr == 0x5555 => {
                self.bank_offset = (data as usize) * 0x10000;
                self.mode = Read;
            },
            _ if addr == 0x5555 && data == 0xAA => {
                self.mode = ModeAA;
            },
            _ => {}
        }
    }
}

impl SaveRAM for FLASH {
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
