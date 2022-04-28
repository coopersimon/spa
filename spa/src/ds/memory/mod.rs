mod dma;
mod main;
mod shared;
mod power;

use arm::{Mem32, MemCycleType};

use std::{
    path::PathBuf,
    sync::{Arc, Barrier}
};

use crate::{
    common::{
        bios::BIOS,
        dma::{
            DMA as ds7DMA,
            DMAAddress
        },
        timers::Timers,
        wram::WRAM,
        framecomms::FrameSender,
        joypad::Joypad
    },
    utils::{
        meminterface::{MemInterface8, MemInterface16, MemInterface32}
    },
    ds::{
        maths::Accelerators,
        ipc::IPC,
        joypad::DSJoypad,
        interrupt::{Interrupts, InterruptControl},
        card::DSCardIO,
        rtc::RealTimeClock,
        spi::SPI,
        video::*,
        input::UserInput
    }
};
use dma::DMA;
use main::MainRAM;
use shared::*;
use power::*;

/// How many cycles the ARM7 should run for before syncing.
const ARM7_THREAD_SYNC_CYCLES: usize = 100;
/// How many cycles the ARM9 should run for before syncing.
const ARM9_THREAD_SYNC_CYCLES: usize = ARM7_THREAD_SYNC_CYCLES * 2;

/// Locations for external files that are used by NDS.
pub struct MemoryConfig {
    pub rom_path:       PathBuf,
    pub save_path:      Option<PathBuf>,
    pub ds9_bios_path:  Option<PathBuf>,
    pub ds7_bios_path:  Option<PathBuf>,
    pub firmware_path:  Option<PathBuf>
}

/// Memory bus for DS ARM9 processor.
pub struct DS9MemoryBus<R: Renderer> {
    bios:           BIOS,
    post_flag:      DS9PostFlag,
    pub halt:       bool,

    main_ram:       MainRAM,
    shared_wram:    ARM9SharedRAM,

    video:          DSVideo<R>,

    ipc:            IPC,

    timers:             Timers,
    joypad:             Joypad,
    accelerators:       Accelerators,

    dma:                DMA,
    interrupt_control:  InterruptControl,
    card:               DSCardIO,

    // sync
    counter:            usize,
    barrier:            Arc<Barrier>,
    frame_sender:       FrameSender<UserInput>,
}

impl<R: Renderer> DS9MemoryBus<R> {
    pub fn new(config: &MemoryConfig, frame_sender: FrameSender<UserInput>) -> (Self, Box<DS7MemoryBus>) {
        let (arm9_wram, arm7_wram) = ARM9SharedRAM::new();
        let (ds9_ipc, ds7_ipc) = IPC::new();
        let main_ram = MainRAM::new();

        let (video, arm7_vram) = DSVideo::new(R::new(frame_sender.get_frame_buffer(0), frame_sender.get_frame_buffer(1)));

        let arm9_bios = BIOS::new_from_file(config.ds9_bios_path.as_ref().map(|p| p.as_path()).unwrap()).unwrap();
        let arm7_bios = BIOS::new_from_file(config.ds7_bios_path.as_ref().map(|p| p.as_path()).unwrap()).unwrap();
        let spi = SPI::new(config.firmware_path.as_ref().map(|p| p.as_path()));

        let key1 = (0..0x412).map(|n| arm7_bios.read_word(0x30 + (n*4))).collect::<Vec<_>>();
        let (card_9, card_7) = DSCardIO::new(&config.rom_path, key1).unwrap();

        let barrier = Arc::new(Barrier::new(2));

        (Self{
            bios:               arm9_bios,
            post_flag:          DS9PostFlag::new(),
            halt:               false,

            main_ram:           main_ram.clone(),
            shared_wram:        arm9_wram,

            video:              video,

            ipc:                ds9_ipc,
            timers:             Timers::new(),
            joypad:             Joypad::new(),
            accelerators:       Accelerators::new(),
            dma:                DMA::new(),
            interrupt_control:  InterruptControl::new(),
            card:               card_9,
            
            counter:            0,
            barrier:            barrier.clone(),
            frame_sender:       frame_sender
        }, Box::new(DS7MemoryBus{
            bios:               arm7_bios,
            power_control:      DS7PowerControl::new(),

            main_ram:           main_ram,
            wram:               WRAM::new(64 * 1024),
            shared_wram:        arm7_wram,

            vram:               arm7_vram,

            ipc:                ds7_ipc,
            timers:             Timers::new(),
            joypad:             Joypad::new(),
            ds_joypad:          DSJoypad::new(),
            rtc:                RealTimeClock::new(),
            spi:                spi,

            dma:                ds7DMA::new(),
            interrupt_control:  InterruptControl::new(),
            card:               card_7,

            counter:            0,
            barrier:            barrier
        }))
    }
}

// Internal
impl <R: Renderer> DS9MemoryBus<R> {
    fn read_mem_control_byte(&self, addr: u32) -> u8 {
        match addr {
            0 => self.video.mem.get_a_cnt(),
            1 => self.video.mem.get_b_cnt(),
            2 => self.video.mem.get_c_cnt(),
            3 => self.video.mem.get_d_cnt(),
            4 => self.video.mem.get_e_cnt(),
            5 => self.video.mem.get_f_cnt(),
            6 => self.video.mem.get_g_cnt(),
            7 => self.shared_wram.get_bank_control(),
            8 => self.video.mem.get_h_cnt(),
            9 => self.video.mem.get_i_cnt(),
            _ => 0,
        }
    }

    fn write_mem_control_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0 => self.video.mem.set_a_cnt(data),
            1 => self.video.mem.set_b_cnt(data),
            2 => self.video.mem.set_c_cnt(data),
            3 => self.video.mem.set_d_cnt(data),
            4 => self.video.mem.set_e_cnt(data),
            5 => self.video.mem.set_f_cnt(data),
            6 => self.video.mem.set_g_cnt(data),
            7 => self.shared_wram.set_bank_control(data),
            8 => self.video.mem.set_h_cnt(data),
            9 => self.video.mem.set_i_cnt(data),
            _ => {},
        }
    }

    fn read_mem_control_halfword(&self, addr: u32) -> u16 {
        use crate::utils::bytes::u16;
        u16::make(
            self.read_mem_control_byte(addr),
            self.read_mem_control_byte(addr + 1),
        )
    }

    fn write_mem_control_halfword(&mut self, addr: u32, data: u16) {
        use crate::utils::bytes::u16;
        self.write_mem_control_byte(addr, u16::lo(data));
        self.write_mem_control_byte(addr + 1, u16::hi(data));
    }

    fn read_mem_control_word(&self, addr: u32) -> u32 {
        u32::from_le_bytes([
            self.read_mem_control_byte(addr),
            self.read_mem_control_byte(addr + 1),
            self.read_mem_control_byte(addr + 2),
            self.read_mem_control_byte(addr + 3)
        ])
    }

    fn write_mem_control_word(&mut self, addr: u32, data: u32) {
        let bytes = data.to_le_bytes();
        self.write_mem_control_byte(addr, bytes[0]);
        self.write_mem_control_byte(addr + 1, bytes[1]);
        self.write_mem_control_byte(addr + 2, bytes[2]);
        self.write_mem_control_byte(addr + 3, bytes[3]);
    }

    /// Do a DMA transfer if possible.
    /// 
    /// This function clocks the memory bus internally.
    /// It will continue until the transfer is done.
    fn do_dma(&mut self) {
        // TODO: keep executing if inside cache?
        let mut last_active = 4;
        loop {
            if let Some(c) = self.dma.get_active() {
                // Check if DMA channel has changed since last transfer.
                let access = if last_active != c {
                    last_active = c;
                    if self.do_clock(2) {
                        self.frame_end();
                    }
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
                        self.interrupt_control.interrupt_request(Interrupts::from_bits_truncate(irq as u32));
                        self.dma.set_inactive(c);
                        cycles
                    }
                };
                if self.do_clock(cycles) {
                    self.frame_end();
                }
            } else {
                break;
            }
        }
    }

    /// Indicate to all of the devices on the memory bus that cycles have passed.
    /// 
    /// Returns true if VBlank occurred, and therefore the frame is ready to be presented.
    fn do_clock(&mut self, cycles: usize) -> bool {
        self.counter += cycles;
        if self.counter >= ARM9_THREAD_SYNC_CYCLES {
            self.counter -= ARM9_THREAD_SYNC_CYCLES;
            self.barrier.wait();
        }

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

        let (timer_irq, _, _) = self.timers.clock(cycles);
        let joypad_irq = if self.joypad.get_interrupt() {
            Interrupts::KEYPAD
        } else {
            Interrupts::empty()
        };

        self.interrupt_control.interrupt_request(
            joypad_irq |
            Interrupts::from_bits_truncate(timer_irq.into()) |
            self.ipc.get_interrupts() |
            self.card.get_interrupt() |
            video_irq
        );

        vblank
    }

    fn check_irq(&self) -> bool {
        self.interrupt_control.irq()
    }

    /// Called when vblank occurs. Halts emulation until the next frame.
    fn frame_end(&mut self) {
        //self.game_pak.flush_save();

        let input = self.frame_sender.sync_frame();
        self.joypad.set_all_buttons(input.buttons);
        // TODO: send input to ARM7
    }
}

impl<R: Renderer> Mem32 for DS9MemoryBus<R> {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        if self.do_clock(cycles) {
            self.frame_end();
        }
        self.do_dma();

        // Check if CPU is halted.
        if self.halt {
            loop {
                if self.do_clock(1) {
                    self.frame_end();
                }
                self.do_dma();
                if self.check_irq() {
                    self.halt = false;
                    return Some(arm::ExternalException::IRQ);
                }
            }
        }

        if self.check_irq() {
            self.halt = false;
            Some(arm::ExternalException::IRQ)
        } else {
            None
        }
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => (
                self.main_ram.read_byte(addr & 0x3F_FFFF),
                if cycle.is_non_seq() {18} else {2}
            ),
            0x0300_0000..=0x03FF_FFFF => (
                self.shared_wram.read_byte(addr),
                if cycle.is_non_seq() {8} else {2}
            ),

            // I/O
            0x0400_0240..=0x0400_024B => (self.read_mem_control_byte(addr & 0xF), if cycle.is_non_seq() {8} else {2}),
            0x0400_1000..=0x0400_106F => (self.video.mem.mut_engine_b().registers.read_byte(addr & 0xFF), if cycle.is_non_seq() {8} else {2}),
            0x0410_0000..=0x0410_0003 => (self.ipc.read_byte(addr), if cycle.is_non_seq() {8} else {2}),
            0x0410_0010..=0x0410_0013 => (self.card.read_byte(addr), if cycle.is_non_seq() {8} else {2}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_byte(addr), if cycle.is_non_seq() {8} else {2}),

            // VRAM
            0x0500_0000..=0x05FF_FFFF => (self.video.mem.read_byte_palette(addr & 0x7FF), 2),
            0x0600_0000..=0x06FF_FFFF => (self.video.mem.read_byte_vram(addr), 2),
            0x0700_0000..=0x07FF_FFFF => (self.video.mem.read_byte_oam(addr & 0x7FF), 2),

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.sram_wait_cycles()),

            0xFFFF_0000..=0xFFFF_FFFF => (self.bios.read_byte(addr & 0xFFF), if cycle.is_non_seq() {8} else {2}),

            _ => (0, 2) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_byte(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {18} else {2}
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared RAM
                self.shared_wram.write_byte(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            // I/O
            0x0400_0240..=0x0400_024B => {
                self.write_mem_control_byte(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_1000..=0x0400_106F => {
                self.video.mem.mut_engine_b().registers.write_byte(addr & 0xFF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0410_0010..=0x0410_0013 => {
                self.card.write_byte(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_byte(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // VRAM
            0x0500_0000..=0x05FF_FFFF => {
                self.video.mem.write_byte_palette(addr, data);
                2
            },
            0x0600_0000..=0x06FF_FFFF => {
                self.video.mem.write_byte_vram(addr, data);
                2
            },
            0x0700_0000..=0x07FF_FFFF => {
                self.video.mem.write_byte_oam(addr, data);
                2
            },

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_byte(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => (self.main_ram.read_halfword(addr & 0x3F_FFFF), if cycle.is_non_seq() {18} else {2}),
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_halfword(addr), if cycle.is_non_seq() {8} else {2}),

            // I/O
            // TODO: mem ctl
            0x0400_0240..=0x0400_024B => (self.read_mem_control_halfword(addr & 0xF), if cycle.is_non_seq() {8} else {2}),
            0x0400_1000..=0x0400_106F => (self.video.mem.mut_engine_b().registers.read_halfword(addr & 0xFF), 2),
            0x0410_0000..=0x0410_0003 => (self.ipc.read_halfword(addr), if cycle.is_non_seq() {8} else {2}),
            0x0410_0010..=0x0410_0013 => (self.card.read_halfword(addr), if cycle.is_non_seq() {8} else {2}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_halfword(addr), if cycle.is_non_seq() {8} else {2}),

            // VRAM
            0x0500_0000..=0x05FF_FFFF => (self.video.mem.read_halfword_palette(addr & 0x7FF), 2),
            0x0600_0000..=0x06FF_FFFF => (self.video.mem.read_halfword_vram(addr), 2),
            0x0700_0000..=0x07FF_FFFF => (self.video.mem.read_halfword_oam(addr & 0x7FF), 2),

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.sram_wait_cycles()),

            0xFFFF_0000..=0xFFFF_FFFF => (self.bios.read_halfword(addr & 0xFFF), if cycle.is_non_seq() {8} else {2}),

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_halfword(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {18} else {2}
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared WRAM
                self.shared_wram.write_halfword(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // I/O
            0x0400_0240..=0x0400_024B => {
                self.write_mem_control_halfword(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_1000..=0x0400_106F => {
                self.video.mem.mut_engine_b().registers.write_halfword(addr & 0xFF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0410_0010..=0x0410_0013 => {
                self.card.write_halfword(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_0000..=0x04FF_FFFF => {
                self.io_write_halfword(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // VRAM
            0x0500_0000..=0x05FF_FFFF => {
                self.video.mem.write_halfword_palette(addr, data);
                2
            },
            0x0600_0000..=0x06FF_FFFF => {
                self.video.mem.write_halfword_vram(addr, data);
                2
            },
            0x0700_0000..=0x07FF_FFFF => {
                self.video.mem.write_halfword_oam(addr, data);
                2
            },

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_halfword(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => (self.main_ram.read_word(addr & 0x3F_FFFF), if cycle.is_non_seq() {20} else {4}),
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_word(addr), if cycle.is_non_seq() {8} else {2}),

            // I/O
            0x0400_0240..=0x0400_024B => (self.read_mem_control_word(addr & 0xF), if cycle.is_non_seq() {8} else {2}),
            0x0400_1000..=0x0400_106F => (self.video.mem.mut_engine_b().registers.read_word(addr & 0xFF), 2),
            0x0410_0000..=0x0410_0003 => (self.ipc.read_word(addr), if cycle.is_non_seq() {8} else {2}),
            0x0410_0010..=0x0410_0013 => (self.card.read_word(addr), if cycle.is_non_seq() {8} else {2}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_word(addr), if cycle.is_non_seq() {8} else {2}),

            // VRAM
            0x0500_0000..=0x05FF_FFFF => (self.video.mem.read_word_palette(addr & 0x7FF), 4),
            0x0600_0000..=0x06FF_FFFF => (self.video.mem.read_word_vram(addr), 4),
            0x0700_0000..=0x07FF_FFFF => (self.video.mem.read_word_oam(addr & 0x7FF), 2),

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_0(cycle) << 1),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_1(cycle) << 1),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_2(cycle) << 1),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.sram_wait_cycles() << 1),

            0xFFFF_0000..=0xFFFF_FFFF => (self.bios.read_word(addr & 0xFFF), if cycle.is_non_seq() {8} else {2}),

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_word(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {20} else {4}
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared WRAM
                self.shared_wram.write_word(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // I/O
            0x0400_0240..=0x0400_024B => {
                self.write_mem_control_word(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_1000..=0x0400_106F => {
                self.video.mem.mut_engine_b().registers.write_word(addr & 0xFF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0410_0010..=0x0410_0013 => {
                self.card.write_word(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_0000..=0x04FF_FFFF => {
                self.io_write_word(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // VRAM
            0x0500_0000..=0x05FF_FFFF => {
                self.video.mem.write_word_palette(addr, data);
                4
            },
            0x0600_0000..=0x06FF_FFFF => {
                self.video.mem.write_word_vram(addr, data);
                4
            },
            0x0700_0000..=0x07FF_FFFF => {
                self.video.mem.write_word_oam(addr, data);
                2
            },

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle) << 1,
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle) << 1,
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle) << 1,
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_word(addr, data);
            //    self.game_pak_control.sram_wait_cycles() << 1
            //},

            _ => 1 // Unused
        }
    }
}

impl<R: Renderer> DS9MemoryBus<R> {
    MemoryBusIO!{
        (0x0400_0000, 0x0400_006F, video),
        (0x0400_00B0, 0x0400_00EF, dma),
        (0x0400_0100, 0x0400_010F, timers),
        (0x0400_0130, 0x0400_0133, joypad),
        (0x0400_0180, 0x0400_018F, ipc),
        (0x0400_01A0, 0x0400_01BF, card),
        (0x0400_0208, 0x0400_0217, interrupt_control),
        (0x0400_0280, 0x0400_02BF, accelerators),
        (0x0400_0300, 0x0400_0301, post_flag)
    }
}

/// Memory bus for DS ARM7 processor.
pub struct DS7MemoryBus {
    bios:           BIOS,
    power_control:  DS7PowerControl,

    main_ram:       MainRAM,
    wram:           WRAM,
    shared_wram:    ARM7SharedRAM,

    vram:           ARM7VRAM,

    ipc:    IPC,

    timers:     Timers,
    joypad:     Joypad,
    ds_joypad:  DSJoypad,
    rtc:        RealTimeClock,
    spi:        SPI,

    dma:    ds7DMA,
    interrupt_control:  InterruptControl,
    card:               DSCardIO,

    counter:            usize,
    barrier:            Arc<Barrier>
}

// Internal
impl DS7MemoryBus {
    /// Do a DMA transfer if possible.
    /// 
    /// This function clocks the memory bus internally.
    /// It will continue until the transfer is done.
    fn do_dma(&mut self) {
        // TODO: keep executing if inside cache?
        let mut last_active = 4;
        loop {
            if let Some(c) = self.dma.get_active() {
                // Check if DMA channel has changed since last transfer.
                let access = if last_active != c {
                    last_active = c;
                    if self.do_clock(4) {
                        //self.frame_end();
                    }
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
                        self.interrupt_control.interrupt_request(Interrupts::from_bits_truncate(irq as u32));
                        self.dma.set_inactive(c);
                        cycles
                    }
                };
                if self.do_clock(cycles) {
                    //self.frame_end();
                }
            } else {
                break;
            }
        }
    }

    /// Indicate to all of the devices on the memory bus that cycles have passed.
    /// 
    /// Returns true if VBlank occurred, and therefore the frame is ready to be presented.
    fn do_clock(&mut self, cycles: usize) -> bool {
        self.counter += cycles;
        if self.counter >= ARM7_THREAD_SYNC_CYCLES {
            self.counter -= ARM7_THREAD_SYNC_CYCLES;
            self.barrier.wait();
        }

        self.card.clock(cycles);

        let (timer_irq, _, _) = self.timers.clock(cycles);
        //self.audio.clock(cycles);

        let joypad_irq = if self.joypad.get_interrupt() {
            Interrupts::KEYPAD
        } else {
            Interrupts::empty()
        };

        self.interrupt_control.interrupt_request(
            joypad_irq |
            Interrupts::from_bits_truncate(timer_irq.into()) |
            self.ipc.get_interrupts() |
            self.card.get_interrupt()
        );

        false
    }

    fn check_irq(&self) -> bool {
        self.interrupt_control.irq()
    }
}

impl Mem32 for DS7MemoryBus {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        if self.do_clock(cycles) {
            //self.frame_end();
        }
        self.do_dma();

        // Check if CPU is halted.
        if self.power_control.halt {
            loop {
                if self.do_clock(1) {
                    //self.frame_end();
                }
                self.do_dma();
                if self.check_irq() {
                    self.power_control.halt = false;
                    return Some(arm::ExternalException::IRQ);
                }
            }
        }

        if self.check_irq() {
            self.power_control.halt = false;
            Some(arm::ExternalException::IRQ)
        } else {
            None
        }
    }

    fn load_byte(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u8, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_byte(addr), 1),

            0x0200_0000..=0x02FF_FFFF => (self.main_ram.read_byte(addr & 0x3F_FFFF), if cycle.is_non_seq() {9} else {1}),
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_byte(addr), 1),
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_byte(addr & 0xFFFF), 1),

            0x0400_0241 => (self.shared_wram.get_bank_status(), if cycle.is_non_seq() {4} else {1}),
            0x0410_0000..=0x0410_0003 => (self.ipc.read_byte(addr), if cycle.is_non_seq() {4} else {1}),
            0x0410_0010..=0x0410_0013 => (self.card.read_byte(addr), if cycle.is_non_seq() {4} else {1}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_byte(addr), 1),

            0x0600_0000..=0x06FF_FFFF => (self.vram.read_byte(addr), 1),

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_byte(addr), self.game_pak_control.sram_wait_cycles()),

            _ => (0, 1) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_byte(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {9} else {1}
            },
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_byte(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_byte(addr & 0xFFFF, data);
                1
            },

            0x0410_0010..=0x0410_0013 => {
                self.card.write_byte(addr, data);
                if cycle.is_non_seq() {4} else {1}
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_byte(addr, data);
                1
            },

            0x0600_0000..=0x06FF_FFFF => {
                self.vram.write_byte(addr, data);
                1
            },

            // TODO: GBA slot
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_byte(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_halfword(addr), 1),
            0x0200_0000..=0x02FF_FFFF => (self.main_ram.read_halfword(addr & 0x3F_FFFF), if cycle.is_non_seq() {9} else {1}),
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_halfword(addr), 1),
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_halfword(addr & 0xFFFF), 1),

            0x0410_0000..=0x0410_0003 => (self.ipc.read_halfword(addr), if cycle.is_non_seq() {4} else {1}),
            0x0410_0010..=0x0410_0013 => (self.card.read_halfword(addr), if cycle.is_non_seq() {4} else {1}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_halfword(addr), 1),

            0x0600_0000..=0x06FF_FFFF => (self.vram.read_halfword(addr), 1),

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_0(cycle)),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_1(cycle)),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.wait_cycles_2(cycle)),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_halfword(addr), self.game_pak_control.sram_wait_cycles()),

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_halfword(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {9} else {1}
            },
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_halfword(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_halfword(addr & 0xFFFF, data);
                1
            },

            0x0410_0010..=0x0410_0013 => {
                self.card.write_halfword(addr, data);
                if cycle.is_non_seq() {4} else {1}
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_halfword(addr, data);
                1
            },

            0x0600_0000..=0x06FF_FFFF => {
                self.vram.write_halfword(addr, data);
                1
            },

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle),
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle),
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle),
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_halfword(addr, data);
            //    self.game_pak_control.sram_wait_cycles()
            //},

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_word(addr), 1),
            0x0200_0000..=0x02FF_FFFF => (self.main_ram.read_word(addr & 0x3F_FFFF), if cycle.is_non_seq() {10} else {2}),
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_word(addr), 1),
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_word(addr & 0xFFFF), 1),

            0x0410_0000..=0x0410_0003 => (self.ipc.read_word(addr), if cycle.is_non_seq() {4} else {1}),
            0x0410_0010..=0x0410_0013 => (self.card.read_word(addr), if cycle.is_non_seq() {4} else {1}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_word(addr), 1),

            0x0600_0000..=0x06FF_FFFF => (self.vram.read_word(addr), 1),

            // Cart
            //0x0800_0000..=0x09FF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_0(cycle) << 1),
            //0x0A00_0000..=0x0BFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_1(cycle) << 1),
            //0x0C00_0000..=0x0DFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.wait_cycles_2(cycle) << 1),
            //0x0E00_0000..=0x0EFF_FFFF => (self.game_pak.read_word(addr), self.game_pak_control.sram_wait_cycles() << 1),

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_word(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {10} else {2}
            },
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_word(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_word(addr & 0xFFFF, data);
                1
            },

            0x0410_0010..=0x0410_0013 => {
                self.card.write_word(addr, data);
                if cycle.is_non_seq() {4} else {1}
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_word(addr, data);
                1
            },

            0x0600_0000..=0x06FF_FFFF => {
                self.vram.write_word(addr, data);
                1
            },

            // Cart
            //0x0800_0000..=0x09FF_FFFF => self.game_pak_control.wait_cycles_0(cycle) << 1,
            //0x0A00_0000..=0x0BFF_FFFF => self.game_pak_control.wait_cycles_1(cycle) << 1,
            //0x0C00_0000..=0x0CFF_FFFF => self.game_pak_control.wait_cycles_2(cycle) << 1,
            //0x0D00_0000..=0x0EFF_FFFF => {
            //    self.game_pak.write_word(addr, data);
            //    self.game_pak_control.sram_wait_cycles() << 1
            //},

            _ => 1 // Unused
        }
    }
}

impl DS7MemoryBus {
    MemoryBusIO!{
        (0x0400_00B0, 0x0400_00DF, dma),
        (0x0400_0100, 0x0400_010F, timers),
        (0x0400_0130, 0x0400_0133, joypad),
        (0x0400_0136, 0x0400_0137, ds_joypad),
        (0x0400_0138, 0x0400_013B, rtc),
        (0x0400_0180, 0x0400_018F, ipc),
        (0x0400_01A0, 0x0400_01BF, card),
        (0x0400_01C0, 0x0400_01C3, spi),
        (0x0400_0208, 0x0400_0217, interrupt_control),
        (0x0400_0300, 0x0400_0303, power_control)
    }
}
