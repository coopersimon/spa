/// Memory bus
mod wram;
mod dma;
mod cart;
mod bios;

use arm::{Mem32, MemCycleType};
use crate::{
    common::bits::u8,
    common::meminterface::{MemInterface8, MemInterface16},
    timers::Timers,
    joypad::{Joypad, Buttons},
    interrupt::InterruptControl,
    video::*,
    audio::GBAAudio
};
use dma::{DMA, DMAAddress};
use cart::{GamePak, GamePakController};
pub use wram::WRAM;

/// Game Boy Advance memory bus
pub struct MemoryBus<R: Renderer> {
    bios:       bios::BIOS,
    internal:   Internal,

    wram:       WRAM,
    fast_wram:  WRAM,

    game_pak:           GamePak,
    game_pak_control:   GamePakController,

    video:              GBAVideo<R>,

    audio:              GBAAudio,

    timers:             Timers,
    joypad:             Joypad,

    dma:                DMA,
    interrupt_control:  InterruptControl,
}

impl<R: Renderer> MemoryBus<R> {
    pub fn new(cart_path: &std::path::Path, bios_path: Option<&std::path::Path>) -> std::io::Result<Self> {
        let bios = bios::BIOS::new(bios_path)?;
        let game_pak = cart::GamePak::new(cart_path)?;
        Ok(Self {
            bios:       bios,
            internal:   Internal::new(),

            wram:       WRAM::new(256 * 1024),
            fast_wram:  WRAM::new(32 * 1024),

            game_pak:           game_pak,
            game_pak_control:   GamePakController::new(),

            video:              GBAVideo::new(R::new()),

            audio:              GBAAudio::new(),

            timers:             Timers::new(),
            joypad:             Joypad::new(),

            dma:                DMA::new(),
            interrupt_control:  InterruptControl::new(),
        })
    }

    pub fn get_frame_data(&self, buffer: &mut [u8]) {
        self.video.get_frame_data(buffer);
    }

    pub fn render_size(&self) -> (usize, usize) {
        self.video.render_size()
    }

    /// Do a DMA transfer if possible.
    /// Returns the number of cycles passed.
    /// 
    /// This function clocks the memory bus internally.
    /// It will continue until the transfer is done.
    pub fn do_dma(&mut self) -> usize {
        let mut cycle_count = 0;
        let mut last_active = 4;
        loop {
            if let Some(c) = self.dma.get_active() {
                // Check if DMA channel has changed since last transfer.
                let access = if last_active != c {
                    last_active = c;
                    self.clock(2);
                    arm::MemCycleType::N
                } else {
                    arm::MemCycleType::S
                };
                // Transfer one piece of data.
                let cycles = match self.dma.channels[c].next_addrs() {
                    DMAAddress::Addr {
                        source, dest
                    } => if self.dma.channels[c].transfer_32bit_word() {
                        let (data, load_cycles) = self.load_word(access, source);
                        let store_cycles = self.store_word(access, dest, data);
                        load_cycles + store_cycles
                    } else {
                        let (data, load_cycles) = self.load_halfword(access, source);
                        let store_cycles = self.store_halfword(access, dest, data);
                        load_cycles + store_cycles
                    },
                    DMAAddress::Done {
                        source, dest, irq
                    } => {
                        let cycles = if self.dma.channels[c].transfer_32bit_word() {
                            let (data, load_cycles) = self.load_word(access, source);
                            let store_cycles = self.store_word(access, dest, data);
                            load_cycles + store_cycles
                        } else {
                            let (data, load_cycles) = self.load_halfword(access, source);
                            let store_cycles = self.store_halfword(access, dest, data);
                            load_cycles + store_cycles
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
        let (video_signal, video_irq) = self.video.clock(cycles);
        match video_signal {
            Signal::VBlank => self.dma.on_vblank(),
            Signal::HBlank => self.dma.on_hblank(),
            Signal::None => {},
        }
        self.interrupt_control.interrupt_request(
            self.joypad.get_interrupt() |
            self.timers.clock(cycles)   |
            video_irq
            // TODO: clock audio
        );
    }

    pub fn check_irq(&self) -> bool {
        self.interrupt_control.irq()
    }

    pub fn set_button(&mut self, buttons: Buttons, pressed: bool) {
        self.joypad.set_button(buttons, pressed);
    }
}

impl<R: Renderer> Mem32 for MemoryBus<R> {
    type Addr = u32;

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_byte(addr), 1),                // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_byte(addr & 0x3_FFFF), 3),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_byte(addr & 0x7FFF), 1),  // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_byte(addr), 1),                  // I/O

            // VRAM
            0x0500_0000..=0x07FF_FFFF => (self.video.read_byte(addr), 1),

            // Cart
            0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_byte(addr - 0x0800_0000), self.game_pak_control.wait_cycles_0(cycle)),
            0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_byte(addr - 0x0A00_0000), self.game_pak_control.wait_cycles_1(cycle)),
            0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_byte(addr - 0x0C00_0000), self.game_pak_control.wait_cycles_2(cycle)),

            _ => (0, 1) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
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

            // VRAM
            0x0500_0000..=0x07FF_FFFF => {
                self.video.write_byte(addr, data);
                1
            },

            // Cart
            0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            0x0C00_0000..=0x0DFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_halfword(addr), 1),                // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_halfword(addr & 0x3_FFFF), 3),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_halfword(addr & 0x7FFF), 1),  // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_halfword(addr), 1),                  // I/O

            // VRAM
            0x0500_0000..=0x07FF_FFFF => (self.video.read_halfword(addr), 1),

            // Cart
            0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_halfword(addr - 0x0800_0000), self.game_pak_control.wait_cycles_0(cycle)),
            0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_halfword(addr - 0x0A00_0000), self.game_pak_control.wait_cycles_1(cycle)),
            0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_halfword(addr - 0x0C00_0000), self.game_pak_control.wait_cycles_2(cycle)),

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
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

            // VRAM
            0x0500_0000..=0x07FF_FFFF => {
                self.video.write_halfword(addr, data);
                1
            },

            // Cart
            0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            0x0C00_0000..=0x0DFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_word(addr), 1),                // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_word(addr & 0x3_FFFF), 6),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_word(addr & 0x7FFF), 1),  // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_word(addr), 1),                  // I/O

            // VRAM
            0x0500_0000..=0x06FF_FFFF => (self.video.read_word(addr), 2),   // VRAM & Palette
            0x0700_0000..=0x0700_03FF => (self.video.read_word(addr), 1),   // OAM

            // Cart
            0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_word(addr - 0x0800_0000), self.game_pak_control.wait_cycles_0(cycle) * 2),
            0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_word(addr - 0x0A00_0000), self.game_pak_control.wait_cycles_1(cycle) * 2),
            0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_word(addr - 0x0C00_0000), self.game_pak_control.wait_cycles_2(cycle) * 2),

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
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

            // VRAM & Palette
            0x0500_0000..=0x06FF_FFFF => {
                self.video.write_word(addr, data);
                2
            },
            // OAM
            0x0700_0000..=0x0700_03FF => {
                self.video.write_word(addr, data);
                1
            },

            // Cart
            0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle) * 2,
            0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle) * 2,
            0x0C00_0000..=0x0DFF_FFFF => self.game_pak_control.wait_cycles_2(cycle) * 2,

            _ => 1 // Unused
        }
    }
}

/// IO on the bus.
/// There are a ton of devices that sit on IO so use this macro to construct the functions.
macro_rules! MemoryBusIO {
    {$(($start_addr:expr, $end_addr:expr, $device:ident)),*} => {
        impl<R: Renderer> MemoryBus<R> {
            fn io_read_byte(&self, addr: u32) -> u8 {
                match addr {
                    $($start_addr..=$end_addr => self.$device.read_byte(addr - $start_addr),)*
                    _ => 0//panic!(format!("trying to load from unmapped io address ${:08X}", addr)),
                }
            }
            fn io_write_byte(&mut self, addr: u32, data: u8) {
                match addr {
                    $($start_addr..=$end_addr => self.$device.write_byte(addr - $start_addr, data),)*
                    _ => {}//panic!(format!("trying to write to unmapped io address ${:08X}", addr)),
                }
            }

            fn io_read_halfword(&self, addr: u32) -> u16 {
                match addr {
                    $($start_addr..=$end_addr => self.$device.read_halfword(addr - $start_addr),)*
                    _ => 0//panic!(format!("trying to load from unmapped io address ${:08X}", addr)),
                }
            }
            fn io_write_halfword(&mut self, addr: u32, data: u16) {
                match addr {
                    $($start_addr..=$end_addr => self.$device.write_halfword(addr - $start_addr, data),)*
                    _ => {}//panic!(format!("trying to write to unmapped io address ${:08X}", addr)),
                }
            }

            fn io_read_word(&self, addr: u32) -> u32 {
                match addr {
                    $($start_addr..=$end_addr => self.$device.read_word(addr - $start_addr),)*
                    _ => 0//panic!(format!("trying to load from unmapped io address ${:08X}", addr)),
                }
            }
            fn io_write_word(&mut self, addr: u32, data: u32) {
                match addr {
                    $($start_addr..=$end_addr => self.$device.write_word(addr - $start_addr, data),)*
                    _ => {}//panic!(format!("trying to write to unmapped io address ${:08X}", addr)),
                }
            }
        }
    };
}

MemoryBusIO!{
    (0x0400_0000, 0x0400_0057, video),
    (0x0400_0060, 0x0400_008F, audio),
    (0x0400_00B0, 0x0400_00DF, dma),
    (0x0400_0100, 0x0400_010F, timers),
    (0x0400_0130, 0x0400_0133, joypad),
    (0x0400_0204, 0x0400_0207, game_pak_control),
    (0x0400_0200, 0x0400_020B, interrupt_control),
    (0x0400_0300, 0x0400_0301, internal)
}

/// Internal registers which are used by the BIOS.
struct Internal {
    post_boot_flag: u8,
    
    halt:   bool,
    stop:   bool,
}

impl Internal {
    pub fn new() -> Self {
        Self {
            post_boot_flag: 0,
            halt:   false,
            stop:   false,
        }
    }
}

impl MemInterface8 for Internal {
    fn read_byte(&self, addr: u32) -> u8 {
        match addr {
            0 => self.post_boot_flag,
            1 => 0,
            2 => 0,
            3 => 0,
            _ => unreachable!()
        }
    }

    fn write_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0 => self.post_boot_flag = data & 1,
            1 => if u8::test_bit(data, 7) {
                self.stop = !self.stop;
            } else {
                self.halt = !self.halt;
            },
            2 => {},
            3 => {},
            _ => unreachable!()
        }
    }
}