// SPI for interfacing with save RAM.

use bitflags::bitflags;
use crate::utils::bits::u8;

bitflags!{
    #[derive(Default)]
    struct Status: u8 {
        const WRITE_PROTECT = u8::bits(2, 3);
        const WRITE_ENABLE  = u8::bit(1);
        const WRITE_ACTIVE  = u8::bit(0);
    }
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

pub struct SPI {
    // TODO: also support persistent mem
    ram: Vec<u8>,

    status:     Status,

    state:      State,
    can_read:   bool,
}

impl SPI {
    pub fn new() -> Self {
        Self {
            ram: vec![0; 512 * 1024],
        
            status:     Status::default(),
        
            state:      State::Idle,
            can_read:   false,
        }
    }

    pub fn deselect(&mut self) {
        self.state = State::Idle;
        self.can_read = false;
    }

    pub fn read(&mut self) -> u8 {
        use State::*;
        let data = match self.state {
            ReadStatus if self.can_read => {
                self.state = Idle;
                self.status.bits()
            },
            Read(addr) if self.can_read => {
                let data = self.ram[addr as usize];
                self.state = Read(addr + 1);
                data
            },
            _ => 0,
        };
        //println!("READ {:X}", data);
        data
    }

    pub fn write(&mut self, data: u8) {
        //println!("WRITE {:X}", data);
        use State::*;
        match self.state {
            Idle => match data {
                0x06 => self.status.insert(Status::WRITE_ENABLE),
                0x04 => self.status.remove(Status::WRITE_ENABLE),
                0x05 => self.state = ReadStatus,
                0x01 => self.state = WriteStatus,
                0x03 => self.state = PrepRead { byte: 0, addr: 0 },
                0x02 => self.state = PrepWrite { byte: 0, addr: 0 },
                0x0A => self.state = PrepWrite { byte: 0, addr: 0 },
                0x0 => {},
                _ => panic!("unsupported save ram op {:X}", data),
            },
            PrepRead{byte: 1, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = Read(addr);
            },
            PrepRead{byte, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = PrepRead{byte: byte + 1, addr};
            },
            PrepWrite{byte: 1, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = Write(addr);
            },
            PrepWrite{byte, addr} => {
                let addr = (addr << 8) | (data as u32);
                self.state = PrepWrite{byte: byte + 1, addr};
            },
            WriteStatus => {
                self.status = Status::from_bits_truncate(data);
                self.state = Idle;
            },
            Write(addr) => {
                self.ram[addr as usize] = data;
                self.state = Write(addr + 1);
            },
            _ => self.can_read = true,
        }
    }
}
