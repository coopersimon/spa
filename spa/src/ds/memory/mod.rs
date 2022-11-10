mod dma;
mod main;
mod shared;
mod power;
mod exmem;

use arm::{Mem32, MemCycleType};
use crossbeam_channel::{Sender, Receiver, bounded, unbounded};

use std::{
    path::PathBuf,
    sync::{Arc, Barrier}
};

use crate::{
    common::{
        mem::{
            bios::BIOS,
            ram::RAM,
        },
        peripheral::{
            dma::{
                DMA as ds7DMA,
                DMAAddress
            },
            timers::Timers,
            joypad::Joypad,
        },
        video::framecomms::FrameSender,
        resampler::SamplePacket
    },
    utils::{
        meminterface::{MemInterface8, MemInterface16, MemInterface32}
    },
    ds::{
        maths::Accelerators,
        ipc::IPC,
        joypad::DSJoypad,
        interrupt::{Interrupts, InterruptControl},
        card::*,
        rtc::RealTimeClock,
        spi::SPI,
        video::*,
        audio::DSAudio,
        input::UserInput
    }
};
use dma::DMA;
use main::MainRAM;
use shared::*;
use power::*;
use exmem::*;

/// How many cycles the ARM7 should run for before syncing.
const ARM7_THREAD_SYNC_CYCLES: usize = 2000;
/// How many cycles the ARM9 should run for before syncing.
const ARM9_THREAD_SYNC_CYCLES: usize = ARM7_THREAD_SYNC_CYCLES * 2;

/// Locations for external files that are used by NDS.
pub struct MemoryConfig {
    pub rom_path:       PathBuf,
    pub save_path:      Option<PathBuf>,
    pub ds9_bios_path:  Option<PathBuf>,
    pub ds7_bios_path:  Option<PathBuf>,
    pub firmware_path:  Option<PathBuf>,

    pub fast_boot:      bool
}

/// Memory bus for DS ARM9 processor.
pub struct DS9MemoryBus<R: Renderer> {
    bios:           BIOS,
    power_control:  DS9PowerControl,
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
    ex_mem_control:     ExMemControl,
    card:               DSCardIO,

    // sync
    inner_counter:      usize,
    counter:            usize,
    barrier:            Arc<Barrier>,
    frame_sender:       FrameSender<UserInput>,
    input_send:         Sender<UserInput>
}

impl<R: Renderer> DS9MemoryBus<R> {
    pub fn new(config: &MemoryConfig, frame_sender: FrameSender<UserInput>) -> (Self, Box<DS7MemoryBus>) {
        let (arm9_wram, arm7_wram) = ARM9SharedRAM::new();
        let (ds9_ipc, ds7_ipc) = IPC::new();
        let main_ram = MainRAM::new();

        let (arm9_video, arm7_video, arm7_vram) = DSVideo::new(frame_sender.get_frame_buffer(0), frame_sender.get_frame_buffer(1));

        let arm9_bios = BIOS::new_from_file(config.ds9_bios_path.as_ref().map(|p| p.as_path()).unwrap()).unwrap();
        let arm7_bios = BIOS::new_from_file(config.ds7_bios_path.as_ref().map(|p| p.as_path()).unwrap()).unwrap();
        let spi = SPI::new(config.firmware_path.as_ref().map(|p| p.as_path()));

        let (ex_mem_control, ex_mem_status) = ExMemControl::new();
        let key1 = (0..0x412).map(|n| arm7_bios.read_word(0x30 + (n*4))).collect::<Vec<_>>();
        let (card_9, card_7) = DSCardIO::new(&config.rom_path, config.save_path.clone(), key1).unwrap();

        let barrier = Arc::new(Barrier::new(2));
        let (input_send, input_recv) = bounded(1);

        (Self{
            bios:               arm9_bios,
            power_control:      DS9PowerControl::new(config.fast_boot),
            halt:               false,

            main_ram:           main_ram.clone(),
            shared_wram:        arm9_wram,

            video:              arm9_video,

            ipc:                ds9_ipc,
            timers:             Timers::new(),
            joypad:             Joypad::new(),
            accelerators:       Accelerators::new(),
            dma:                DMA::new(),
            interrupt_control:  InterruptControl::new("ARM9"),
            ex_mem_control:     ex_mem_control,
            card:               card_9,
            
            inner_counter:      0,
            counter:            0,
            barrier:            barrier.clone(),
            frame_sender:       frame_sender,
            input_send:         input_send
        }, Box::new(DS7MemoryBus{
            bios:               arm7_bios,
            power_control:      DS7PowerControl::new(config.fast_boot),

            main_ram:           main_ram,
            wram:               RAM::new(64 * 1024),
            shared_wram:        arm7_wram,

            video:              arm7_video,
            vram:               arm7_vram,

            audio:              DSAudio::new(),

            ipc:                ds7_ipc,
            timers:             Timers::new(),
            joypad:             Joypad::new(),
            ds_joypad:          DSJoypad::new(),
            rtc:                RealTimeClock::new(),
            spi:                spi,

            dma:                ds7DMA::new(),
            interrupt_control:  InterruptControl::new("ARM7"),
            ex_mem_status:      ex_mem_status,
            card:               card_7,

            inner_counter:      0,
            counter:            0,
            v_counter:          0,
            barrier:            barrier,
            input_recv:         input_recv
        }))
    }

    /// Get the game cart header.
    pub fn get_header(&self) -> CardHeader {
        self.card.get_header()
    }

    /// Setup ARM9 boot area, for fast booting without BIOS.
    /// 
    /// Also copies header into RAM.
    pub fn setup_boot_area(&mut self, header: &CardHeader) {
        let boot_area_size = header.arm9_size() as usize;
        let mut buffer = vec![0_u8; boot_area_size];
        self.card.load_data(header.arm9_rom_offset(), &mut buffer);

        let arm9_addr = header.arm9_ram_addr();
        for (n, byte) in buffer.iter().enumerate() {
            self.store_byte(MemCycleType::N, arm9_addr + (n as u32), *byte);
        }

        // Copy header data into top of RAM.
        for (n, byte) in header.as_slice().iter().enumerate() {
            self.main_ram.write_byte((0x3F_FE00 + n) as u32, *byte);
        }
        
        // Autostart.
        self.main_ram.write_byte(0x3F_FE1F, 4);

        self.card.fast_boot();

        // Write additional data into RAM.
        self.main_ram.write_word(0x3F_F800, self.card.get_rom_id());
        self.main_ram.write_word(0x3F_F804, self.card.get_rom_id());
        self.main_ram.write_word(0x3F_F880, 7); // NDS9-7 msg
        self.main_ram.write_word(0x3F_F884, 6); // NDS7 status
        self.main_ram.write_word(0x3F_F890, 0xB0002A22); // Boot flags
        self.main_ram.write_word(0x3F_FC00, self.card.get_rom_id());
        self.main_ram.write_word(0x3F_FC04, self.card.get_rom_id());
        self.main_ram.write_word(0x3F_FC40, 1); // Boot flag

        // Write user settings into RAM.
        // Birthday:
        self.main_ram.write_byte(0x3F_FC83, 0x1);
        self.main_ram.write_byte(0x3F_FC84, 0x1);
        // Language:
        self.main_ram.write_halfword(0x3F_FCE4, 0xEC_41);
        // Touchscreen calibration:
        self.main_ram.write_halfword(0x3F_FCD8, 0);   // ADC.X1
        self.main_ram.write_halfword(0x3F_FCDA, 0);   // ADC.Y1
        self.main_ram.write_halfword(0x3F_FCDE, 0xFF0);   // ADC.X2
        self.main_ram.write_halfword(0x3F_FCE0, 0xBF0);   // ADC.Y2
        self.main_ram.write_byte(0x3F_FCDC, 0);   // SCR.X1
        self.main_ram.write_byte(0x3F_FCDD, 0);   // SCR.Y1
        self.main_ram.write_byte(0x3F_FCE2, 255);   // SCR.X2
        self.main_ram.write_byte(0x3F_FCE3, 191);   // SCR.Y2
    }
}

// Internal
impl <R: Renderer> DS9MemoryBus<R> {
    fn read_mem_control_byte(&self, addr: u32) -> u8 {
        match addr {
            0 => self.video.mem.get_cnt(VRAMRegion::A),
            1 => self.video.mem.get_cnt(VRAMRegion::B),
            2 => self.video.mem.get_cnt(VRAMRegion::C),
            3 => self.video.mem.get_cnt(VRAMRegion::D),
            4 => self.video.mem.get_cnt(VRAMRegion::E),
            5 => self.video.mem.get_cnt(VRAMRegion::F),
            6 => self.video.mem.get_cnt(VRAMRegion::G),
            7 => self.shared_wram.get_bank_control(),
            8 => self.video.mem.get_cnt(VRAMRegion::H),
            9 => self.video.mem.get_cnt(VRAMRegion::I),
            _ => 0,
        }
    }

    fn write_mem_control_byte(&mut self, addr: u32, data: u8) {
        match addr {
            0 => self.video.mem.set_cnt(VRAMRegion::A, data),
            1 => self.video.mem.set_cnt(VRAMRegion::B, data),
            2 => self.video.mem.set_cnt(VRAMRegion::C, data),
            3 => self.video.mem.set_cnt(VRAMRegion::D, data),
            4 => self.video.mem.set_cnt(VRAMRegion::E, data),
            5 => self.video.mem.set_cnt(VRAMRegion::F, data),
            6 => self.video.mem.set_cnt(VRAMRegion::G, data),
            7 => self.shared_wram.set_bank_control(data),
            8 => self.video.mem.set_cnt(VRAMRegion::H, data),
            9 => self.video.mem.set_cnt(VRAMRegion::I, data),
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
        let mut cycles = 0;
        loop {
            if let Some(c) = self.dma.get_active() {
                // Check if DMA channel has changed since last transfer.
                let access = if last_active != c {
                    last_active = c;
                    if self.do_clock(4) {
                        self.frame_end();
                    }
                    arm::MemCycleType::N
                } else {
                    arm::MemCycleType::S
                };
                // Transfer one piece of data.
                match self.dma.channels[c].next_addrs() {
                    DMAAddress::Addr {
                        source, dest
                    } => {
                        cycles += if self.dma.channels[c].transfer_32bit_word() {
                            let (data, load_cycles) = self.load_word(access, source);
                            let store_cycles = self.store_word(access, dest, data);
                            load_cycles + store_cycles
                        } else {
                            let (data, load_cycles) = self.load_halfword(access, source);
                            let store_cycles = self.store_halfword(access, dest, data);
                            load_cycles + store_cycles
                        };

                        if cycles > 8 {
                            if self.do_clock(cycles) {
                                self.frame_end();
                            }
                            cycles = 0;
                        }
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

                        if self.do_clock(cycles) {
                            self.frame_end();
                        }
                    }
                };
            } else {
                break;
            }
        }

        if cycles > 0 {
            if self.do_clock(cycles) {
                self.frame_end();
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

        let (video_signal, video_irq, geom_fifo_dma) = self.video.clock(cycles);
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
        if geom_fifo_dma {
            self.dma.on_geom_fifo();
        }

        let (timer_irq, _, _) = self.timers.clock(cycles);
        let joypad_irq = if self.joypad.get_interrupt() {
            Interrupts::KEYPAD
        } else {
            Interrupts::empty()
        };

        if self.card.check_card_dma() {
            self.dma.on_card();
        }

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
        let input = self.frame_sender.sync_frame();
        self.joypad.set_all_buttons(input.buttons);
        self.input_send.send(input).unwrap();
    }
}

const DS7_MAIN_RAM_N: usize = 5;
const DS7_MAIN_RAM_S: usize = 1;
const DS7_MAIN_RAM_WORD_N: usize = DS7_MAIN_RAM_N + DS7_MAIN_RAM_S;
const DS7_MAIN_RAM_WORD_S: usize = DS7_MAIN_RAM_S + DS7_MAIN_RAM_S;

const DS9_MAIN_RAM_N: usize = DS7_MAIN_RAM_N * 2;
const DS9_MAIN_RAM_S: usize = DS7_MAIN_RAM_S * 2;
const DS9_MAIN_RAM_WORD_N: usize = DS9_MAIN_RAM_N + DS9_MAIN_RAM_S;
const DS9_MAIN_RAM_WORD_S: usize = DS9_MAIN_RAM_S + DS9_MAIN_RAM_S;

impl<R: Renderer> Mem32 for DS9MemoryBus<R> {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        // Check if CPU is halted.
        if self.halt {
            loop {
                if self.do_clock(8) {
                    self.frame_end();
                }
                self.do_dma();
                if self.check_irq() {
                    self.halt = false;
                    break;
                }
            }
        } else {
            if self.dma.get_active().is_some() {
                self.do_dma();
            }
        }

        self.inner_counter += cycles;
        if self.inner_counter < 16 {
            if self.check_irq() {
                return Some(arm::ExternalException::IRQ);
            }
            return None;
        }

        if self.do_clock(self.inner_counter) {
            self.frame_end();
        }
        self.inner_counter = 0;

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
                if cycle.is_non_seq() {DS9_MAIN_RAM_N} else {DS9_MAIN_RAM_S}  // TODO: S=N for instr
            ),
            0x0300_0000..=0x03FF_FFFF => (
                self.shared_wram.read_byte(addr),
                if cycle.is_non_seq() {8} else {2}
            ),

            // I/O
            0x0400_0240..=0x0400_024B => (self.read_mem_control_byte(addr & 0xF), if cycle.is_non_seq() {8} else {2}),
            0x0400_1000..=0x0400_106F => (self.video.mem.mut_engine_b().registers.read_byte(addr & 0xFF), if cycle.is_non_seq() {8} else {2}),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_byte(addr), if cycle.is_non_seq() {8} else {2}),

            // VRAM
            0x0500_0000..=0x05FF_FFFF => (self.video.mem.read_byte_palette(addr & 0x7FF), if cycle.is_non_seq() {8} else {2}),
            0x0600_0000..=0x06FF_FFFF => (self.video.mem.read_byte_vram(addr), if cycle.is_non_seq() {8} else {2}),
            0x0700_0000..=0x07FF_FFFF => (self.video.mem.read_byte_oam(addr & 0x7FF), if cycle.is_non_seq() {8} else {2}),

            // TODO: GBA slot

            0xFFFF_0000..=0xFFFF_FFFF => (self.bios.read_byte(addr & 0xFFF), if cycle.is_non_seq() {8} else {2}),

            _ => (0, 2) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_byte(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {DS9_MAIN_RAM_N} else {DS9_MAIN_RAM_S}  // TODO: S=N for instr
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared RAM
                self.shared_wram.write_byte(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            // I/O
            0x0400_0240..=0x0400_024B => {
                self.write_mem_control_byte(addr & 0xF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_1000..=0x0400_106F => {
                self.video.mem.mut_engine_b().registers.write_byte(addr & 0xFF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_byte(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // VRAM
            0x0500_0000..=0x05FF_FFFF => {
                self.video.mem.write_byte_palette(addr & 0x7FF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0600_0000..=0x06FF_FFFF => {
                self.video.mem.write_byte_vram(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0700_0000..=0x07FF_FFFF => {
                self.video.mem.write_byte_oam(addr & 0x7FF, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // TODO: GBA slot

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => (
                self.main_ram.read_halfword(addr & 0x3F_FFFF),
                if cycle.is_non_seq() {DS9_MAIN_RAM_N} else {DS9_MAIN_RAM_S}  // TODO: S=N for instr
            ),
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_halfword(addr), if cycle.is_non_seq() {8} else {2}),

            // I/O
            0x0400_0240..=0x0400_024B => (self.read_mem_control_halfword(addr & 0xF), if cycle.is_non_seq() {8} else {2}),
            0x0400_1000..=0x0400_106F => (self.video.mem.mut_engine_b().registers.read_halfword(addr & 0xFF), 2),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_halfword(addr), if cycle.is_non_seq() {8} else {2}),

            // VRAM
            0x0500_0000..=0x05FF_FFFF => (self.video.mem.read_halfword_palette(addr & 0x7FF), if cycle.is_non_seq() {8} else {2}),
            0x0600_0000..=0x06FF_FFFF => (self.video.mem.read_halfword_vram(addr), if cycle.is_non_seq() {8} else {2}),
            0x0700_0000..=0x07FF_FFFF => (self.video.mem.read_halfword_oam(addr & 0x7FF), if cycle.is_non_seq() {8} else {2}),

            // TODO: GBA slot

            0xFFFF_0000..=0xFFFF_FFFF => (self.bios.read_halfword(addr & 0xFFF), if cycle.is_non_seq() {8} else {2}),

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_halfword(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {DS9_MAIN_RAM_N} else {DS9_MAIN_RAM_S}  // TODO: S=N for instr
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared WRAM
                self.shared_wram.write_halfword(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // I/O
            0x0400_0240..=0x0400_024B => {
                self.write_mem_control_halfword(addr & 0xF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_1000..=0x0400_106F => {
                self.video.mem.mut_engine_b().registers.write_halfword(addr & 0xFF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_0000..=0x04FF_FFFF => {
                self.io_write_halfword(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // VRAM
            0x0500_0000..=0x05FF_FFFF => {
                self.video.mem.write_halfword_palette(addr & 0x7FF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0600_0000..=0x06FF_FFFF => {
                self.video.mem.write_halfword_vram(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0700_0000..=0x07FF_FFFF => {
                self.video.mem.write_halfword_oam(addr & 0x7FF, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // TODO: GBA slot

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0200_0000..=0x02FF_FFFF => (
                self.main_ram.read_word(addr & 0x3F_FFFF),
                if cycle.is_non_seq() {DS9_MAIN_RAM_WORD_N} else {DS9_MAIN_RAM_WORD_S}  // TODO: S=N for instr
            ),
            0x0300_0000..=0x03FF_FFFF => (self.shared_wram.read_word(addr), if cycle.is_non_seq() {8} else {2}),

            // I/O
            0x0400_0240..=0x0400_024B => (self.read_mem_control_word(addr & 0xF), if cycle.is_non_seq() {8} else {2}),
            0x0400_1000..=0x0400_106F => (self.video.mem.mut_engine_b().registers.read_word(addr & 0xFF), 2),
            0x0410_0010..=0x0410_0013 => (self.card.read_word(addr), 10),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_word(addr), if cycle.is_non_seq() {8} else {2}),

            // VRAM
            0x0500_0000..=0x05FF_FFFF => (self.video.mem.read_word_palette(addr & 0x7FF), if cycle.is_non_seq() {10} else {4}),
            0x0600_0000..=0x06FF_FFFF => (self.video.mem.read_word_vram(addr), if cycle.is_non_seq() {10} else {4}),
            0x0700_0000..=0x07FF_FFFF => (self.video.mem.read_word_oam(addr & 0x7FF), if cycle.is_non_seq() {8} else {2}),

            // TODO: GBA slot

            0xFFFF_0000..=0xFFFF_FFFF => (self.bios.read_word(addr & 0xFFF), if cycle.is_non_seq() {8} else {2}),

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_word(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {DS9_MAIN_RAM_WORD_N} else {DS9_MAIN_RAM_WORD_S}  // TODO: S=N for instr
            },
            0x0300_0000..=0x03FF_FFFF => {  // Shared WRAM
                self.shared_wram.write_word(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // I/O
            0x0400_0240..=0x0400_024B => {
                self.write_mem_control_word(addr & 0xF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_1000..=0x0400_106F => {
                self.video.mem.mut_engine_b().registers.write_word(addr & 0xFF, data);
                if cycle.is_non_seq() {8} else {2}
            },
            0x0400_0000..=0x04FF_FFFF => {
                self.io_write_word(addr, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // VRAM
            0x0500_0000..=0x05FF_FFFF => {
                self.video.mem.write_word_palette(addr & 0x7FF, data);
                if cycle.is_non_seq() {10} else {4}
            },
            0x0600_0000..=0x06FF_FFFF => {
                self.video.mem.write_word_vram(addr, data);
                if cycle.is_non_seq() {10} else {4}
            },
            0x0700_0000..=0x07FF_FFFF => {
                self.video.mem.write_word_oam(addr & 0x7FF, data);
                if cycle.is_non_seq() {8} else {2}
            },

            // TODO: GBA slot

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
        (0x0400_0204, 0x0400_0207, ex_mem_control),
        (0x0400_0208, 0x0400_0217, interrupt_control),
        (0x0400_0280, 0x0400_02BF, accelerators),
        (0x0400_0300, 0x0400_0303, power_control),
        (0x0400_0304, 0x0400_06FF, video),
        (0x0410_0000, 0x0410_0003, ipc),
        (0x0410_0010, 0x0410_0013, card)
    }
}

/// Memory bus for DS ARM7 processor.
pub struct DS7MemoryBus {
    bios:           BIOS,
    power_control:  DS7PowerControl,

    main_ram:       MainRAM,
    wram:           RAM,
    shared_wram:    ARM7SharedRAM,

    video:          ARM7Video,
    vram:           ARM7VRAM,

    audio:          DSAudio,

    ipc:    IPC,

    timers:     Timers,
    joypad:     Joypad,
    ds_joypad:  DSJoypad,
    rtc:        RealTimeClock,
    spi:        SPI,

    dma:                ds7DMA,
    interrupt_control:  InterruptControl,
    ex_mem_status:      ExMemStatus,
    card:               DSCardIO,

    // Sync
    inner_counter:      usize,
    counter:            usize,
    v_counter:          usize,
    barrier:            Arc<Barrier>,
    input_recv:         Receiver<UserInput>
}

impl DS7MemoryBus {
    /// Setup ARM7 boot area, for fast booting without BIOS.
    pub fn setup_boot_area(&mut self, header: &CardHeader) {
        let boot_area_size = header.arm7_size() as usize;
        let mut buffer = vec![0_u8; boot_area_size];
        self.card.load_data(header.arm7_rom_offset(), &mut buffer);

        let arm7_addr = header.arm7_ram_addr();
        for (n, byte) in buffer.iter().enumerate() {
            self.store_byte(MemCycleType::N, arm7_addr + (n as u32), *byte);
        }
    }

    pub fn enable_audio(&mut self) -> Receiver<SamplePacket> {
        let (sample_tx, sample_rx) = unbounded();
        self.audio.enable_audio(sample_tx);
        sample_rx
    }
}

// Internal
impl DS7MemoryBus {
    /// Do a DMA transfer if possible.
    /// 
    /// This function clocks the memory bus internally.
    /// It will continue until the transfer is done.
    fn do_dma(&mut self) {
        let mut last_active = 4;
        loop {
            if let Some(c) = self.dma.get_active() {
                // Check if DMA channel has changed since last transfer.
                let access = if last_active != c {
                    last_active = c;
                    self.do_clock(4);
                    MemCycleType::N
                } else {
                    MemCycleType::S
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
                self.do_clock(cycles);
            } else {
                break;
            }
        }
    }

    /// Do an audio DMA transfer.
    /// 
    /// This function clocks the memory bus internally.
    /// It will continue until the transfer is done.
    fn do_audio_dma(&mut self, audio_channels: u16, audio_cap_dma_0: bool, audio_cap_dma_1: bool) {

        if audio_channels == 0 && !audio_cap_dma_0 && !audio_cap_dma_1 {
            return;
        }

        let mut cycle_count = 0;
        for chan_idx in (0..16).filter(|c| crate::utils::bits::u16::test_bit(audio_channels, *c)) {
            for _ in 0..4 {
                let addr = self.audio.get_dma_addr(chan_idx);
                let (data, load_cycles) = self.load_word(MemCycleType::S, addr);
                self.audio.write_fifo(chan_idx, data);
                cycle_count += load_cycles;
            }
        }
        if audio_cap_dma_0 {
            for _ in 0..4 {
                let addr = self.audio.get_capture_dma_addr(0);
                let data = self.audio.read_capture_fifo(0);
                cycle_count += self.store_word(MemCycleType::S, addr, data);
            }
        }
        if audio_cap_dma_1 {
            for _ in 0..4 {
                let addr = self.audio.get_capture_dma_addr(1);
                let data = self.audio.read_capture_fifo(1);
                cycle_count += self.store_word(MemCycleType::S, addr, data);
            }
        }
        self.do_clock(cycle_count);
    }

    /// Indicate to all of the devices on the memory bus that cycles have passed.
    fn do_clock(&mut self, cycles: usize) {
        let mut vblank = Interrupts::empty();

        self.counter += cycles;
        if self.counter >= ARM7_THREAD_SYNC_CYCLES {
            self.counter -= ARM7_THREAD_SYNC_CYCLES;
            self.barrier.wait();

            // Check buttons + touchpad
            if let Ok(new_input) = self.input_recv.try_recv() {
                self.set_input(new_input);
                self.card.flush_save();
            }
        }

        // TODO: make this a better const
        const V: usize = 6 * 355 * 263;
        self.v_counter += cycles;
        if self.v_counter >= V {
            self.v_counter -= V;
            self.dma.on_vblank();
            if self.video.v_blank_enabled() {
                vblank = Interrupts::V_BLANK;
            }
        }

        self.card.clock(cycles);
        if self.card.check_card_dma() {
            // TODO: make this more clear.
            // H-blank trigger setting on GBA is same as NDS card trigger.
            self.dma.on_hblank();
        }
        let v_count_irq = if self.video.v_count_irq() {
            Interrupts::V_COUNTER
        } else {
            Interrupts::empty()
        };

        let (timer_irq, _, _) = self.timers.clock(cycles);

        let (audio_channels, audio_cap_dma_0, audio_cap_dma_1) = self.audio.clock(cycles);
        self.do_audio_dma(audio_channels, audio_cap_dma_0, audio_cap_dma_1);

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
            v_count_irq |
            vblank
        );
    }

    fn check_irq(&self) -> bool {
        self.interrupt_control.irq()
    }

    fn set_input(&mut self, new_input: UserInput) {
        self.joypad.set_all_buttons(new_input.buttons);
        self.ds_joypad.set_all_buttons(new_input.ds_buttons);
        self.spi.write_tsc_values(new_input.touchscreen);
    }
}

impl Mem32 for DS7MemoryBus {
    type Addr = u32;

    fn clock(&mut self, cycles: usize) -> Option<arm::ExternalException> {
        // Check if CPU is halted.
        if self.power_control.halt {
            loop {
                if self.check_irq() {
                    self.power_control.halt = false;
                    return Some(arm::ExternalException::IRQ);
                }
                self.do_dma();
                self.do_clock(4);
            }
        } else {
            if self.dma.get_active().is_some() {
                self.do_dma();
            }
        }

        self.inner_counter += cycles;
        if self.inner_counter < 8 {
            if self.check_irq() {
                return Some(arm::ExternalException::IRQ);
            }
            return None;
        }

        self.do_clock(self.inner_counter);
        self.inner_counter = 0;

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

            0x0200_0000..=0x02FF_FFFF => (
                self.main_ram.read_byte(addr & 0x3F_FFFF),
                if cycle.is_non_seq() {DS7_MAIN_RAM_N} else {DS7_MAIN_RAM_S}
            ),
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_byte(addr), 1),
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_byte(addr & 0xFFFF), 1),

            0x0400_0240 => (self.vram.get_status(), 1),
            0x0400_0241 => (self.shared_wram.get_bank_status(), 1),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_byte(addr), 1),

            0x0600_0000..=0x06FF_FFFF => (self.vram.read_byte(addr), 1),

            // TODO: GBA slot

            _ => (0, 1) // Unused
        }
    }
    fn store_byte(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u8) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_byte(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {DS7_MAIN_RAM_N} else {DS7_MAIN_RAM_S}
            },
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_byte(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_byte(addr & 0xFFFF, data);
                1
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

            _ => 1 // Unused
        }
    }

    fn load_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u16, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_halfword(addr), 1),
            0x0200_0000..=0x02FF_FFFF => (
                self.main_ram.read_halfword(addr & 0x3F_FFFF),
                if cycle.is_non_seq() {DS7_MAIN_RAM_N} else {DS7_MAIN_RAM_S}
            ),
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_halfword(addr), 1),
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_halfword(addr & 0xFFFF), 1),

            0x0400_0000..=0x04FF_FFFF => (self.io_read_halfword(addr), 1),

            0x0600_0000..=0x06FF_FFFF => (self.vram.read_halfword(addr), 1),

            // TODO: GBA slot

            _ => (0, 1) // Unused
        }
    }
    fn store_halfword(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u16) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_halfword(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {DS7_MAIN_RAM_N} else {DS7_MAIN_RAM_S}
            },
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_halfword(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_halfword(addr & 0xFFFF, data);
                1
            },

            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_halfword(addr, data);
                1
            },

            0x0600_0000..=0x06FF_FFFF => {
                self.vram.write_halfword(addr, data);
                1
            },

            // TODO: GBA slot

            _ => 1 // Unused
        }
    }

    fn load_word(&mut self, cycle: MemCycleType, addr: Self::Addr) -> (u32, usize) {
        match addr {
            0x0000_0000..=0x0000_3FFF => (self.bios.read_word(addr), 1),
            0x0200_0000..=0x02FF_FFFF => (
                self.main_ram.read_word(addr & 0x3F_FFFF),
                if cycle.is_non_seq() {DS7_MAIN_RAM_WORD_N} else {DS7_MAIN_RAM_WORD_S}
            ),
            0x0300_0000..=0x037F_FFFF => (self.shared_wram.read_word(addr), 1),
            0x0380_0000..=0x03FF_FFFF => (self.wram.read_word(addr & 0xFFFF), 1),

            0x0410_0010..=0x0410_0013 => (self.card.read_word(addr), 5),
            0x0400_0000..=0x04FF_FFFF => (self.io_read_word(addr), 1),

            0x0600_0000..=0x06FF_FFFF => (self.vram.read_word(addr), 2),

            // TODO: GBA slot

            _ => (0, 1) // Unused
        }
    }
    fn store_word(&mut self, cycle: MemCycleType, addr: Self::Addr, data: u32) -> usize {
        match addr {
            0x0200_0000..=0x02FF_FFFF => {  // WRAM
                self.main_ram.write_word(addr & 0x3F_FFFF, data);
                if cycle.is_non_seq() {DS7_MAIN_RAM_WORD_N} else {DS7_MAIN_RAM_WORD_S}
            },
            0x0300_0000..=0x037F_FFFF => {  // Shared RAM
                self.shared_wram.write_word(addr, data);
                1
            },
            0x0380_0000..=0x03FF_FFFF => {  // ARM7 WRAM
                self.wram.write_word(addr & 0xFFFF, data);
                1
            },

            0x0400_0000..=0x04FF_FFFF => {  // I/O
                self.io_write_word(addr, data);
                1
            },

            0x0600_0000..=0x06FF_FFFF => {
                self.vram.write_word(addr, data);
                2
            },

            // TODO: GBA slot

            _ => 1 // Unused
        }
    }
}

impl DS7MemoryBus {
    MemoryBusIO!{
        (0x0400_0004, 0x0400_0007, video),
        (0x0400_00B0, 0x0400_00DF, dma),
        (0x0400_0100, 0x0400_010F, timers),
        (0x0400_0130, 0x0400_0133, joypad),
        (0x0400_0136, 0x0400_0137, ds_joypad),
        (0x0400_0138, 0x0400_013B, rtc),
        (0x0400_0180, 0x0400_018F, ipc),
        (0x0400_01A0, 0x0400_01BF, card),
        (0x0400_01C0, 0x0400_01C3, spi),
        (0x0400_0204, 0x0400_0207, ex_mem_status),
        (0x0400_0208, 0x0400_0217, interrupt_control),
        (0x0400_0300, 0x0400_0307, power_control),
        (0x0400_0400, 0x0400_051F, audio),
        (0x0410_0000, 0x0410_0003, ipc),
        (0x0410_0010, 0x0410_0013, card)
    }
}
