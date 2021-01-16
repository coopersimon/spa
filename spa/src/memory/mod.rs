/// Memory bus
mod wram;

use arm::{
    Mem32, Clockable
};
use crate::timers::Timers;

/// Game Boy Advance memory bus
pub struct MemoryBus {
    wram:       wram::WRAM,
    fast_wram:  wram::WRAM,

    timers:     Timers,
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            wram:       wram::WRAM::new(256 * 1024),
            fast_wram:  wram::WRAM::new(32 * 1024),

            timers:     Timers::new(),
        }
    }
}

impl Mem32 for MemoryBus {
    type Addr = u32;

    fn load_byte(&mut self, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (0, 0),    // BIOS
            0x0200_0000..=0x0203_FFFF => (self.wram.read_byte(addr), 3),        // WRAM
            0x0300_0000..=0x0300_7FFF => (self.fast_wram.read_byte(addr), 1),   // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_byte(addr), 1),          // I/O

            0x0500_0000..=0x0500_03FF => (0, 0),    // Palette RAM
            0x0600_0000..=0x0601_7FFF => (0, 0),    // VRAM
            0x0700_0000..=0x0700_03FF => (0, 0),    // OAM

            0x0800_0000..=0x0FFF_FFFF => (0, 0),    // Cart

            _ => (0, 1) // Unused
        }
    }
    fn store_byte(&mut self, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            0x0200_0000..=0x0203_FFFF => {  // WRAM
                self.wram.write_byte(addr, data);
                3
            },
            0x0300_0000..=0x0300_7FFF => {  // FAST WRAM
                self.fast_wram.write_byte(addr, data);
                1
            },
            0x0400_0000..=0x0400_03FE => {  // I/O
                self.io_write_byte(addr, data);
                1
            },

            0x0500_0000..=0x0500_03FF => 1,    // Palette RAM
            0x0600_0000..=0x0601_7FFF => 1,    // VRAM
            0x0700_0000..=0x0700_03FF => 1,    // OAM

            0x0800_0000..=0x0FFF_FFFF => 1,    // Cart

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (0, 0),    // BIOS
            0x0200_0000..=0x0203_FFFF => (self.wram.read_halfword(addr), 3),        // WRAM
            0x0300_0000..=0x0300_7FFF => (self.fast_wram.read_halfword(addr), 1),   // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_halfword(addr), 1),          // I/O

            0x0500_0000..=0x0500_03FF => (0, 0),    // Palette RAM
            0x0600_0000..=0x0601_7FFF => (0, 0),    // VRAM
            0x0700_0000..=0x0700_03FF => (0, 0),    // OAM

            0x0800_0000..=0x0FFF_FFFF => (0, 0),    // Cart

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            0x0200_0000..=0x0203_FFFF => {  // WRAM
                self.wram.write_halfword(addr, data);
                3
            },
            0x0300_0000..=0x0300_7FFF => {  // FAST WRAM
                self.fast_wram.write_halfword(addr, data);
                1
            },
            0x0400_0000..=0x0400_03FE => {  // I/O
                self.io_write_halfword(addr, data);
                1
            },

            0x0500_0000..=0x0500_03FF => 1,    // Palette RAM
            0x0600_0000..=0x0601_7FFF => 1,    // VRAM
            0x0700_0000..=0x0700_03FF => 1,    // OAM

            0x0800_0000..=0x0FFF_FFFF => 1,    // Cart

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (0, 0),    // BIOS
            0x0200_0000..=0x0203_FFFF => (self.wram.read_word(addr), 6),        // WRAM
            0x0300_0000..=0x0300_7FFF => (self.fast_wram.read_word(addr), 1),   // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_word(addr), 1),          // I/O

            0x0500_0000..=0x0500_03FF => (0, 0),    // Palette RAM
            0x0600_0000..=0x0601_7FFF => (0, 0),    // VRAM
            0x0700_0000..=0x0700_03FF => (0, 0),    // OAM

            0x0800_0000..=0x0FFF_FFFF => (0, 0),    // Cart

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0000_0000..=0x0000_3FFF => 1, // BIOS
            0x0200_0000..=0x0203_FFFF => {  // WRAM
                self.wram.write_word(addr, data);
                6
            },
            0x0300_0000..=0x0300_7FFF => {  // FAST WRAM
                self.fast_wram.write_word(addr, data);
                1
            },
            0x0400_0000..=0x0400_03FE => {  // I/O
                self.io_write_word(addr, data);
                1
            },

            0x0500_0000..=0x0500_03FF => 1,    // Palette RAM
            0x0600_0000..=0x0601_7FFF => 1,    // VRAM
            0x0700_0000..=0x0700_03FF => 1,    // OAM

            0x0800_0000..=0x0FFF_FFFF => 1,    // Cart

            _ => 1 // Unused
        }
    }
}

impl Clockable for MemoryBus {
    fn clock(&mut self, cycles: usize) -> Option<arm::Exception> {
        let ret = self.timers.clock(cycles);
        // TODO: clock video
        // TODO: clock audio
        ret
    }
}

// Internal
impl MemoryBus {
    fn io_read_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x0400_0100..=0x0400_010F => self.timers.read_byte(addr - 0x0400_0100),
            _ => unreachable!()
        }
    }
    fn io_write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0x0400_0100..=0x0400_010F => self.timers.write_byte(addr - 0x0400_0100, data),
            _ => unreachable!()
        }
    }

    fn io_read_halfword(&mut self, addr: u32) -> u16 {
        match addr {
            0x0400_0100..=0x0400_010F => self.timers.read_halfword(addr - 0x0400_0100),
            _ => unreachable!()
        }
    }
    fn io_write_halfword(&mut self, addr: u32, data: u16) {
        match addr {
            0x0400_0100..=0x0400_010F => self.timers.write_halfword(addr - 0x0400_0100, data),
            _ => unreachable!()
        }
    }

    fn io_read_word(&mut self, addr: u32) -> u32 {
        match addr {
            0x0400_0100..=0x0400_010F => self.timers.read_word(addr - 0x0400_0100),
            _ => unreachable!()
        }
    }
    fn io_write_word(&mut self, addr: u32, data: u32) {
        match addr {
            0x0400_0100..=0x0400_010F => self.timers.write_word(addr - 0x0400_0100, data),
            _ => unreachable!()
        }
    }
}