mod header;
mod save;

use bitflags::bitflags;
use parking_lot::Mutex;
use std::{
    io::{
        Result,
        Read,
        Seek,
        SeekFrom
    },
    fs::File,
    path::{Path, PathBuf},
    sync::Arc
};

use crate::utils::{
    bits::u16,
    bytes,
    meminterface::{MemInterface16, MemInterface32}
};
use crate::ds::interrupt::Interrupts;
pub use header::CardHeader;
use save::SPI;

bitflags!{
    #[derive(Default)]
    pub struct GamecardControl: u16 {
        const NDS_SLOT_ENABLE   = u16::bit(15);
        const TRANSFER_IRQ      = u16::bit(14);
        const NDS_SLOT_MODE     = u16::bit(13);
        const SPI_BUSY          = u16::bit(7);
        const SPI_HOLD          = u16::bit(6);
        const SPI_BAUDRATE      = u16::bits(0, 1);
    }
}

bitflags!{
    #[derive(Default)]
    pub struct RomControlHi: u16 {
        const START_STAT    = u16::bit(15);
        const DATA_WRITE    = u16::bit(14);
        const RELEASE       = u16::bit(13);
        const KEY1_GAP_CLK  = u16::bit(12);
        const TRANSFER_RATE = u16::bit(11);
        const BLOCK_SIZE    = u16::bits(8, 10);
        const DATA_STATUS   = u16::bit(7);
        const KEY2_COMMAND  = u16::bit(6);
        const KEY1_GAP2_LEN = u16::bits(0, 5);
    }
}

bitflags!{
    #[derive(Default)]
    pub struct RomControlLo: u16 {
        const KEY2_APPLY    = u16::bit(15);
        const KEY2_SP       = u16::bit(14);
        const KEY2_DATA     = u16::bit(13);
        const KEY1_GAP1_LEN = u16::bits(0, 12);
    }
}

/// We read 16kB at a time from disk.
const ROM_BUFFER_SIZE: u32 = 16 * 1024;

/// DS Card attached to IO ports.
pub struct DSCardIO {
    card:       Arc<Mutex<DSCard>>
}

impl DSCardIO {
    pub fn new(rom_path: &Path, save_path: Option<PathBuf>, key1: Vec<u32>) -> Result<(Self, Self)> {
        let card = DSCard::new(rom_path, save_path, key1)?;
        let card_arc = Arc::new(Mutex::new(card));
        Ok((DSCardIO{
            card: card_arc.clone()
        }, DSCardIO{
            card: card_arc
        }))
    }

    /// To be called by only the processor with access rights.
    /// Returns any interrupts, plus a bool indicating if DMA should begin.
    pub fn clock(&mut self, cycles: usize) -> (Interrupts, bool) {
        self.card.lock().clock(cycles)
    }
    
    /// To be called by only the ARM7 processor.
    pub fn flush_save(&mut self) {
        self.card.lock().flush_save();
    }

    pub fn get_header(&self) -> CardHeader {
        let mut data = vec![0; 0x200];
        self.card.lock().load_data(0, &mut data);
        CardHeader::new(data)
    }

    pub fn get_rom_id(&self) -> u32 {
        u32::from_le_bytes(self.card.lock().rom_id)
    }

    pub fn load_data(&self, from_addr: u32, into_buffer: &mut [u8]) {
        self.card.lock().load_data(from_addr, into_buffer);
    }

    /// Fast boot mode setup.
    pub fn fast_boot(&mut self, rom_ctrl_init: u32) {
        self.card.lock().fast_boot(rom_ctrl_init);
    }
}

impl MemInterface32 for DSCardIO {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.card.lock().read_byte(addr)
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.card.lock().write_byte(addr, data);
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        self.card.lock().read_halfword(addr)
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.card.lock().write_halfword(addr, data);
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        self.card.lock().read_word(addr)
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        self.card.lock().write_word(addr, data);
    }
}

/// The DS card ROM.
struct DSCard {
    rom_file:       Option<File>,
    rom_buffer:     Vec<u8>,
    buffer_tag:     u32,
    read_addr:      u32,
    secure_block:   u32,

    spi_control:    GamecardControl,
    spi:            SPI,

    rom_control_lo: RomControlLo,
    rom_control_hi: RomControlHi,

    key1_instr: Vec<u32>, // 0x1048 byte key
    key1_secure: Vec<u32>, // 0x1048 byte key
    rom_id: [u8; 4],

    command: [u8; 8],
    seed_0: [u8; 8],
    seed_1: [u8; 8],

    key2_0: u64,
    key2_1: u64,

    /// Number of bytes to transfer.
    transfer_count: usize,
    /// Cycles needed to transfer:
    /// some games require this to be correct(ish)
    transfer_cycles: usize,
    cmd_encrypt_mode: CommandEncryptMode,
    data_state: DSCardDataState,

    dma_ready: bool,
    interrupt: bool,
}

impl DSCard {
    fn new(rom_path: &Path, save_path: Option<PathBuf>, key1: Vec<u32>) -> Result<Self> {
        let mut buffer = vec![0xFF; ROM_BUFFER_SIZE as usize];

        let rom_file: Option<File> = {
            let mut rom_file = File::open(rom_path)?;
            rom_file.seek(SeekFrom::Start(0))?;
            rom_file.read_exact(&mut buffer)?;
            Some(rom_file)
        };

        // Game ID code.
        let game_id = u32::from_le_bytes([buffer[0xC], buffer[0xD], buffer[0xE], buffer[0xF]]);
        let key1_instr = dscrypto::key1::init(game_id, &key1, 2, 2);
        let key1_secure = dscrypto::key1::init(game_id, &key1, 2, 3);

        // ROM ID
        let rom_id = if let Some(rom_file) = rom_file.as_ref() {
            let unit_code = buffer[0x12];
            let dsi = (unit_code & 2) == 2;
            let file_size_mb = rom_file.metadata().unwrap().len() / (1024 * 1024);
            let id_size = (file_size_mb as u8) - 1;
            let id_hi_flags =
                if id_size >= 0x7F {0x80} else {0x00} |
                if dsi {0xC0} else {0x00};
            [0xC2, id_size, 0x00, id_hi_flags]
        } else {
            [0xFF, 0xFF, 0xFF, 0xFF]
        };

        Ok(Self {
            rom_file:       rom_file,
            rom_buffer:     buffer,
            buffer_tag:     0,
            read_addr:      0,
            secure_block:   0,

            spi_control:    GamecardControl::default(),
            spi:            SPI::new(save_path),
            rom_control_lo: RomControlLo::default(),
            rom_control_hi: RomControlHi::default(),

            key1_instr,
            key1_secure,
            rom_id,

            command: [0; 8],
            seed_0: [0xE8, 0xE0, 0x6D, 0xC5, 0x58, 0, 0, 0],
            seed_1: [0x05, 0x9B, 0x9B, 0x87, 0x5C, 0, 0, 0],

            key2_0: 0,
            key2_1: 0,

            transfer_count: 0,
            transfer_cycles: 0,
            cmd_encrypt_mode: CommandEncryptMode::None,
            data_state: DSCardDataState::Dummy,

            dma_ready: false,
            interrupt: false,
        })
    }

    fn clock(&mut self, cycles: usize) -> (Interrupts, bool) {
        if self.transfer_cycles > 0 {
            self.transfer_cycles = self.transfer_cycles.saturating_sub(cycles);
            if self.transfer_cycles == 0 {
                self.card_ready();
            }
        }
        let interrupt = if std::mem::replace(&mut self.interrupt, false) {
            Interrupts::CARD_COMPLETE
        } else {
            Interrupts::empty()
        };
        (interrupt, self.dma_ready)
    }

    fn flush_save(&mut self) {
        self.spi.flush();
    }

    fn load_data(&mut self, from_addr: u32, into_buffer: &mut [u8]) {
        if let Some(rom_file) = self.rom_file.as_mut() {
            rom_file.seek(SeekFrom::Start(from_addr as u64)).unwrap();
            rom_file.read(into_buffer).unwrap();
        } else {
            // TODO: handle this more gracefully...
            panic!("no cart loaded");
        }
    }

    fn fast_boot(&mut self, rom_ctrl_init: u32) {
        // TODO: load seed 0
        //self.apply_key2_seeds();
        self.cmd_encrypt_mode = CommandEncryptMode::Key2;
        self.data_state = DSCardDataState::Key2Dummy;

        self.rom_control_lo = RomControlLo::from_bits_truncate(bytes::u32::lo(rom_ctrl_init));
        self.rom_control_hi = RomControlHi::from_bits_truncate(bytes::u32::hi(rom_ctrl_init));
    }
}

impl MemInterface16 for DSCard {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_01A0 => self.spi_control.bits(),
            0x0400_01A2 => {
                let data = self.spi.read();
                if !self.spi_control.contains(GamecardControl::SPI_HOLD) {
                    self.spi.deselect();
                }
                data as u16
            },
            0x0400_01A4 => self.rom_control_lo.bits(),
            0x0400_01A6 => self.rom_control_hi.bits(),
            0x0400_01A8..=0x0400_01AF => 0,     // Command
            0x0400_01B0..=0x0400_01BF => 0,   // Encryption seeds

            0x0410_0010 | 0x0410_0012 => {    // Data out
                let lo = self.get_data_out();
                let hi = self.get_data_out();
                bytes::u16::make(hi, lo)
            },

            _ => panic!("trying to read {:X}", addr),
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x0400_01A0 => self.spi_control = GamecardControl::from_bits_truncate(bytes::u16::set_lo(self.spi_control.bits(), data)),
            0x0400_01A1 => self.spi_control = GamecardControl::from_bits_truncate(bytes::u16::set_hi(self.spi_control.bits(), data)),

            0x0400_01A8..=0x0400_01AF => {
                let idx = 0x0400_01AF - addr;
                self.command[idx as usize] = data;
            },

            0x0400_01B0..=0x0400_01B3 => {
                let idx = addr - 0x0400_01B0;
                self.seed_0[idx as usize] = data;
            },
            0x0400_01B8 => {
                self.seed_0[4] = data & 0x7F;
                //println!("Seed0: {:?}", self.seed_0);
            },
            0x0400_01B4..=0x0400_01B7 => {
                let idx = addr - 0x0400_01B4;
                self.seed_1[idx as usize] = data;
            },
            0x0400_01BA => {
                self.seed_1[4] = data & 0x7F;
                //println!("Seed1: {:?}", self.seed_1);
            },

            0x0410_0010 => {},   // Data in
            0x0410_0012 => {},   // Data in

            _ => panic!("writing with byte to {:X} in card", addr),
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_01A0 => {
                //println!("write SPI control: {:X}", data);
                self.spi_control = GamecardControl::from_bits_truncate(data);
            },
            0x0400_01A2 => self.spi.write(data as u8),
            0x0400_01A4 => self.write_rom_control_lo(data),
            0x0400_01A6 => self.write_rom_control_hi(data),

            0x0400_01A8 => {
                self.command[7] = bytes::u16::lo(data);
                self.command[6] = bytes::u16::hi(data);
            },
            0x0400_01AA => {
                self.command[5] = bytes::u16::lo(data);
                self.command[4] = bytes::u16::hi(data);
            },
            0x0400_01AC => {
                self.command[3] = bytes::u16::lo(data);
                self.command[2] = bytes::u16::hi(data);
            },
            0x0400_01AE => {
                self.command[1] = bytes::u16::lo(data);
                self.command[0] = bytes::u16::hi(data);
            },

            0x0400_01B0 => {
                self.seed_0[0] = bytes::u16::lo(data);
                self.seed_0[1] = bytes::u16::hi(data);
            },
            0x0400_01B2 => {
                self.seed_0[2] = bytes::u16::lo(data);
                self.seed_0[3] = bytes::u16::hi(data);
            },
            0x0400_01B8 => {
                self.seed_0[4] = bytes::u16::lo(data) & 0x7F;
                //println!("Seed0: {:?}", self.seed_0);
            },
            0x0400_01B4 => {
                self.seed_1[0] = bytes::u16::lo(data);
                self.seed_1[1] = bytes::u16::hi(data);
            },
            0x0400_01B6 => {
                self.seed_1[2] = bytes::u16::lo(data);
                self.seed_1[3] = bytes::u16::hi(data);
            },
            0x0400_01BA => {
                self.seed_1[4] = bytes::u16::lo(data) & 0x7F;
                //println!("Seed1: {:?}", self.seed_1);
            },

            0x0410_0010 => {},   // Data in
            0x0410_0012 => {},   // Data in

            _ => unreachable!(),
        }
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_01A0 => {
                let data = self.spi.read();
                if !self.spi_control.contains(GamecardControl::SPI_HOLD) {
                    self.spi.deselect();
                }
                bytes::u32::make(data as u16, self.spi_control.bits())
            },
            0x0400_01A4 => {
                bytes::u32::make(self.rom_control_hi.bits(), self.rom_control_lo.bits())
            },
            0x0400_01A8..=0x0400_01AF => 0,     // Command
            0x0400_01B0..=0x0400_01BF => 0,   // Encryption seeds
            0x0410_0010 => {    // Data out
                u32::from_le_bytes([
                    self.get_data_out(),
                    self.get_data_out(),
                    self.get_data_out(),
                    self.get_data_out()
                ])
            },
            _ => panic!("read word from card @ {:X}", addr)
        }
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_01A0 => {
                //println!("write SPI control: {:X}", bytes::u32::lo(data));
                self.spi_control = GamecardControl::from_bits_truncate(bytes::u32::lo(data));
                self.spi.write(bytes::u32::hi(data) as u8);
            },
            0x0400_01A4 => {
                self.write_rom_control_lo(bytes::u32::lo(data));
                self.write_rom_control_hi(bytes::u32::hi(data))
            },
            0x0400_01A8 => {
                let bytes = data.to_le_bytes();
                self.command[7] = bytes[0];
                self.command[6] = bytes[1];
                self.command[5] = bytes[2];
                self.command[4] = bytes[3];
            },
            0x0400_01B0 => {
                let data_bytes = data.to_le_bytes();
                self.seed_0[0] = data_bytes[0];
                self.seed_0[1] = data_bytes[1];
                self.seed_0[2] = data_bytes[2];
                self.seed_0[3] = data_bytes[3];
            },
            0x0400_01B4 => {
                let data_bytes = data.to_le_bytes();
                self.seed_1[0] = data_bytes[0];
                self.seed_1[1] = data_bytes[1];
                self.seed_1[2] = data_bytes[2];
                self.seed_1[3] = data_bytes[3];
            },
            0x0400_01AC => {
                let bytes = data.to_le_bytes();
                self.command[3] = bytes[0];
                self.command[2] = bytes[1];
                self.command[1] = bytes[2];
                self.command[0] = bytes[3];
            },
            _ => panic!("write word to card @ {:X}", addr)
        }
    }
}

/// How the input commands are encrypted.
#[derive(PartialEq)]
enum CommandEncryptMode {
    None,
    Key1,
    Key2
}

/// States for the card, initially set by sending a command.
/// 
/// These states relate to the data returned or read by the cart,
/// via port at 0x0410_0010
#[derive(Clone, Copy, Debug)]
enum DSCardDataState {
    Dummy,              // 9F, 3C, 3D(?), 
    Header,             // 00 + addr,
    ID,                 // 90
    Key2,               // 4
    Key1ID,             // 1
    SecureBlock,        // 2
    Key2Disable,        // 6
    EnterMain,          // A
    Key2Dummy,
    GetData,            // B7
    Key2ID              // B8
}

// Internal
impl DSCard {
    /// Data transfer is complete.
    fn transfer_complete(&mut self) {
        self.rom_control_hi.remove(RomControlHi::START_STAT | RomControlHi::DATA_STATUS);
        self.dma_ready = false; // do we need to set this to false?
        if self.spi_control.contains(GamecardControl::TRANSFER_IRQ) {
            //println!("trigger int");
            self.interrupt = true;
        }
    }

    /// Card data is ready to be loaded.
    fn card_ready(&mut self) {
        self.rom_control_hi.insert(RomControlHi::DATA_STATUS);
        self.dma_ready = true;
        if self.transfer_count == 0 {
            self.transfer_complete();
        }
    }

    fn write_rom_control_lo(&mut self, data: u16) {
        self.rom_control_lo = RomControlLo::from_bits_truncate(data);
        //println!("Set ROMCTRL lo: {:X}", data);
        if self.rom_control_lo.contains(RomControlLo::KEY2_APPLY) {
            self.apply_key2_seeds();
            self.rom_control_lo.remove(RomControlLo::KEY2_APPLY);
        }
    }

    fn write_rom_control_hi(&mut self, data: u16) {
        let data_status = self.rom_control_hi & RomControlHi::DATA_STATUS;
        self.rom_control_hi = RomControlHi::from_bits_truncate(data);
        //println!("Set ROMCTRL hi: {:X}", data);
        self.rom_control_hi.remove(RomControlHi::DATA_STATUS);
        self.rom_control_hi.insert(data_status);
        if self.rom_control_hi.contains(RomControlHi::START_STAT) {
            self.transfer_count = match (self.rom_control_hi & RomControlHi::BLOCK_SIZE).bits() >> 8 {
                0 => 0,
                7 => 4,
                n => 0x100 << n
            };
            self.do_command();
        }
    }

    fn do_command(&mut self) {
        use CommandEncryptMode::*;
        self.data_state = match self.cmd_encrypt_mode {
            None => self.unencrypted_command(),
            Key1 => self.key1_command(),
            Key2 => {
                if self.rom_control_hi.contains(RomControlHi::KEY2_COMMAND) {
                    self.key2_command()
                } else {
                    self.key1_command()
                }
            },
        };
        //println!("do command {:?} | block size: {:X}", self.data_state, self.transfer_count);
    }

    fn apply_key2_seeds(&mut self) {
        self.key2_0 = u64::from_le_bytes(self.seed_0).reverse_bits() >> 25;
        self.key2_1 = u64::from_le_bytes(self.seed_1).reverse_bits() >> 25;
        //println!("KEY2: {:X} | {:X}", self.key2_0, self.key2_1);
        self.transfer_complete();
    }

    fn unencrypted_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = u64::from_le_bytes(self.command);
        //println!("got command {:X}", command);
        self.transfer_cycles = 100;
        match command >> 56 {   // Command is MSB
            0x9F => Dummy,
            0x00 => {
                self.read_addr = (command >> 24) as u32;
                Header
            },
            0x90 => ID,
            0x3C => {
                self.cmd_encrypt_mode = CommandEncryptMode::Key1;
                Dummy
            },
            _ => panic!("unrecognised DS card command: {:X}", command)
        }
    }

    fn key1_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = dscrypto::key1::decrypt(u64::from_le_bytes(self.command), &self.key1_instr);
        //println!("got K1 command {:X} => {:X}", u64::from_le_bytes(self.command), command);
        let key2_dummy_count = (self.rom_control_lo & RomControlLo::KEY1_GAP1_LEN).bits();
        // TODO: check bit size.
        self.transfer_cycles = (key2_dummy_count as usize) * 5;
        match command >> 60 {
            0x4 => {
                Key2
            },
            0x1 => {
                Key1ID
            },
            0x2 => {
                let block = ((command >> 44) & 0xFFFF) as u32;
                let addr = block * 0x1000;
                self.load_block(addr);
                if self.secure_block != block {
                    self.read_addr = addr;
                    self.secure_block = block;
                }
                //println!("Load secure block {:X} : {:X}", self.read_addr, self.transfer_count);
                SecureBlock
            },
            0x6 => {
                self.cmd_encrypt_mode = CommandEncryptMode::None;
                Key2Disable
            },
            0xA => {
                self.cmd_encrypt_mode = CommandEncryptMode::Key2;
                EnterMain
            },
            _ => panic!("unrecognised DS card command: {:X} (key1: {:X})", command, u64::from_le_bytes(self.command))
        }
    }

    fn key2_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = u64::from_le_bytes(self.command);
        //println!("got K2 command {:X}", command);
        self.transfer_cycles = 100;
        match command >> 56 {
            0xB7 => {
                let addr = (command >> 24) as u32;
                let addr = if addr >= 0x8000 {
                    addr
                } else {
                    0x8000 + (addr & 0x1FF)
                };
                //println!("Read from {:X}", addr);
                self.load_block(addr);
                self.read_addr = addr;
                GetData
            },
            0xB8 => {
                Key2ID
            },
            _ => {
                //println!("unrecognised (key2) DS card command: {:X}", command);
                Key2Dummy
            }
        }
    }

    fn get_data_out(&mut self) -> u8 {
        use DSCardDataState::*;
        let data = match self.data_state {
            Dummy => 0xFF,
            Header => {
                let addr = self.read_addr;
                self.read_addr += 1;
                self.read_card_byte(addr)
            },
            ID | Key1ID | Key2ID => {
                let idx = 4 - self.transfer_count;
                self.rom_id[idx]
            },
            Key2 => {
                // TODO: calc keys
                self.transfer_count = 1;
                //println!("read key2");
                0xFF
            },
            SecureBlock => {
                let addr = self.read_addr;
                self.read_addr += 1;
                self.read_card_byte(addr)
            },
            Key2Disable => 0,
            Key2Dummy => 0,
            EnterMain => 0,
            GetData => {
                let addr = self.read_addr;
                self.read_addr += 1;
                self.read_card_byte(addr)
            }
        };
        if self.transfer_count > 0 {
            self.transfer_count -= 1;
            if self.transfer_count == 0 {
                self.transfer_complete();
                self.data_state = if self.cmd_encrypt_mode == CommandEncryptMode::Key2 {Key2Dummy} else {Dummy};
            }
        }
        data
    }

    /// Read a byte from the actual game card ROM.
    /// 
    /// Reads from the current loaded block.
    fn read_card_byte(&mut self, addr: u32) -> u8 {
        self.rom_buffer[(addr % ROM_BUFFER_SIZE) as usize]
    }

    /// Load a 16kB block into memory from card.
    fn load_block(&mut self, addr: u32) {
        let tag = addr / ROM_BUFFER_SIZE;
        if tag != self.buffer_tag {
            self.buffer_tag = tag;
            let seek_addr = (tag * ROM_BUFFER_SIZE) as u64;
            if let Some(rom_file) = self.rom_file.as_mut() {
                rom_file.seek(SeekFrom::Start(seek_addr)).unwrap();
                rom_file.read_exact(&mut self.rom_buffer).unwrap();
            }
            if tag == 1 {
                self.encrypt_secure_area();
            }
        }
    }

    /// When loading the secure area at boot, the first 2kB needs to be encrypted.
    /// 
    /// Many ROM dumps decrypt this area.
    fn encrypt_secure_area(&mut self) {
        const ENCRY_OBJ: u64 = 0x6A624F7972636E65;
        const DESTROYED: u64 = 0xE7FFDEFFE7FFDEFF;

        // Re-encrypt the first 2kB.
        let id = u64::from_le_bytes([
            self.rom_buffer[0],
            self.rom_buffer[1],
            self.rom_buffer[2],
            self.rom_buffer[3],
            self.rom_buffer[4],
            self.rom_buffer[5],
            self.rom_buffer[6],
            self.rom_buffer[7]
        ]);
        if id == DESTROYED {
            // Unencrypted.
            let obj_buf = u64::to_le_bytes(ENCRY_OBJ);
            for (n, b) in obj_buf.iter().enumerate() {
                self.rom_buffer[n] = *b;
            }
        }
        if id == DESTROYED || id == ENCRY_OBJ {
            //println!("Encrypting...");
            // Needs to be encrypted.
            for i in 0..256 {
                let addr_offset = i * 8;
                let decrypted_block = u64::from_le_bytes([
                    self.rom_buffer[addr_offset],
                    self.rom_buffer[addr_offset + 1],
                    self.rom_buffer[addr_offset + 2],
                    self.rom_buffer[addr_offset + 3],
                    self.rom_buffer[addr_offset + 4],
                    self.rom_buffer[addr_offset + 5],
                    self.rom_buffer[addr_offset + 6],
                    self.rom_buffer[addr_offset + 7]
                ]);
                let encrypted_block = if i == 0 {
                    let block = dscrypto::key1::encrypt(decrypted_block, &self.key1_secure);
                    dscrypto::key1::encrypt(block, &self.key1_instr)
                } else {
                    dscrypto::key1::encrypt(decrypted_block, &self.key1_secure)
                };
                for (n, b) in encrypted_block.to_le_bytes().iter().enumerate() {
                    self.rom_buffer[addr_offset + n] = *b;
                }
            }

            // TEST
            // do crc16.
            /*const CRC_VAL: [u16; 256] = [
                0x0000, 0xC0C1, 0xC181, 0x0140, 0xC301, 0x03C0, 0x0280, 0xC241,
                0xC601, 0x06C0, 0x0780, 0xC741, 0x0500, 0xC5C1, 0xC481, 0x0440,
                0xCC01, 0x0CC0, 0x0D80, 0xCD41, 0x0F00, 0xCFC1, 0xCE81, 0x0E40,
                0x0A00, 0xCAC1, 0xCB81, 0x0B40, 0xC901, 0x09C0, 0x0880, 0xC841,
                0xD801, 0x18C0, 0x1980, 0xD941, 0x1B00, 0xDBC1, 0xDA81, 0x1A40,
                0x1E00, 0xDEC1, 0xDF81, 0x1F40, 0xDD01, 0x1DC0, 0x1C80, 0xDC41,
                0x1400, 0xD4C1, 0xD581, 0x1540, 0xD701, 0x17C0, 0x1680, 0xD641,
                0xD201, 0x12C0, 0x1380, 0xD341, 0x1100, 0xD1C1, 0xD081, 0x1040,
                0xF001, 0x30C0, 0x3180, 0xF141, 0x3300, 0xF3C1, 0xF281, 0x3240,
                0x3600, 0xF6C1, 0xF781, 0x3740, 0xF501, 0x35C0, 0x3480, 0xF441,
                0x3C00, 0xFCC1, 0xFD81, 0x3D40, 0xFF01, 0x3FC0, 0x3E80, 0xFE41,
                0xFA01, 0x3AC0, 0x3B80, 0xFB41, 0x3900, 0xF9C1, 0xF881, 0x3840,
                0x2800, 0xE8C1, 0xE981, 0x2940, 0xEB01, 0x2BC0, 0x2A80, 0xEA41,
                0xEE01, 0x2EC0, 0x2F80, 0xEF41, 0x2D00, 0xEDC1, 0xEC81, 0x2C40,
                0xE401, 0x24C0, 0x2580, 0xE541, 0x2700, 0xE7C1, 0xE681, 0x2640,
                0x2200, 0xE2C1, 0xE381, 0x2340, 0xE101, 0x21C0, 0x2080, 0xE041,
                0xA001, 0x60C0, 0x6180, 0xA141, 0x6300, 0xA3C1, 0xA281, 0x6240,
                0x6600, 0xA6C1, 0xA781, 0x6740, 0xA501, 0x65C0, 0x6480, 0xA441,
                0x6C00, 0xACC1, 0xAD81, 0x6D40, 0xAF01, 0x6FC0, 0x6E80, 0xAE41,
                0xAA01, 0x6AC0, 0x6B80, 0xAB41, 0x6900, 0xA9C1, 0xA881, 0x6840,
                0x7800, 0xB8C1, 0xB981, 0x7940, 0xBB01, 0x7BC0, 0x7A80, 0xBA41,
                0xBE01, 0x7EC0, 0x7F80, 0xBF41, 0x7D00, 0xBDC1, 0xBC81, 0x7C40,
                0xB401, 0x74C0, 0x7580, 0xB541, 0x7700, 0xB7C1, 0xB681, 0x7640,
                0x7200, 0xB2C1, 0xB381, 0x7340, 0xB101, 0x71C0, 0x7080, 0xB041,
                0x5000, 0x90C1, 0x9181, 0x5140, 0x9301, 0x53C0, 0x5280, 0x9241,
                0x9601, 0x56C0, 0x5780, 0x9741, 0x5500, 0x95C1, 0x9481, 0x5440,
                0x9C01, 0x5CC0, 0x5D80, 0x9D41, 0x5F00, 0x9FC1, 0x9E81, 0x5E40,
                0x5A00, 0x9AC1, 0x9B81, 0x5B40, 0x9901, 0x59C0, 0x5880, 0x9841,
                0x8801, 0x48C0, 0x4980, 0x8941, 0x4B00, 0x8BC1, 0x8A81, 0x4A40,
                0x4E00, 0x8EC1, 0x8F81, 0x4F40, 0x8D01, 0x4DC0, 0x4C80, 0x8C41,
                0x4400, 0x84C1, 0x8581, 0x4540, 0x8701, 0x47C0, 0x4680, 0x8641,
                0x8201, 0x42C0, 0x4380, 0x8341, 0x4100, 0x81C1, 0x8081, 0x4040
            ];
            let mut crc = 0xFFFF_u16;
            for i in 0x0..0x4000 {
                let data = self.rom_buffer[i] as u16;
                let idx = (crc ^ data) & 0xFF;
                crc = (crc >> 8) ^ CRC_VAL[idx as usize];
            }
            println!("got crc: ${:X}", crc);*/
        }
    }
}
