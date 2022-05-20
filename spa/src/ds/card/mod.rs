mod header;

use bitflags::bitflags;

use std::{
    io::{
        Result,
        Read,
        Seek,
        SeekFrom
    },
    fs::File,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering}
    }
};

use crate::utils::{
    bits::u16,
    bytes,
    meminterface::{MemInterface16, MemInterface32}
};
use crate::ds::interrupt::Interrupts;
pub use header::CardHeader;

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
        const KEY2_DATA     = u16::bit(13);
        const KEY1_GAP1_LEN = u16::bits(0, 12);
    }
}

/// We read 1kB at a time from disk.
const ROM_BUFFER_SIZE: u32 = 1024;

const ROM_ID: [u8; 4] = [0xC2, 0x1F, 0x00, 0x00];

/// DS Card attached to IO ports.
pub struct DSCardIO {
    card:       Arc<Mutex<DSCard>>,
    interrupt:  Arc<AtomicBool>,
}

impl DSCardIO {
    pub fn new(rom_path: &Path, key1: Vec<u32>) -> Result<(Self, Self)> {
        let interrupt_7 = Arc::new(AtomicBool::new(false));
        let interrupt_9 = Arc::new(AtomicBool::new(false));
        let card = Arc::new(Mutex::new(DSCard::new(rom_path, key1, interrupt_7.clone(), interrupt_9.clone())?));
        Ok((DSCardIO{
            card: card.clone(),
            interrupt: interrupt_9
        }, DSCardIO{
            card: card,
            interrupt: interrupt_7
        }))
    }

    /// To be called by only the ARM7 processor.
    pub fn clock(&mut self, cycles: usize) {
        //self.card.lock().unwrap().clock(cycles);
    }

    pub fn get_interrupt(&self) -> Interrupts {
        if self.interrupt.swap(false, Ordering::Acquire) {
            Interrupts::CARD_COMPLETE
        } else {
            Interrupts::empty()
        }
    }

    pub fn get_header(&self) -> CardHeader {
        let mut data = vec![0; 0x200];
        self.card.lock().unwrap().load_data(0, &mut data);
        CardHeader::new(data)
    }

    pub fn load_data(&self, from_addr: u32, into_buffer: &mut [u8]) {
        self.card.lock().unwrap().load_data(from_addr, into_buffer);
    }

    /// Fast boot mode setup.
    pub fn fast_boot(&mut self) {
        self.card.lock().unwrap().fast_boot();
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

    spi_control: GamecardControl,
    rom_control_lo: RomControlLo,
    rom_control_hi: RomControlHi,

    key1: Vec<u32>, // 0x1048 byte key

    command: [u8; 8],
    seed_0: [u8; 8],
    seed_1: [u8; 8],

    key2_0: u64,
    key2_1: u64,

    transfer_count: usize,
    /// Key-1 encrypted commands return 0x910 bytes of dummy data
    key2_dummy_count: usize,
    cmd_encrypt_mode: CommandEncryptMode,
    data_state: DSCardDataState,

    interrupt_7: Arc<AtomicBool>,
    interrupt_9: Arc<AtomicBool>
}

impl DSCard {
    fn new(rom_path: &Path, key1: Vec<u32>, interrupt_7: Arc<AtomicBool>, interrupt_9: Arc<AtomicBool>) -> Result<Self> {
        let mut rom_file = File::open(rom_path)?;
        let mut buffer = vec![0; ROM_BUFFER_SIZE as usize];

        rom_file.seek(SeekFrom::Start(0))?;
        rom_file.read_exact(&mut buffer)?;

        // Game ID code.
        let id_code = u32::from_le_bytes([buffer[0xC], buffer[0xD], buffer[0xE], buffer[0xF]]);
        let key1_level2 = dscrypto::key1::init(id_code, &key1, 2, 2);

        Ok(Self {
            rom_file:   rom_file,
            rom_buffer: buffer,
            buffer_tag: 0,

            spi_control: GamecardControl::default(),
            rom_control_lo: RomControlLo::default(),
            rom_control_hi: RomControlHi::default(),

            key1: key1_level2,

            command: [0; 8],
            seed_0: [0xE8, 0xE0, 0x6D, 0xC5, 0x58, 0, 0, 0],
            seed_1: [0x05, 0x9B, 0x9B, 0x87, 0x5C, 0, 0, 0],

            key2_0: 0,
            key2_1: 0,

            transfer_count: 0,
            key2_dummy_count: 0,
            cmd_encrypt_mode: CommandEncryptMode::None,
            data_state: DSCardDataState::Dummy,

            interrupt_7: interrupt_7,
            interrupt_9: interrupt_9,
        })
    }

    fn clock(&mut self, cycles: usize) {
        if self.key2_dummy_count > 0 {
            if self.key2_dummy_count > cycles {
                // TODO: 1 per cycle?
                self.key2_dummy_count -= cycles;
            } else {
                self.key2_dummy_count = 0;
                self.rom_control_hi.insert(RomControlHi::DATA_STATUS);
                self.rom_control_hi.remove(RomControlHi::START_STAT);
                //self.trigger_interrupt();
            }
        }
    }

    fn load_data(&mut self, from_addr: u32, into_buffer: &mut [u8]) {
        self.rom_file.seek(SeekFrom::Start(from_addr as u64)).unwrap();
        self.rom_file.read(into_buffer).unwrap();
    }

    fn fast_boot(&mut self) {
        // TODO: load seed 0
        //self.apply_key2_seeds();
        self.cmd_encrypt_mode = CommandEncryptMode::Key2;
    }
}

impl MemInterface16 for DSCard {
    fn read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            // 0x0400_01A0
            0x0 => self.spi_control.bits(),   // AUXSPICNT
            0x2 => 0,   // AUXSPIDATA
            0x4 => self.rom_control_lo.bits(),   // ROMCTRL
            0x6 => self.rom_control_hi.bits(), // ROMCTRL
            0x8..=0xF => 0,     // Command
            0x10..=0x1F => 0,   // Encryption seeds

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
            // 0x0400_01A0
            0x0 => self.spi_control = GamecardControl::from_bits_truncate(bytes::u16::set_lo(self.spi_control.bits(), data)),
            0x1 => self.spi_control = GamecardControl::from_bits_truncate(bytes::u16::set_hi(self.spi_control.bits(), data)),

            0x8..=0xF => {
                let idx = 0xF - addr;
                self.command[idx as usize] = data;
            },

            0x10..=0x13 => {
                let idx = addr - 0x10;
                self.seed_0[idx as usize] = data;
            },
            0x18 => {
                self.seed_0[4] = data & 0x7F;
                println!("Seed0: {:?}", self.seed_0);
            },
            0x14..=0x17 => {
                let idx = addr - 0x14;
                self.seed_1[idx as usize] = data;
            },
            0x1A => {
                self.seed_1[4] = data & 0x7F;
                println!("Seed1: {:?}", self.seed_1);
            },

            0x0410_0010 => {},   // Data in
            0x0410_0012 => {},   // Data in

            _ => panic!("writing with byte to {:X} in card", addr),
        }
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            // 0x0400_01A0
            0x0 => self.spi_control = GamecardControl::from_bits_truncate(data),   // AUXSPICNT
            0x2 => {},   // AUXSPIDATA
            0x4 => self.write_rom_control_lo(data),   // ROMCTRL
            0x6 => self.write_rom_control_hi(data),   // ROMCTRL

            0x8 => {
                self.command[7] = bytes::u16::lo(data);
                self.command[6] = bytes::u16::hi(data);
            },
            0xA => {
                self.command[5] = bytes::u16::lo(data);
                self.command[4] = bytes::u16::hi(data);
            },
            0xC => {
                self.command[3] = bytes::u16::lo(data);
                self.command[2] = bytes::u16::hi(data);
            },
            0xE => {
                self.command[1] = bytes::u16::lo(data);
                self.command[0] = bytes::u16::hi(data);
            },

            0x10 => {
                self.seed_0[0] = bytes::u16::lo(data);
                self.seed_0[1] = bytes::u16::hi(data);
            },
            0x12 => {
                self.seed_0[2] = bytes::u16::lo(data);
                self.seed_0[3] = bytes::u16::hi(data);
            },
            0x18 => {
                self.seed_0[4] = bytes::u16::lo(data) & 0x7F;
                println!("Seed0: {:?}", self.seed_0);
            },
            0x14 => {
                self.seed_1[0] = bytes::u16::lo(data);
                self.seed_1[1] = bytes::u16::hi(data);
            },
            0x16 => {
                self.seed_1[2] = bytes::u16::lo(data);
                self.seed_1[3] = bytes::u16::hi(data);
            },
            0x1A => {
                self.seed_1[4] = bytes::u16::lo(data) & 0x7F;
                println!("Seed1: {:?}", self.seed_1);
            },

            0x0410_0010 => {},   // Data in
            0x0410_0012 => {},   // Data in

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
#[derive(Clone, Copy, Debug)]
enum DSCardDataState {
    Dummy,              // 9F, 3C, 3D(?), 
    Header(u32),        // 00 + addr
    ID,                 // 90
    Key2,               // 4
    Key1ID,             // 1
    SecureBlock(u32),   // 2
    Key2Disable(u32),   // 6
    EnterMain(u32),     // A
    GetData(u32),       // B7
    Key2ID              // B8
}

// Internal
impl DSCard {
    fn trigger_interrupt(&mut self) {
        if self.spi_control.contains(GamecardControl::TRANSFER_IRQ) {
            //println!("trigger int");
            self.interrupt_7.store(true, Ordering::Release);
            self.interrupt_9.store(true, Ordering::Release);
        }
    }

    fn write_rom_control_lo(&mut self, data: u16) {
        self.rom_control_lo = RomControlLo::from_bits_truncate(data);
        //println!("Set ROMCTRL lo: {:X}", data);
        if self.rom_control_lo.contains(RomControlLo::KEY2_APPLY) {
            self.apply_key2_seeds();
        }
    }

    fn write_rom_control_hi(&mut self, data: u16) {
        self.rom_control_hi = RomControlHi::from_bits_truncate(data);
        //println!("Set ROMCTRL hi: {:X}", data);
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
            Key2 => self.key2_command(),
        };
        //println!("do command {:?} | block size: {:X}", self.data_state, self.transfer_count);
    }

    fn apply_key2_seeds(&mut self) {
        self.key2_0 = u64::from_le_bytes(self.seed_0).reverse_bits() >> 25;
        self.key2_1 = u64::from_le_bytes(self.seed_1).reverse_bits() >> 25;
        println!("KEY2: {:X} | {:X}", self.key2_0, self.key2_1);
        self.trigger_interrupt();
    }

    fn unencrypted_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = u64::from_le_bytes(self.command);
        //println!("got command {:X}", command);
        self.rom_control_hi.insert(RomControlHi::DATA_STATUS);
        match command >> 56 {   // Command is MSB
            0x9F => Dummy,
            0x00 => {
                let addr = (command >> 24) as u32;
                Header(addr)
            },
            0x90 => ID,
            0x3C => {
                self.cmd_encrypt_mode = CommandEncryptMode::Key1;
                self.rom_control_hi.remove(RomControlHi::START_STAT | RomControlHi::DATA_STATUS);
                //self.trigger_interrupt();
                Dummy
            },
            _ => panic!("unrecognised DS card command: {:X}", command)
        }
    }

    fn key1_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = dscrypto::key1::decrypt(u64::from_le_bytes(self.command), &self.key1);
        println!("got command {:X} => {:X}", u64::from_le_bytes(self.command), command);
        self.key2_dummy_count = 0x910;
        match command >> 60 {
            0x4 => {
                //self.rom_control_hi.remove(RomControlHi::START_STAT | RomControlHi::DATA_STATUS);
                Key2
            },
            0x1 => {
                //self.trigger_interrupt();
                Key1ID
            },
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
                //self.trigger_interrupt();
                EnterMain(0)
            }
            _ => panic!("unrecognised DS card command: {:X} (key1: {:X})", command, u64::from_le_bytes(self.command))
        }
    }

    fn key2_command(&mut self) -> DSCardDataState {
        use DSCardDataState::*;
        let command = u64::from_le_bytes(self.command);
        //println!("got command {:X}", command);
        self.rom_control_hi.insert(RomControlHi::DATA_STATUS);
        match command >> 56 {
            0xB7 => {
                let addr = (command >> 24) as u32;
                if addr >= 0x8000 {
                    GetData(addr)
                } else {
                    GetData(0x8000 + (addr & 0x1FF))
                }
            },
            0xB8 => Key2ID,
            _ => panic!("unrecognised (key2) DS card command: {:X}", command)
        }
    }

    fn get_data_out(&mut self) -> u8 {
        use DSCardDataState::*;
        /*if self.key2_dummy_count > 0 {
            self.key2_dummy_count -= 1;
            // TODO: encode?
            println!("return key2 dummy {:X}", self.key2_dummy_count);
            return 0;
        }*/
        let data = match self.data_state {
            Dummy => 0xFF,
            Header(addr) => if addr >= 0x200 {
                0xFF
            } else {
                self.data_state = Header(addr + 1);
                self.read_card_byte(addr)
            },
            ID => {
                let idx = 4 - self.transfer_count;
                ROM_ID[idx]
            },
            Key1ID | Key2ID => {
                let idx = 4 - self.transfer_count;
                //self.encrypt_byte_key2(ROM_ID[idx])
                ROM_ID[idx]
            },
            Key2 => {
                // TODO: calc keys
                //self.transfer_count = 1;
                0xFF
            },
            SecureBlock(addr) => {
                let data = self.read_card_byte(addr);
                self.data_state = SecureBlock(addr + 1);
                data
                //self.encrypt_byte_key2(data)
            },
            Key2Disable(_) => 0,
            EnterMain(_) => 0,
            GetData(addr) => {
                let data = self.read_card_byte(addr);
                self.data_state = GetData(addr + 1);
                data
                //self.encrypt_byte_key2(data)
            }
        };
        //println!("read data: {:X}", data);
        self.transfer_count -= 1;
        if self.transfer_count == 0 {
            self.data_state = Dummy;
            self.rom_control_hi.remove(RomControlHi::START_STAT | RomControlHi::DATA_STATUS);
            //self.trigger_interrupt();
        }
        data
    }

    /// Read a byte from the actual game card ROM.
    fn read_card_byte(&mut self, addr: u32) -> u8 {
        let tag = addr / ROM_BUFFER_SIZE;
        if tag != self.buffer_tag {
            self.buffer_tag = tag;
            let seek_addr = (tag * ROM_BUFFER_SIZE) as u64;
            self.rom_file.seek(SeekFrom::Start(seek_addr)).unwrap();
            self.rom_file.read_exact(&mut self.rom_buffer).unwrap();
        }
        self.rom_buffer[(addr % ROM_BUFFER_SIZE) as usize]
    }

    #[inline]
    fn encrypt_byte_key2(&mut self, data_in: u8) -> u8 {
        let (data, key2_0, key2_1) = dscrypto::key_2_encrypt(data_in, self.key2_0, self.key2_1);
        self.key2_0 = key2_0;
        self.key2_1 = key2_1;
        data
    }
}
