
use parking_lot::{Mutex, MutexGuard};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering}
};
use crate::common::mem::wram::WRAM;
use crate::utils::bits::u32;

const BANK_SIZE: usize = 16 * 1024;
const BANK_MASK: u32 = (BANK_SIZE as u32) - 1;

/// Reading and writing of shared RAM.
pub trait SharedRAM {
    fn get_bank(&self, addr: u32) -> Option<MutexGuard<WRAM>>;

    fn read_byte(&mut self, addr: u32) -> u8 {
        self.get_bank(addr).map(|bank| {
            bank.read_byte(addr & BANK_MASK)
        }).unwrap_or_default()
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        self.get_bank(addr).map(|mut bank| {
            bank.write_byte(addr & BANK_MASK, data);
        });
    }

    fn read_halfword(&mut self, addr: u32) -> u16 {
        self.get_bank(addr).map(|bank| {
            bank.read_halfword(addr & BANK_MASK)
        }).unwrap_or_default()
    }

    fn write_halfword(&mut self, addr: u32, data: u16) {
        self.get_bank(addr).map(|mut bank| {
            bank.write_halfword(addr & BANK_MASK, data);
        });
    }

    fn read_word(&mut self, addr: u32) -> u32 {
        self.get_bank(addr).map(|bank| {
            bank.read_word(addr & BANK_MASK)
        }).unwrap_or_default()
    }

    fn write_word(&mut self, addr: u32, data: u32) {
        self.get_bank(addr).map(|mut bank| {
            bank.write_word(addr & BANK_MASK, data);
        });
    }
}

/// Shared 16K+16K of WRAM.
/// Found on ARM9 bus.
pub struct ARM9SharedRAM {
    bank_control:   u8,
    bank_status:    Arc<AtomicU8>,
    
    lo_bank:        Arc<Mutex<WRAM>>,
    hi_bank:        Arc<Mutex<WRAM>>,
}

impl ARM9SharedRAM {
    pub fn new() -> (Self, ARM7SharedRAM) {
        let lo_bank = Arc::new(Mutex::new(WRAM::new(BANK_SIZE)));
        let hi_bank = Arc::new(Mutex::new(WRAM::new(BANK_SIZE)));
        let bank_status = Arc::new(AtomicU8::new(3));
        (Self {
            bank_control:   0,
            bank_status:    bank_status.clone(),
            lo_bank:        lo_bank.clone(),
            hi_bank:        hi_bank.clone()
        }, ARM7SharedRAM {
            bank_status:    bank_status,
            lo_bank:        lo_bank,
            hi_bank:        hi_bank
        })
    }

    pub fn set_bank_control(&mut self, data: u8) {
        self.bank_control = data;
        self.bank_status.store(data, Ordering::Release);
    }

    pub fn get_bank_control(&self) -> u8 {
        self.bank_control
    }
}

impl SharedRAM for ARM9SharedRAM {
    fn get_bank(&self, addr: u32) -> Option<MutexGuard<WRAM>> {
        match self.bank_control {
            0 => if u32::test_bit(addr, 14) {   // 0x4000
                Some(self.hi_bank.lock())
            } else {
                Some(self.lo_bank.lock())
            },
            1 => Some(self.hi_bank.lock()),
            2 => Some(self.lo_bank.lock()),
            _ => None,  // unmapped
        }
    }
}

/// Shared 16K+16K of WRAM.
/// Found on ARM7 bus.
pub struct ARM7SharedRAM {
    bank_status:    Arc<AtomicU8>,

    lo_bank:        Arc<Mutex<WRAM>>,
    hi_bank:        Arc<Mutex<WRAM>>,
}

impl ARM7SharedRAM {
    pub fn get_bank_status(&self) -> u8 {
        self.bank_status.load(Ordering::Acquire)
    }
}

impl SharedRAM for ARM7SharedRAM {
    fn get_bank(&self, addr: u32) -> Option<MutexGuard<WRAM>> {
        match self.bank_status.load(Ordering::Acquire) {
            1 => Some(self.lo_bank.lock()),
            2 => Some(self.hi_bank.lock()),
            3 => if u32::test_bit(addr, 14) {   // 0x4000
                Some(self.hi_bank.lock())
            } else {
                Some(self.lo_bank.lock())
            },
            _ => None,  // unmapped
        }
    }
}
