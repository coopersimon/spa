/// Memory bus
mod wram;
mod dma;

use arm::Mem32;
use crate::{
    common::meminterface::MemInterface16,
    timers::Timers,
    joypad::{Joypad, Buttons},
    interrupt::InterruptControl,
};
use dma::{DMA, DMAAddress};

/// Game Boy Advance memory bus
pub struct MemoryBus {
    wram:       wram::WRAM,
    fast_wram:  wram::WRAM,

    timers:             Timers,
    joypad:             Joypad,

    dma:                DMA,
    interrupt_control:  InterruptControl,
}

impl MemoryBus {
    pub fn new() -> Self {
        Self {
            wram:       wram::WRAM::new(256 * 1024),
            fast_wram:  wram::WRAM::new(32 * 1024),

            timers:             Timers::new(),
            joypad:             Joypad::new(),

            dma:                DMA::new(),
            interrupt_control:  InterruptControl::new(),
        }
    }

    /// Do a DMA transfer if possible.
    /// Returns the number of cycles passed.
    /// 
    /// This function clocks the memory bus internally.
    pub fn do_dma(&mut self) -> usize {
        let mut cycle_count = 0;
        loop {
            if let Some(c) = self.dma.get_active() {
                let cycles = match self.dma.channels[c].next_addrs() {
                    DMAAddress::Addr {
                        source, dest
                    } => if self.dma.channels[c].transfer_32bit_word() {
                        let (data, load_cycles) = self.load_word(source);
                        let store_cycles = self.store_word(dest, data);
                        std::cmp::max(load_cycles, store_cycles)
                    } else {
                        let (data, load_cycles) = self.load_halfword(source);
                        let store_cycles = self.store_halfword(dest, data);
                        std::cmp::max(load_cycles, store_cycles)
                    },
                    DMAAddress::Done {
                        source, dest, irq
                    } => {
                        let cycles = if self.dma.channels[c].transfer_32bit_word() {
                            let (data, load_cycles) = self.load_word(source);
                            let store_cycles = self.store_word(dest, data);
                            std::cmp::max(load_cycles, store_cycles)
                        } else {
                            let (data, load_cycles) = self.load_halfword(source);
                            let store_cycles = self.store_halfword(dest, data);
                            std::cmp::max(load_cycles, store_cycles)
                        };
                        self.interrupt_control.interrupt_request(irq);
                        self.dma.set_inactive(c);
                        cycles
                    }
                };
                self.clock(cycles);
                cycle_count += cycles;
            } else {
                break cycle_count;
            }
        }
    }

    /// Indicate to the memory bus that cycles have passed.
    /// The cycles passed into here should come from the CPU.
    pub fn clock(&mut self, cycles: usize) {
        self.interrupt_control.interrupt_request(
            self.joypad.get_interrupt() |
            self.timers.clock(cycles)
            // TODO: DMA
            // TODO: clock video
            // TODO: clock audio
        );
    }

    pub fn check_exceptions(&self) -> Option<arm::Exception> {
        self.interrupt_control.irq()
    }

    pub fn set_button(&mut self, buttons: Buttons, pressed: bool) {
        self.joypad.set_button(buttons, pressed);
    }
}

impl Mem32 for MemoryBus {
    type Addr = u32;

    fn load_byte(&mut self, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (0, 0),    // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_byte(addr & 0x3_FFFF), 3),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_byte(addr & 0x7FFF), 1),  // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_byte(addr), 1),                  // I/O

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
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.wram.write_byte(addr & 0x3_FFFF, data);
                3
            },
            0x0300_0000..=0x03FF_FFFF => {  // FAST WRAM
                self.fast_wram.write_byte(addr & 0x7FFF, data);
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
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_halfword(addr & 0x3_FFFF), 3),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_halfword(addr & 0x7FFF), 1),  // FAST WRAM
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
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.wram.write_halfword(addr & 0x3_FFFF, data);
                3
            },
            0x0300_0000..=0x03FF_FFFF => {  // FAST WRAM
                self.fast_wram.write_halfword(addr & 0x7FFF, data);
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
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_word(addr & 0x3_FFFF), 6),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_word(addr & 0x7FFF), 1),  // FAST WRAM
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
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.wram.write_word(addr & 0x3_FFFF, data);
                6
            },
            0x0300_0000..=0x03FF_FFFF => {  // FAST WRAM
                self.fast_wram.write_word(addr & 0x7FFF, data);
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

/// IO on the bus.
/// There are a ton of devices that sit on IO so use this macro to construct the functions.
macro_rules! MemoryBusIO {
    {$(($start_addr:expr, $end_addr:expr, $device:ident)),*} => {
        impl MemoryBus {
            fn io_read_byte(&mut self, addr: u32) -> u8 {
                match addr {
                    $($start_addr..=$end_addr => self.$device.read_byte(addr - $start_addr),)*
                    _ => unreachable!()
                }
            }
            fn io_write_byte(&mut self, addr: u32, data: u8) {
                match addr {
                    $($start_addr..=$end_addr => self.$device.write_byte(addr - $start_addr, data),)*
                    _ => unreachable!()
                }
            }

            fn io_read_halfword(&mut self, addr: u32) -> u16 {
                match addr {
                    $($start_addr..=$end_addr => self.$device.read_halfword(addr - $start_addr),)*
                    _ => unreachable!()
                }
            }
            fn io_write_halfword(&mut self, addr: u32, data: u16) {
                match addr {
                    $($start_addr..=$end_addr => self.$device.write_halfword(addr - $start_addr, data),)*
                    _ => unreachable!()
                }
            }

            fn io_read_word(&mut self, addr: u32) -> u32 {
                match addr {
                    $($start_addr..=$end_addr => self.$device.read_word(addr - $start_addr),)*
                    _ => unreachable!()
                }
            }
            fn io_write_word(&mut self, addr: u32, data: u32) {
                match addr {
                    $($start_addr..=$end_addr => self.$device.write_word(addr - $start_addr, data),)*
                    _ => unreachable!()
                }
            }
        }
    };
}

MemoryBusIO!{
    (0x0400_00B0, 0x0400_00DF, dma),
    (0x0400_0100, 0x0400_010F, timers),
    (0x0400_0130, 0x0400_0133, joypad),
    (0x0400_0200, 0x0400_020F, interrupt_control)
}
