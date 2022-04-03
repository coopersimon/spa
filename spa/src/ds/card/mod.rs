mod crypto;

use std::{
    io::{
        Result,
        Read,
        Seek,
        SeekFrom
    },
    fs::File,
    path::Path,
    sync::{Arc, Mutex}
};

use crate::utils::{
    bytes::u16,
    meminterface::{MemInterface16, MemInterface32}
};

/// We read 1kB at a time from disk.
const ROM_BUFFER_SIZE: u32 = 1024;

/// DS Card attached to IO ports.
#[derive(Clone)]
pub struct DSCardIO {
    card:   Arc<Mutex<DSCard>>
}

impl DSCardIO {
    pub fn new(rom_path: &Path) -> Result<Self> {
        let card = Arc::new(Mutex::new(DSCard::new(rom_path)?));
        Ok(DSCardIO{
            card
        })
    }
}

impl MemInterface32 for DSCardIO {
    fn read_byte(&mut self, addr: u32) -> u8 {
        self.card.lock().unwrap().read_byte(addr)
    }
    fn write_byte(&mut self, addr: u32, data: u8) {
        self.card.lock().unwrap().write_byte(addr, data);
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        self.card.lock().unwrap().read_halfword(addr)
    }
    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.card.lock().unwrap().write_halfword(addr, data);
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        self.card.lock().unwrap().read_word(addr)
    }
    fn write_word(&mut self, addr: u32, data: u32) {
        self.card.lock().unwrap().write_word(addr, data);
    }
}

/// The DS card ROM.
struct DSCard {
    rom_file:   File,
    rom_buffer: Vec<u8>,
    buffer_tag: u32,

    command: [u8; 8],
    seed_0: [u8; 8],
    seed_1: [u8; 8],

    key2_0: u64,
    key2_1: u64,

    /// Key-1 encrypted commands return 0x910 bytes of dummy data
    key2_dummy_count: usize,
    cmd_encrypt_mode: CommandEncryptMode,
    data_state: DSCardDataState,
}

impl DSCard {
    fn new(rom_path: &Path) -> Result<Self> {
        let mut rom_file = File::open(rom_path)?;
        let mut buffer = vec![0; ROM_BUFFER_SIZE as usize];

        rom_file.seek(SeekFrom::Start(0))?;
        rom_file.read(&mut buffer)?;

        Ok(Self {
            rom_file:   rom_file,
            rom_buffer: buffer,
            buffer_tag: 0,

            command: [0; 8],
            seed_0: [0xE8, 0xE0, 0x6D, 0xC5, 0x58, 0, 0, 0],
            seed_1: [0x05, 0x9B, 0x9B, 0x87, 0x5C, 0, 0, 0],

            key2_0: 0,
            key2_1: 0,

            key2_dummy_count: 0,
            cmd_encrypt_mode: CommandEncryptMode::None,
            data_state: DSCardDataState::Dummy,
        })
    }
}

impl MemInterface16 for DSCard {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            // 0x0400_01A0
            0x0 => 0,   // AUXSPICNT
            0x2 => 0,   // AUXSPIDATA
            0x4 => 0,   // ROMCTRL
            0x6 => 0,   // ROMCTRL
            0x8..=0xF => 0,     // Command
            0x10..=0x1B => 0,   // Encryption seeds

            // 0x0410_0010
            0x000F_FE70 | 0x000F_FE72 => {    // Data out
                let lo = self.get_data_out();
                let hi = self.get_data_out();
                u16::make(hi, lo)
            },

            _ => unreachable!(),
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            // 0x0400_01A0
            0x0 => {},   // AUXSPICNT
            0x2 => {},   // AUXSPIDATA
            0x4 => {},   // ROMCTRL
            0x6 => {},   // ROMCTRL

            0x8 => {
                self.command[7] = u16::lo(data);
                self.command[6] = u16::hi(data);
            },
            0xA => {
                self.command[5] = u16::lo(data);
                self.command[4] = u16::hi(data);
            },
            0xC => {
                self.command[3] = u16::lo(data);
                self.command[2] = u16::hi(data);
            },
            0xE => {
                self.command[1] = u16::lo(data);
                self.command[0] = u16::hi(data);
                self.do_command();
            },

            0x10 => {
                self.seed_0[0] = u16::lo(data);
                self.seed_0[1] = u16::hi(data);
            },
            0x12 => {
                self.seed_0[2] = u16::lo(data);
                self.seed_0[3] = u16::hi(data);
            },
            0x18 => {
                self.seed_0[4] = u16::lo(data) & 0x7F;
            },
            0x14 => {
                self.seed_1[0] = u16::lo(data);
                self.seed_1[1] = u16::hi(data);
            },
            0x16 => {
                self.seed_1[2] = u16::lo(data);
                self.seed_1[3] = u16::hi(data);
            },
            0x1A => {
                self.seed_1[4] = u16::lo(data) & 0x7F;
            },

            // 0x0410_0010
            0x000F_FE70 => {},   // Data in
            0x000F_FE72 => {},   // Data in

            _ => unreachable!(),
        }
    }
}

/// How the input commands are encrypted.
enum CommandEncryptMode {
    None,
    Key1,
    Key2
}

/// States for the card, initially set by sending a command.
/// 
/// These states relate to the data returned or read by the cart,
/// via port at 0x0410_0010
enum DSCardDataState {
    Dummy,              // 9F, 3C, 3D(?), 
    Header(u32),        // 00 + addr
    ID(u32),            // 90
    Key2,               // 4
    Key1ID(u32),        // 1
    SecureBlock(u32),   // 2
    Key2Disable(u32),   // 6
    EnterMain(u32),     // A
    GetData(u32),       // B7
    Key2ID(u32)         // B8
}

// Internal
impl DSCard {
    fn do_command(&mut self) {
        use CommandEncryptMode::*;
        self.data_state = match self.cmd_encrypt_mode {
            None => self.unencrypted_command(),
            Key1 => self.key1_command(),
            Key2 => self.key2_command(),
        };
    }

    fn unencrypted_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = u64::from_le_bytes(self.command);
        match command >> 56 {   // Command is MSB
            0x9F => Dummy,
            0x00 => {
                let addr = (command >> 24) as u32;
                Header(addr)
            },
            0x90 => ID(0),
            0x3C => {
                self.cmd_encrypt_mode = CommandEncryptMode::Key1;
                Dummy
            },
            _ => panic!("unrecognised DS card command: {:X}", command)
        }
    }

    fn key1_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        // TODO: get key buffer
        let command = crypto::key_1_decrypt(u64::from_le_bytes(self.command), &[]);
        self.key2_dummy_count = 0x910;
        match command >> 60 {
            0x4 => Key2,
            0x1 => Key1ID(0),
            0x2 => {
                let block = ((command >> 44) & 0xFFFF) as u32;
                let addr = block * 0x1000;
                SecureBlock(addr)
            },
            0x6 => {
                self.cmd_encrypt_mode = CommandEncryptMode::None;
                Key2Disable(0)
            },
            0xA => {
                self.cmd_encrypt_mode = CommandEncryptMode::Key2;
                EnterMain(0)
            }
            _ => panic!("unrecognised DS card command: {:X} (key1: {:X})", command, u64::from_le_bytes(self.command))
        }
    }

    fn key2_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let mut out = [0_u8; 8];
        for i in 0..8 {
            out[7-i] = self.encrypt_byte_key2(self.command[7-i]);
        }
        let command = u64::from_le_bytes(out);
        match command >> 56 {
            0xB7 => {
                let addr = (command >> 24) as u32;
                if addr >= 0x8000 {
                    GetData(addr)
                } else {
                    GetData(0x8000 + (addr & 0x1FF))
                }
            },
            0xB8 => Key2ID(0),
            _ => panic!("unrecognised DS card command: {:X} (key2: {:X})", command, u64::from_le_bytes(self.command))
        }
    }

    fn get_data_out(&mut self) -> u8 {
        use DSCardDataState::*;
        if self.key2_dummy_count > 0 {
            self.key2_dummy_count -= 1;
            // TODO: encode?
            return 0;
        }
        let (data, new_state) = match self.data_state {
            Dummy => (0xFF, Dummy),
            Header(addr) => if addr >= 0x200 {
                (0xFF, Dummy)
            } else {
                (self.read_card_byte(addr), Header(addr + 1))
            },
            ID(n) => (0, ID(n + 1)),
            Key1ID(n) => (0, Key1ID(n + 1)),
            Key2ID(n) => (0, Key2ID(n + 1)),
            Key2 => {
                // TODO: calc keys

                (0xFF, Dummy)
            },
            SecureBlock(addr) => {
                let data = self.read_card_byte(addr);
                (self.encrypt_byte_key2(data), SecureBlock(addr + 1))
            },
            Key2Disable(n) => (0, Key2Disable(n + 1)),
            EnterMain(n) => (0, EnterMain(n + 1)),
            GetData(addr) => {
                let data = self.read_card_byte(addr);
                (self.encrypt_byte_key2(data), GetData(addr + 1))
            }
        };
        self.data_state = new_state;
        data
    }

    /// Read a byte from the actual game card ROM.
    fn read_card_byte(&mut self, addr: u32) -> u8 {
        let tag = addr / ROM_BUFFER_SIZE;
        if tag != self.buffer_tag {
            self.buffer_tag = tag;
            let seek_addr = (tag * ROM_BUFFER_SIZE) as u64;
            self.rom_file.seek(SeekFrom::Start(seek_addr)).unwrap();
            self.rom_file.read(&mut self.rom_buffer).unwrap();
        }
        self.rom_buffer[(addr % ROM_BUFFER_SIZE) as usize]
    }

    #[inline]
    fn encrypt_byte_key2(&mut self, data_in: u8) -> u8 {
        let (data, key2_0, key2_1) = crypto::key_2_encrypt(data_in, self.key2_0, self.key2_1);
        self.key2_0 = key2_0;
        self.key2_1 = key2_1;
        data
    }
}
