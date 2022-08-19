// Save backup for NDS.

use std::{
    io::{
        Result,
        Read,
        Write,
        Seek,
        SeekFrom
    },
    fs::File,
    path::PathBuf
};
use super::SaveType;

pub const HEADER_SIZE: usize = 8;
pub const SMALL_EEPROM_CODE: &'static str = "S";
pub const EEPROM_CODE: &'static str = "E";
pub const FLASH_CODE: &'static str = "F";

pub fn type_from_file(file: &mut File) -> SaveType {
    let mut buffer = vec![0; HEADER_SIZE];

    file.seek(SeekFrom::Start(0)).unwrap();
    file.read_exact(&mut buffer).unwrap();

    let code = String::from_utf8(buffer).unwrap();
    match &code[..1] {
        SMALL_EEPROM_CODE => {
            let size = code[1..8].parse::<u32>().unwrap();
            SaveType::EEPROM(size as usize)
        },
        EEPROM_CODE => {
            let size_kb = code[1..8].parse::<u32>().unwrap();
            SaveType::EEPROM((size_kb as usize) * 1024)
        },
        FLASH_CODE => {
            let size_kb = code[1..8].parse::<u32>().unwrap();
            SaveType::FLASH((size_kb as usize) * 1024)
        }
        _ => panic!("unknown save type"),
    }
}

/// Deals with storing save data, and backing up
/// to disk (optionally).
pub struct SaveFile {
    buffer: Vec<u8>,
    file:   Option<File>,
    dirty:  bool,
}

impl SaveFile {
    /// Construct a save buffer from a file,
    /// or a known save type.
    pub fn from_file(file: Option<File>, size: usize) -> Result<Self> {
        let mut buffer = vec![0; size];

        if let Some(mut file) = file {
            file.seek(SeekFrom::Start(HEADER_SIZE as u64))?;
            file.read_exact(&mut buffer)?;
            Ok(Self {
                buffer,
                file: Some(file),
                dirty: false,
            })
        } else {
            Ok(Self {
                buffer, file,
                dirty: false,
            })
        }
    }

    /// Construct a new save file from a path,
    /// with an inferred type.
    pub fn from_type(file_path: &Option<PathBuf>, save_type: SaveType) -> Self {
        let file = file_path.as_ref().map(|path| {
            let mut file = File::create(path).expect("Couldn't make save file!");
            // Write header.
            file.seek(SeekFrom::Start(0)).expect("Couldn't seek to start of save file!");
            file.write_all(&save_type.to_buffer()).expect("Couldn't write header to save file!");
            file
        });

        let size = match save_type {
            SaveType::SmallEEPROM(n) => n,
            SaveType::EEPROM(n) => n,
            SaveType::FLASH(n) => n
        };

        Self {
            buffer: vec![0; size],
            file:   file,
            dirty:  false,
        }
    }

    pub fn read_byte(&self, addr: u32) -> u8 {
        self.buffer[addr as usize]
    }

    pub fn write_byte(&mut self, addr: u32, data: u8) {
        self.buffer[addr as usize] = data;
        self.dirty = true;
    }

    pub fn flush(&mut self) {
        if self.dirty {
            self.dirty = false;
            if let Some(file) = &mut self.file {
                file.seek(SeekFrom::Start(HEADER_SIZE as u64)).expect("Couldn't seek to start of save file!");
                file.write_all(&self.buffer).expect("Couldn't write to save file!");
            }
        }
    }
}