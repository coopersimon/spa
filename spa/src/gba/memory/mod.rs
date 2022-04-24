/// Memory bus
mod cart;
mod swi;

use arm::{Mem32, MemCycleType};
use crossbeam_channel::{Receiver, unbounded};

use std::path::PathBuf;

use crate::{
    utils::{
        bits::u8,
        meminterface::{MemInterface8, MemInterface16}
    },
    common::{
        bios::BIOS,
        dma::{DMA, DMAAddress},
        wram::WRAM,
        timers::Timers,
        framecomms::FrameSender
    },
    gba::{
        joypad::{Joypad, Buttons},
        interrupt::{Interrupts, InterruptControl},
        video::*,
        audio::{GBAAudio, SamplePacket}
    }
};
use cart::{GamePak, GamePakController};
pub use swi::emulated_swi;

/// Locations for external files that are used by GBA.
pub struct MemoryConfig {
    pub rom_path:   PathBuf,
    pub save_path:  Option<PathBuf>,
    pub bios_path:  Option<PathBuf>,
}

/// Game Boy Advance memory bus
pub struct MemoryBus<R: Renderer> {
    bios:       BIOS,
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

    frame_sender:       FrameSender<Buttons>,
}

impl<R: Renderer> MemoryBus<R> {
    pub fn new(config: &MemoryConfig, frame_sender: FrameSender<Buttons>) -> std::io::Result<Box<Self>> {
        let bios = if let Some(path) = &config.bios_path {
            BIOS::new_from_file(&path)?
        } else {
            construct_bios()
        };
        let game_pak = cart::GamePak::new(&config.rom_path, config.save_path.as_ref().map(|p| p.as_path()))?;
        Ok(Box::new(Self {
            bios:       bios,
            internal:   Internal::new(),

            wram:       WRAM::new(256 * 1024),
            fast_wram:  WRAM::new(32 * 1024),

            game_pak:           game_pak,
            game_pak_control:   GamePakController::new(),

            video:              GBAVideo::new(R::new(frame_sender.get_frame_buffer(0))),

            audio:              GBAAudio::new(),

            timers:             Timers::new(),
            joypad:             Joypad::new(),

            dma:                DMA::new(),
            interrupt_control:  InterruptControl::new(),

            frame_sender:       frame_sender,
        }))
    }

    pub fn enable_audio(&mut self) -> (Receiver<SamplePacket>, Receiver<f64>) {
        let (sample_tx, sample_rx) = unbounded();
        let (rate_tx, rate_rx) = unbounded();
        self.audio.enable_audio(sample_tx, rate_tx);
        (sample_rx, rate_rx)
    }

    /*pub fn set_button(&mut self, buttons: Buttons, pressed: bool) {
        self.joypad.set_button(buttons, pressed);
    }*/
}

// Internal
impl<R: Renderer> MemoryBus<R> {

    /// Do a DMA transfer if possible.
    /// 
    /// This function clocks the memory bus internally.
    /// It will continue until the transfer is done.
    fn do_dma(&mut self) {
        //let mut cycle_count = 0;
        let mut last_active = 4;
        loop {
            if let Some(c) = self.dma.get_active() {
                // Check if DMA channel has changed since last transfer.
                let access = if last_active != c {
                    last_active = c;
                    if self.do_clock(2) {
                        self.frame_end();
                    }
                    //cycle_count += 2;
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
                        self.interrupt_control.interrupt_request(Interrupts::from_bits_truncate(irq));
                        self.dma.set_inactive(c);
                        cycles
                    }
                };
                if self.do_clock(cycles) {
                    self.frame_end();
                }
                //cycle_count += cycles;
            } else {
                break;
            }
        }
    }

    /// Indicate to all of the devices on the memory bus that cycles have passed.
    /// 
    /// Returns true if VBlank occurred, and therefore the frame is ready to be presented.
    fn do_clock(&mut self, cycles: usize) -> bool {
        let (video_signal, video_irq) = self.video.clock(cycles);
        let vblank = match video_signal {
            Signal::VBlank => {
                self.dma.on_vblank();
                true
            },
            Signal::HBlank => {
                self.dma.on_hblank();
                false
            },
            Signal::None => false,
        };

        let (timer_irq, timer_0, timer_1) = self.timers.clock(cycles);
        if timer_0 {
            self.audio.timer_0_tick();
        }
        if timer_1 {
            self.audio.timer_1_tick();
        }
        if self.audio.dma_1() {
            self.dma.on_sound_fifo_1();
        }
        if self.audio.dma_2() {
            self.dma.on_sound_fifo_2();
        }
        self.audio.clock(cycles);

        self.interrupt_control.interrupt_request(
            self.joypad.get_interrupt() |
            Interrupts::from_bits_truncate(timer_irq) |
            video_irq
        );

        vblank
    }

    fn check_irq(&self) -> bool {
        self.interrupt_control.irq()
    }

    /// Called when vblank occurs. Halts emulation until the next frame.
    fn frame_end(&mut self) {
        self.game_pak.flush_save();

        let buttons = self.frame_sender.sync_frame();
        self.joypad.set_all_buttons(buttons);
    }
}

impl<R: Renderer> Mem32 for MemoryBus<R> {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        if self.do_clock(cycles) {
            self.frame_end();
        }
        self.do_dma();

        // Check if CPU is halted.
        if self.internal.halt {
            loop {
                if self.do_clock(1) {
                    self.frame_end();
                }
                self.do_dma();
                if self.check_irq() {
                    self.internal.halt = false;
                    return Some(arm::ExternalException::IRQ);
                }
            }
        }

        if self.check_irq() {
            self.internal.halt = false;
            Some(arm::ExternalException::IRQ)
        } else {
            None
        }
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_byte(addr), 1),                // BIOS
            0x0200_0000..=0x02FF_FFFF => (self.wram.read_byte(addr & 0x3_FFFF), 3),     // WRAM
            0x0300_0000..=0x03FF_FFFF => (self.fast_wram.read_byte(addr & 0x7FFF), 1),  // FAST WRAM
            0x0400_0000..=0x0400_03FE => (self.io_read_byte(addr), 1),                  // I/O

            // VRAM
            0x0500_0000..=0x07FF_FFFF => (self.video.read_byte(addr), 1),

            // Cart
            0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_0(cycle)),
            0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_1(cycle)),
            0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_2(cycle)),
            0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.sram_wait_cycles()),

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
            0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            0x0D00_0000..=0x0EFF_FFFF => {
                self.game_pak.write_byte(addr, data);
                self.game_pak_control.sram_wait_cycles()
            },

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
            0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_0(cycle)),
            0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_1(cycle)),
            0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_2(cycle)),
            0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.sram_wait_cycles()),

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
            0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            0x0D00_0000..=0x0EFF_FFFF => {
                self.game_pak.write_halfword(addr, data);
                self.game_pak_control.sram_wait_cycles()
            },

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
            0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_0(cycle) << 1),
            0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_1(cycle) << 1),
            0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_2(cycle) << 1),
            0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.sram_wait_cycles() << 1),

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
            0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle) << 1,
            0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle) << 1,
            0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle) << 1,
            0x0D00_0000..=0x0EFF_FFFF => {
                self.game_pak.write_word(addr, data);
                self.game_pak_control.sram_wait_cycles() << 1
            },

            _ => 1 // Unused
        }
    }
}

impl<R: Renderer> MemoryBus<R> {
    MemoryBusIO!{
        (0x0400_0000, 0x0400_0057, video),
        (0x0400_0060, 0x0400_00AF, audio),
        (0x0400_00B0, 0x0400_00DF, dma),
        (0x0400_0100, 0x0400_010F, timers),
        (0x0400_0130, 0x0400_0133, joypad),
        (0x0400_0204, 0x0400_0207, game_pak_control),
        (0x0400_0200, 0x0400_020B, interrupt_control),
        (0x0400_0300, 0x0400_0301, internal)
    }
}

/// Internal registers which are used by the BIOS.
struct Internal {
    post_boot_flag: u8,
    
    pub halt:   bool,
    pub stop:   bool,
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
    fn read_byte(&mut self, addr: u32) -> u8 {
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
                println!("Stop!");
                self.stop = true;
            } else {
                self.halt = true;
            },
            2 => {},
            3 => {},
            _ => unreachable!()
        }
    }
}

// TODO: handle IRQ in code

/// A simple BIOS if the full ROM is not available.
/// 
/// This just deals with IRQ interrupt handling.
/// 
/// Should work for games that don't make use of SWI calls.
pub fn construct_bios() -> BIOS {
    let mut bios_mem = vec![0; 0x4000];

    write_word_to_mem(&mut bios_mem, 0x18, 0xEA00_0042);    // B 0x128
    write_word_to_mem(&mut bios_mem, 0x128, 0xE92D_500F);   // STMFD SP! R0-3,R12,R14
    write_word_to_mem(&mut bios_mem, 0x12C, 0xE3A0_0301);   // MOV R0,#0400_0000
    write_word_to_mem(&mut bios_mem, 0x130, 0xE28F_E000);   // ADD R14,R15,0
    write_word_to_mem(&mut bios_mem, 0x134, 0xE510_F004);   // LDR R15,[R0,#-4]
    write_word_to_mem(&mut bios_mem, 0x138, 0xE8BD_500F);   // LDMFD SP! R0-3,R12,R14
    write_word_to_mem(&mut bios_mem, 0x13C, 0xE25E_F004);   // SUBS R15,R14,#4

    BIOS::new_from_data(bios_mem)
}

fn write_word_to_mem(mem: &mut [u8], addr: usize, data: u32) {
    let bytes = data.to_le_bytes();
    for (dest, byte) in mem[addr..(addr + 4)].iter_mut().zip(&bytes) {
        *dest = *byte;
    }
}
